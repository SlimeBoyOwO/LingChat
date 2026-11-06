#!/usr/bin/env node
// 构建前给 cubek-matmul 打一处补丁：收紧矩阵乘法的自动降级链。
//
// 起因是 Android 真机（Adreno 750，共享内存每 cube 上限 32KB）上，burn 训练里的 matmul
// 走 cubek 的 Strategy::Auto，先试 SimpleCyclicCmma（Cmma 内核，动辄请求 35KB+ 共享内存，
// 是按桌面独显（nVidia）调优的）。Adreno 上这些配置全部 SharedMemoryTooBig，Auto 的一级
// 降级（SimpleUnit）若仍失败就把错误用 ? 向上抛，最外层 burn float_matmul 的 unwrap 直接
// panic 成「训练任务失败」。
//
// 补丁把 Auto 的降级链收严为带最终兜底：SimpleUnit 也失败（SharedMemoryTooBig 等任何
// Unavailable）就再降到 Naive——它是每单元只算一个输出元素的最简单 kernel，共享内存占用
// 极低，任何硬件都能跑；Naive 仍失败才算真正的不可用。这样 Adreno 上任何 shape 最终都能
// 落到可跑的 kernel。
//
// 源码不存进仓库：与 cubecl-wgpu 补丁同机制，构建前从 .crate 缓存（拿不到再从 crates.io
// 下载）解包到 vendor/cubek-matmul（已在 .gitignore），打完补丁留版本标记，重复执行跳过。

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const VERSION = "0.3.1";
const CRATE = `cubek-matmul-${VERSION}`;
const ROOT = path.resolve(import.meta.dirname, "..");
const DEST = path.join(ROOT, "vendor", "cubek-matmul");
const MARKER = path.join(DEST, ".lingchat-patched");
const STRATEGY_RS = path.join(DEST, "src", "strategy.rs");

/** cargo 的 home 目录，未显式设置时按默认位置。 */
function cargoHome() {
  return process.env.CARGO_HOME || path.join(os.homedir(), ".cargo");
}

/** 在 registry 缓存里找已下载的 .crate，找不到返回 null。 */
function findCachedCrate() {
  const cacheRoot = path.join(cargoHome(), "registry", "cache");
  if (!fs.existsSync(cacheRoot)) return null;
  for (const index of fs.readdirSync(cacheRoot)) {
    const candidate = path.join(cacheRoot, index, `${CRATE}.crate`);
    if (fs.existsSync(candidate)) return candidate;
  }
  return null;
}

/** 缓存没有就从 crates.io 下载到临时目录。 */
async function downloadCrate() {
  const url = `https://static.crates.io/crates/cubek-matmul/${CRATE}.crate`;
  console.log(`[patch-cubek-matmul] 缓存未命中，下载 ${url}`);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`下载失败（HTTP ${response.status}）：${url}`);
  }
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "cubek-matmul-")), `${CRATE}.crate`);
  fs.writeFileSync(file, Buffer.from(await response.arrayBuffer()));
  return file;
}

/** 把 .crate 解包成 vendor/cubek-matmul。 */
function unpack(crateFile) {
  const staging = fs.mkdtempSync(path.join(os.tmpdir(), "cubek-matmul-extract-"));
  execFileSync("tar", ["-xzf", crateFile, "-C", staging], { stdio: "inherit" });
  const extracted = path.join(staging, CRATE);
  if (!fs.existsSync(extracted)) {
    throw new Error(`解包后找不到 ${CRATE} 目录：${extracted}`);
  }
  fs.rmSync(DEST, { recursive: true, force: true });
  fs.mkdirSync(path.dirname(DEST), { recursive: true });
  fs.cpSync(extracted, DEST, { recursive: true });
  fs.rmSync(staging, { recursive: true, force: true });
}

/**
 * 收紧 multi-level 的 Strategy::Auto 降级链：SimpleCyclicCmma 失败后 SimpleUnit 若再失败
 * 就降到 Naive（最简单、共享内存最小、任何硬件可跑的最终兜底），而不是把错误向上抛出让
 * 外层 float_matmul 的 unwrap 变成 panic。锚点对不上就报错，避免版本升级后静默失效。
 */
function patchAuto() {
  const source = fs.readFileSync(STRATEGY_RS, "utf8");
  const anchor =
    "    if let Err(err) = multi_level::Strategy::SimpleCyclicCmma(Default::default()).launch_ref(\n        client,\n        lhs.clone(),\n        rhs.clone(),\n        out.clone(),\n        dtypes,\n    ) {\n        match err {\n            MatmulSetupError::Unavailable(_) => {\n                multi_level::Strategy::SimpleUnit(Default::default())\n                    .launch_ref(client, lhs, rhs, out, dtypes)?;\n            }\n            _ => panic!(\"{err:?}\"),\n        }\n    }";
  if (!source.includes(anchor)) {
    throw new Error("未找到 multi-level Strategy::Auto 降级链，cubek-matmul 版本可能已变，需要重新校对补丁");
  }

  const replacement = [
    "    if multi_level::Strategy::SimpleCyclicCmma(Default::default())",
    "        .launch_ref(client, lhs.clone(), rhs.clone(), out.clone(), dtypes)",
    "        .is_err()",
    "    {",
    "        if let Err(unit_err) = multi_level::Strategy::SimpleUnit(Default::default())",
    "            .launch_ref(client, lhs.clone(), rhs.clone(), out.clone(), dtypes)",
    "        {",
    "            // LingChat 补丁：SimpleCyclicCmma 一级降级的 SimpleUnit 仍失败（Adreno 上",
    "            // cmma/unit 配置都可能超出 32KB 共享内存预算）就把错误上抛，最外层 unwrap",
    "            // 会直接 panic。这里再降到 Naive——每单元只算一个输出元素的最简单 kernel，",
    "            // 任何硬件都能跑；仅当 Naive 也失败（unit_err）才算真正不可用。",
            "            if let Err(err) = multi_level::Strategy::Naive",
    "                .launch_ref(client, lhs, rhs, out, dtypes)",
    "            {",
    "                panic!(\"matmul 无可运行内核：{err:?}; unit 兜底失败：{unit_err:?}\");",
    "            }",
    "        }",
    "    }",
  ].join("\n");

  fs.writeFileSync(STRATEGY_RS, source.replace(anchor, replacement));
}

/** 补丁集清单 — 每次改动这里统一提升，用于让已解包的 vendor 强制重打。 */
const PATCH_SET = "1";

/** 已经是打完补丁的状态就跳过。 */
function alreadyPatched() {
  if (!fs.existsSync(MARKER)) return false;
  if (fs.readFileSync(MARKER, "utf8").trim() !== PATCH_SET) return false;
  return fs.existsSync(STRATEGY_RS);
}

async function main() {
  if (alreadyPatched()) {
    console.log(`[patch-cubek-matmul] ${CRATE} 已打过补丁，跳过`);
    return;
  }

  const crateFile = findCachedCrate() ?? (await downloadCrate());
  console.log(`[patch-cubek-matmul] 解包 ${crateFile}`);
  unpack(crateFile);
  patchAuto();
  fs.writeFileSync(MARKER, `${PATCH_SET}\n`);
  console.log(`[patch-cubek-matmul] 已收紧 Strategy::Auto 降级链：${DEST}`);
}

main().catch((error) => {
  console.error(`[patch-cubek-matmul] 失败：${error.message}`);
  process.exit(1);
});