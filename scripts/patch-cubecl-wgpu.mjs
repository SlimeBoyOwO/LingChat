#!/usr/bin/env node
// 构建前给 cubecl-wgpu 打一处补丁：禁用 WgpuStream::write_mapped 这条快路径。
//
// 起因是 Android 真机（Adreno 750，骁龙 8 Gen 3）上跑 burn 训练必崩：cubecl-wgpu 为
// 集成 GPU 特制了一条「统一内存」路径，把 Vulkan 内存长期 map 住、缓存裸指针
// host_ptr，上传张量时直接 memcpy 进去，绕开 wgpu 的校验。驱动回收或重映射之后
// 这个指针会失效，于是写到未映射地址，进程直接 SIGSEGV。
//
// 桌面独显不受影响：device_type 不是 INTEGRATED_GPU 时 host_ptr 恒为 None，这条路径
// 根本不会走——这正是「桌面同一份数据跑满一轮没事、真机必崩」的原因。
//
// 补丁只是让 write_mapped 返回 false，写入改走 wgpu 校验过的 queue.write_buffer，
// 语义不变，代价是上传稍慢。
//
// 源码不存进仓库：每次从 cargo 的 .crate 缓存（拿不到再从 crates.io 下载）解包到
// vendor/cubecl-wgpu（已在 .gitignore 里），打完补丁留一个版本标记，重复执行会跳过。

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const VERSION = "0.11.3";
const CRATE = `cubecl-wgpu-${VERSION}`;
const ROOT = path.resolve(import.meta.dirname, "..");
const DEST = path.join(ROOT, "vendor", "cubecl-wgpu");
const MARKER = path.join(DEST, ".lingchat-patched");
const STREAM_RS = path.join(DEST, "src", "compute", "stream.rs");
const RUNTIME_RS = path.join(DEST, "src", "runtime.rs");

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
  const url = `https://static.crates.io/crates/cubecl-wgpu/${CRATE}.crate`;
  console.log(`[patch-cubecl-wgpu] 缓存未命中，下载 ${url}`);
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`下载失败（HTTP ${response.status}）：${url}`);
  }
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "cubecl-wgpu-")), `${CRATE}.crate`);
  fs.writeFileSync(file, Buffer.from(await response.arrayBuffer()));
  return file;
}

