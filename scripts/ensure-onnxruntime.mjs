// ensure-onnxruntime.mjs
//
// 幂等确保 onnxruntime.dll + DirectML.dll 就位：缺任一才下载，然后复制到
// target/{debug,release}（exe 同目录，供 ort::init_from 加载）。DirectML.dll 是 DML EP
// 的运行时依赖，缺了它选 GPU 会静默回落 CPU。
//
// 由 tauri.conf.json 的 beforeDevCommand / beforeBuildCommand 调用，CI 也会显式调用。
// 用法: node scripts/ensure-onnxruntime.mjs

import { execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const srcTauri = join(projectRoot, "src-tauri");
const binariesDir = join(srcTauri, "binaries");
const downloadScript = join(projectRoot, "scripts", "download-onnxruntime.mjs");

// 需要就位的两个 dll（onnxruntime.dll 含 CPU + DirectML EP；DirectML.dll 是其运行时依赖）
const DLLS = ["onnxruntime.dll", "DirectML.dll"];

// 需要 dll 的输出目录（存在才复制；tauri dev/build 的默认输出）
const TARGET_DIRS = [join(srcTauri, "target", "debug"), join(srcTauri, "target", "release")];

// 目录现场（出错时打出来，省得靠猜）
const listDir = (d) => (existsSync(d) ? readdirSync(d).join(", ") || "(空目录)" : "(目录不存在)");

// 仅 Windows 需要这两个 dll（load-dynamic 只作用于 Windows target；
// Linux/macOS 保持 download-binaries 静态链接，无需 dll）。
// 否则 Linux/macOS 的 CI（beforeBuildCommand 触发本脚本）会白下载几十 MB Windows dll。
if (process.platform !== "win32") {
  console.log("[onnx] 非 Windows 平台，跳过（load-dynamic 仅限 Windows）");
  process.exit(0);
}

// 1. 确保 binaries/ 下两个 dll 都在（任一缺失才下载，幂等）
const missing = DLLS.filter((n) => !existsSync(join(binariesDir, n)));
if (missing.length > 0) {
  console.log(`[onnx] 缺失 ${missing.join(", ")}，开始下载（scripts/download-onnxruntime.mjs）...`);
  execSync(`node "${downloadScript}"`, { stdio: "inherit" });
  // 下载脚本正常退出 ≠ 文件真的落盘（CI 上踩过：它自己打印了「就绪」，下一步却找不到）
  const gone = DLLS.filter((n) => !existsSync(join(binariesDir, n)));
  if (gone.length > 0) {
    throw new Error(
      `下载脚本已退出，但 ${gone.join(", ")} 不在 ${binariesDir}；当前内容: ${listDir(binariesDir)}`,
    );
  }
} else {
  console.log("[onnx] onnxruntime.dll 与 DirectML.dll 均已存在");
}

/// 拷贝单个 dll。
///
/// ⚠️ 必须先清目标再写：CI 上 `target/` 由缓存恢复，可能残留**悬空的重解析点**
/// （符号链接/junction）。此时 `existsSync(dest)` 为 false 骗过所有存在性检查，
/// 而 `writeFileSync` 会以 `ENOENT (open)` 失败——`CREATE_ALWAYS` 穿透到了不存在的
/// 目标。已本地复现（建 junction 再删其目标）。清掉即可。
function copyTo(src, dest, name) {
  try {
    rmSync(dest, { recursive: true, force: true });
    writeFileSync(dest, readFileSync(src));
  } catch (e) {
    console.error(`[onnx] 拷贝 ${name} 失败: ${e.code}；${binariesDir} = ${listDir(binariesDir)}`);
    throw e;
  }
}

// 2. 复制到各输出目录（幂等：直接覆盖为当前版本）
let copied = false;
for (const dir of TARGET_DIRS) {
  if (!existsSync(dir)) continue;
  for (const name of DLLS) {
    const src = join(binariesDir, name);
    if (!existsSync(src)) {
      throw new Error(`拷贝前 ${name} 已不在 ${binariesDir}；当前内容: ${listDir(binariesDir)}`);
    }
    copyTo(src, join(dir, name), name);
  }
  console.log(`[onnx] 已复制到 ${dir}`);
  copied = true;
}
if (!copied) {
  console.log(
    "[onnx] 未找到 target/{debug,release} 目录，跳过复制（构建时将由 tauri 打包 resources 处理）",
  );
}
