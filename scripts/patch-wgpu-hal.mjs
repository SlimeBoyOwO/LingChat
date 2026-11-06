#!/usr/bin/env node
// 构建前给 wgpu-hal 的 GLES 后端打一处补丁：compute 阶段的 storage buffer 上限改按
// compute 值上报，而不是沿用 min(顶点stage, 片段stage)。
//
// 起因是 Android 真机（Adreno）上 burn 训练走到标准卷积内核（cubek-convolution 的
// direct_conv2d）时，wgpu 报 "Too many bindings of type StorageBuffers in Stage
// COMPUTE, limit is 4, count was 5"。
//
// wgpu-hal 的 GLES 后端把 max_storage_buffers_per_shader_stage 取成
// min(GL_MAX_VERTEX_SHADER_STORAGE_BLOCKS, GL_MAX_FRAGMENT_SHADER_STORAGE_BLOCKS)，
// Adreno 的顶点 stage 只有 4，于是整条上报被拖到 4；但 compute 用的
// GL_MAX_COMPUTE_SHADER_STORAGE_BLOCKS 按 GLES 3.1 规范下限就是 8，硬件其实够。
// burn/cubecl 只发 compute 内核，绑定布局全是 compute stage，所以这里在支持 compute
// 时取 max(compute, min(顶点, 片段))，既满足训练需要，又不至于比硬件真正能力多报。
//
// 源码不存进仓库：每次从 cargo 的 .crate 缓存（拿不到再从 crates.io 下载）解包到
// vendor/wgpu-hal（在 .gitignore 里），打完补丁留一个版本标记，重复执行会跳过。

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const VERSION = "30.0.1";
const CRATE = `wgpu-hal-${VERSION}`;
const ROOT = path.resolve(import.meta.dirname, "..");
const DEST = path.join(ROOT, "vendor", "wgpu-hal");
const MARKER = path.join(DEST, ".lingchat-patched");
const ADAPTER_RS = path.join(DEST, "src", "gles", "adapter.rs");

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
  const url = `https://static.crates.io/crates/wgpu-hal/${CRATE}.crate`;
  console.log(`[patch-wgpu-hal] 缓存未命中，下载 ${url}`);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`下载失败（HTTP ${response.status}）：${url}`);
  }
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "wgpu-hal-")), `${CRATE}.crate`);
  fs.writeFileSync(file, Buffer.from(await response.arrayBuffer()));
  return file;
}

/** 把 .crate 解包成 vendor/wgpu-hal。 */
function unpack(crateFile) {
  const staging = fs.mkdtempSync(path.join(os.tmpdir(), "wgpu-hal-extract-"));
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

/** 补丁集清单 — 每次改动这里统一提升，用于让已解包的 vendor 强制重打。 */
const PATCH_SET = "1";

/** 跳过标记同时校验关键补丁文件存在，避免只留版本号却没打上。 */
function alreadyPatched() {
  if (!fs.existsSync(MARKER)) return false;
  if (fs.readFileSync(MARKER, "utf8").trim() !== PATCH_SET) return false;
  return fs.existsSync(ADAPTER_RS);
}

/**
 * Android（Adreno）GLES 下 compute 阶段的 storage buffer 上限改按 compute 上报。
 *
 * 锚点对不上就报错，避免版本升级后静默失效。补丁分两步：
 *  1. 在 storage 限制读取区补一个 compute 值；
 *  2. 把 max_storage_buffers_per_shader_stage 改为支持 compute 时取
 *     max(compute, min(顶点, 片段))。
 */
function patchAdapter() {
  let source = fs.readFileSync(ADAPTER_RS, "utf8");

  // 第一步：在 fragment_shader_storage_textures 读完之后补 compute 值读数。
  const insertAfter = [
    "        let fragment_shader_storage_textures = if supports_storage {",
    "            (unsafe { gl.get_parameter_i32(glow::MAX_FRAGMENT_IMAGE_UNIFORMS) } as u32)",
    "        } else {",
    "            0",
    "        };",
  ].join("\n");
  const inserted = [
    insertAfter,
    "        // LingChat 补丁：compute 阶段单独读 GL_MAX_COMPUTE_SHADER_STORAGE_BLOCKS，",
    "        // 供下方计算 storage buffer 上限时取其值（GLES 3.1 下限为 8）。",
    "        let compute_shader_storage_blocks = if supports_compute {",
    "            (unsafe { gl.get_parameter_i32(glow::MAX_COMPUTE_SHADER_STORAGE_BLOCKS) } as u32)",
    "        } else {",
    "            0",
    "        };",
  ].join("\n");
  if (!source.includes(insertAfter)) {
    throw new Error("未找到 fragment_shader_storage_textures 读取块，wgpu-hal 版本可能已变，需要重新校对补丁");
  }
  source = source.replace(insertAfter, inserted);

  // 第二步：替换 max_storage_buffers_per_shader_stage 的取法。
  const oldResolution = [
    "        let max_storage_buffers_per_shader_stage = if vertex_shader_storage_blocks == 0 {",
    "            fragment_shader_storage_blocks",
    "        } else {",
    "            vertex_shader_storage_blocks.min(fragment_shader_storage_blocks)",
    "        };",
  ].join("\n");
  const newResolution = [
    "        let max_storage_buffers_per_shader_stage = if vertex_shader_storage_blocks == 0 {",
    "            fragment_shader_storage_blocks",
    "        } else if supports_compute && compute_shader_storage_blocks != 0 {",
    "            // LingChat 补丁：burn/cubecl 只发 compute 内核，其绑定布局全是 compute stage。",
    "            // 这里以 compute 值为主（Adreno 顶点只有 4 会整个拖低），但又不低于原来",
    "            // min(顶点, 片段) 的结果，避免对非 compute 用法多报。",
    "            vertex_shader_storage_blocks.min(fragment_shader_storage_blocks).max(compute_shader_storage_blocks)",
    "        } else {",
    "            vertex_shader_storage_blocks.min(fragment_shader_storage_blocks)",
    "        };",
  ].join("\n");
  if (!source.includes(oldResolution)) {
    throw new Error("未找到 max_storage_buffers_per_shader_stage 的取法，wgpu-hal 版本可能已变，需要重新校对补丁");
  }
  source = source.replace(oldResolution, newResolution);

  fs.writeFileSync(ADAPTER_RS, source);
}

async function main() {
  if (alreadyPatched()) {
    console.log(`[patch-wgpu-hal] ${CRATE} 已打过补丁，跳过`);
    return;
  }

  const crateFile = findCachedCrate() ?? (await downloadCrate());
  console.log(`[patch-wgpu-hal] 解包 ${crateFile}`);
  unpack(crateFile);
  patchAdapter();
  fs.writeFileSync(MARKER, `${PATCH_SET}\n`);
  console.log(`[patch-wgpu-hal] 已把 GLES compute 阶段 storage buffer 上限改按 compute 上报：${DEST}`);
}

main().catch((error) => {
  console.error(`[patch-wgpu-hal] 失败：${error.message}`);
  process.exit(1);
});