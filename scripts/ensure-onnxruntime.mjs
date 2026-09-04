// ensure-onnxruntime.mjs
//
// 幂等确保 onnxruntime.dll 就位（开发/打包零操作）。
// 由 tauri.conf.json 的 beforeDevCommand / beforeBuildCommand 自动调用：
//   - dll 缺失时才下载（有则跳过，不重复下载）
//   - 复制到 target/debug、target/release（exe 同目录，供 ort::init_from 加载）
//
// 用法:
//   node scripts/ensure-onnxruntime.mjs
//
// 输出:
//   src-tauri/binaries/onnxruntime.dll （必要时下载）
//   src-tauri/target/{debug,release}/onnxruntime.dll （复制）

import { execSync } from "node:child_process";
import { copyFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const srcTauri = join(projectRoot, "src-tauri");
const dll = join(srcTauri, "binaries", "onnxruntime.dll");
const downloadScript = join(projectRoot, "scripts", "download-onnxruntime.mjs");

// 需要 dll 的输出目录（存在才复制；tauri dev/build 的默认输出）
const TARGET_DIRS = [join(srcTauri, "target", "debug"), join(srcTauri, "target", "release")];

// 仅 Windows 需要 onnxruntime.dll（load-dynamic 只作用于 Windows target；
// Linux/macOS 保持 download-binaries 静态链接，无需 dll）。
// 否则 Linux/macOS 的 CI（beforeBuildCommand 触发本脚本）会白下载 59MB Windows dll。
if (process.platform !== "win32") {
  console.log("[onnx] 非 Windows 平台，跳过（load-dynamic 仅限 Windows）");
  process.exit(0);
}

// 1. 确保 binaries/onnxruntime.dll 存在（缺失才下载，幂等）
if (!existsSync(dll)) {
  console.log("[onnx] onnxruntime.dll 缺失，开始下载（scripts/download-onnxruntime.mjs）...");
  execSync(`node "${downloadScript}"`, { stdio: "inherit" });
} else {
  console.log(`[onnx] onnxruntime.dll 已存在: ${dll}`);
}

// 2. 复制到各输出目录（幂等：直接覆盖为当前版本）
let copied = false;
for (const dir of TARGET_DIRS) {
  if (existsSync(dir)) {
    const dest = join(dir, "onnxruntime.dll");
    copyFileSync(dll, dest);
    console.log(`[onnx] 已复制到 ${dest}`);
    copied = true;
  }
}
if (!copied) {
  console.log("[onnx] 未找到 target/{debug,release} 目录，跳过复制（构建时将由 tauri 打包 resources 处理）");
}