/** 把 .crate 解包成 vendor/cubecl-wgpu。 */
function unpack(crateFile) {
  const staging = fs.mkdtempSync(path.join(os.tmpdir(), "cubecl-wgpu-extract-"));
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

/** 替换 write_mapped 的函数体。锚点对不上就报错，避免版本升级后静默失效。 */
function patchStream() {
  const source = fs.readFileSync(STREAM_RS, "utf8");
  const head = '    #[cfg(not(target_family = "wasm"))]\n    fn write_mapped(';
  const start = source.indexOf(head);
  if (start < 0) {
    throw new Error("未找到 write_mapped 的定义，cubecl-wgpu 版本可能已变，需要重新校对补丁");
  }
  // 函数体一直延续到下一个 wasm 版本的定义之前。
  const tail = '    #[cfg(target_family = "wasm")]\n    fn write_mapped(';
  const end = source.indexOf(tail, start);
  if (end < 0) {
    throw new Error("未找到 write_mapped 的 wasm 版本，无法确定函数体范围");
  }

  const replacement = [
    '    #[cfg(not(target_family = "wasm"))]',
    "    fn write_mapped(&mut self, resource: &WgpuResource, data: &[u8]) -> bool {",
    "        // LingChat 补丁：集成 GPU 的 host_ptr 是持久映射出来的裸指针，驱动回收内存或",
    "        // 重新映射后它就会失效，直接 memcpy 会写到未映射地址（Android Adreno 实测",
    "        // SIGSEGV）。这里一律放弃这条快路径，回退到 wgpu 校验过的队列写入。",
    "        let _ = (resource, data);",
    "        false",
    "    }",
    "",
    "",
  ].join("\n");

  fs.writeFileSync(STREAM_RS, source.slice(0, start) + replacement + source.slice(end));
}

// 补丁后 host_ptr 不再被读取，给两个存储结构体的字段标 allow 免除 dead_code 警告，
// 保持仓库零警告（编译器对 upstream 的私有结构体不会主动豁免）。
const STORAGE_RS = path.join(DEST, "src", "compute", "storage.rs");

function patchStorage() {
  const source = fs.readFileSync(STORAGE_RS, "utf8");
  const from = "pub(crate) struct HostPtr(pub(crate) NonNull<u8>);";
  const to = "pub(crate) struct HostPtr(#[allow(dead_code)] pub(crate) NonNull<u8>);";
  if (!source.includes(from)) {
    throw new Error("未找到 HostPtr 定义，无法标记 dead_code，需要重新校对补丁");
  }
  const step1 = source.replace(from, to);

  const f2 =
    '    /// The buffer host mapping, if host visible.\n    #[new(default)]\n    pub(crate) host_ptr: Option<HostPtr>,\n    /// The buffer offset.';
  const r2 =
    '    /// The buffer host mapping, if host visible.\n    #[allow(dead_code)]\n    #[new(default)]\n    pub(crate) host_ptr: Option<HostPtr>,\n    /// The buffer offset.';
  if (!step1.includes(f2)) {
    throw new Error("未找到 WgpuResource.host_ptr 定义，无法标记 dead_code，需要重新校对补丁");
  }
  const step2 = step1.replace(f2, r2);

  fs.writeFileSync(STORAGE_RS, step2);
}

/** 补丁集清单 — 每次改动这里统一提升，用于让已解包的 vendor 强制重打。 */
const PATCH_SET = "5";

/** 已经是打完补丁的状态就跳过。 */
function alreadyPatched() {
  if (!fs.existsSync(MARKER)) return false;
  if (fs.readFileSync(MARKER, "utf8").trim() !== PATCH_SET) return false;
  return fs.existsSync(STREAM_RS) && fs.existsSync(STORAGE_RS) && fs.existsSync(RUNTIME_RS);
}

/**
 * Android（Adreno）上把 Vulkan 设备请求改走 GLES。
 *
 * Adreno 的 Vulkan 驱动在 wgpu 训练计算管线（尤其扩展/回读）上仍会触发设备毒化
 * （device poisoned / Unexpected error variant），而 Android GLES 驱动对
 * OpenGL ES 3.1 计算管线的覆盖相对稳定。这里把 Android 上 `WgpuBackend::Vulkan`
 * 的请求映射到 `wgpu::Backend::Gl`（GLES），训练计算走 GLES；其它平台仍走 Vulkan。
 *
 * wgpu 的 GL 后端只支持 GLES 3.1+ 的计算能力（node 需 `DownlevelHighlights` 支持
 * compute），Adreno 的 GLES 3.1 计算支持完整，可行。锚点对不上就报错，避免升级静默失效。
 */
function patchRuntime() {
  const source = fs.readFileSync(RUNTIME_RS, "utf8");
  const anchor = [
    "        WgpuBackend::Auto => AutoGraphicsApi::chain(),",
    "        WgpuBackend::Vulkan => alloc::vec![wgpu::Backend::Vulkan],",
  ].join("\n");
  const replacement = [
    "        WgpuBackend::Auto => AutoGraphicsApi::chain(),",
    "        // LingChat 补丁：Android（Adreno）的 Vulkan 驱动在训练计算管线（扩展/回读）上",
    "        // 会触发设备毒化（device poisoned），改走 GLES（wgpu::Backend::Gl）绕开这条",
    "        // 驱动路径；桌面等其它平台仍走 Vulkan。仅 Android 生效。",
    "        #[cfg(target_os = \"android\")]",
    "        WgpuBackend::Vulkan => alloc::vec![wgpu::Backend::Gl],",
    "        #[cfg(not(target_os = \"android\"))]",
    "        WgpuBackend::Vulkan => alloc::vec![wgpu::Backend::Vulkan],",
  ].join("\n");
  if (!source.includes(anchor)) {
    throw new Error("未找到 backend_candidates 的 Vulkan 分支，cubecl-wgpu 版本可能已变，需要重新校对补丁");
  }
  fs.writeFileSync(RUNTIME_RS, source.replace(anchor, replacement));
}

async function main() {
  if (alreadyPatched()) {
    console.log(`[patch-cubecl-wgpu] ${CRATE} 已打过补丁，跳过`);
    return;
  }

  const crateFile = findCachedCrate() ?? (await downloadCrate());
  console.log(`[patch-cubecl-wgpu] 解包 ${crateFile}`);
  unpack(crateFile);
  patchStream();
  patchStorage();
  patchRuntime();
  fs.writeFileSync(MARKER, `${PATCH_SET}\n`);
  console.log(`[patch-cubecl-wgpu] 已禁用 write_mapped、Android 改走 GLES 并消除 dead_code 警告：${DEST}`);
}

main().catch((error) => {
  console.error(`[patch-cubecl-wgpu] 失败：${error.message}`);
  process.exit(1);
});
