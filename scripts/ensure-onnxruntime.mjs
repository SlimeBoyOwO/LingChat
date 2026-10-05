// ensure-onnxruntime.mjs
//
// 一次性准备 onnxruntime.dll + DirectML.dll：缺任一才下载（从 WinML NuGet 包提取），
// 然后复制到仓库根 target/{debug,release}（exe 同目录，供 ort::init_from 加载）。
// DirectML.dll 是 DML EP 的运行时依赖，缺了它选 GPU 会静默回落 CPU。
//
// 由 `pnpm run init` 调用（不再挂 beforeDevCommand/beforeBuildCommand，避免每次构建都检查）。
// 用法: node scripts/ensure-onnxruntime.mjs
// 输出: src-tauri/binaries/{onnxruntime,DirectML}.dll + target/{debug,release}/{同名}
//
// ⚠️ 别改回 ONNX Runtime 的 GitHub Releases：官方自 1.24.4 起不再发布 DirectML 版
// （GitHub / NuGet / PyPI 均已停更）。ort 的 `directml` feature 只是编译期声明，EP 实体
// 必须编在 dll 里 —— 不含 DML EP 时选 GPU 会**静默回落 CPU 且不报错**。
// WinML 版本号跟的是 Windows ML 而非 ORT，升级前须重验 api 版本匹配
// （不匹配会被 ort::init_from 以 BadVersion 拒绝）。

import {
  copyFileSync,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { execSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const binariesDir = join(projectRoot, "src-tauri", "binaries");

// WinML 包版本（非 ORT 版本；2.4.89 内含 ORT 1.27.1）
const WINML_VERSION = process.env.WINML_VERSION || "2.4.89";
const NUPKG_URL = `https://api.nuget.org/v3-flatcontainer/microsoft.windows.ai.machinelearning/${WINML_VERSION}/microsoft.windows.ai.machinelearning.${WINML_VERSION}.nupkg`;

// 包内固定路径。不能按文件名递归查找：包内另有 win-arm64 / win-arm64ec 两份同名 dll。
const WANTED = ["onnxruntime.dll", "DirectML.dll"];
const INNER_DIR = ["runtimes", "win-x64", "native"];

// 复制目标：仓库根 target 下的构建输出（exe 同目录，供 ort::init_from 加载）。
// 后端为 Cargo workspace，产物统一落在仓库根 target/，不是 src-tauri/target/。
const TARGET_DIRS = [join(projectRoot, "target", "debug"), join(projectRoot, "target", "release")];

// 目录现场（出错时打出来，省得靠猜）
const listDir = (d) => (existsSync(d) ? readdirSync(d).join(", ") || "(空目录)" : "(目录不存在)");
const outPath = (name) => join(binariesDir, name);

// 仅 Windows 需要这两个 dll（load-dynamic 只作用于 Windows target；
// Linux/macOS 保持 download-binaries 静态链接，无需 dll）。
if (process.platform !== "win32") {
  console.log("[onnx] 非 Windows 平台，跳过（load-dynamic 仅限 Windows）");
  process.exit(0);
}

async function download(url, dest) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText} for ${url}`);
  await pipeline(res.body, createWriteStream(dest));
}

/// 解压 nupkg（本质是 zip）到 destDir。
/// Windows 用 PowerShell 的 ZipFile：Expand-Archive 只认 .zip 扩展名，Git Bash 的
/// GNU tar 又把 `F:\` 当远程主机名，两条路都不通。非 Windows 用 tar。
function extract(archive, destDir) {
  if (process.platform === "win32") {
    execSync(
      `powershell -NoProfile -Command "Add-Type -AssemblyName System.IO.Compression.FileSystem; ` +
        `[System.IO.Compression.ZipFile]::ExtractToDirectory('${archive}','${destDir}')"`,
      { stdio: "inherit" },
    );
  } else {
    execSync(`tar -xf "${archive}" -C "${destDir}"`, { stdio: "inherit" });
  }
}

/// 确保 binaries/ 下两个 dll 都在（任一缺失才下载，幂等）。
async function fetchDlls() {
  const missing = WANTED.filter((n) => !existsSync(outPath(n)));
  if (missing.length === 0) {
    const sizes = WANTED.map(
      (n) => `${n} (${(statSync(outPath(n)).size / 1024 / 1024).toFixed(1)} MB)`,
    );
    console.log(`[onnx] 已存在，跳过下载: ${sizes.join(", ")}`);
    return;
  }

  mkdirSync(binariesDir, { recursive: true });
  // 临时文件用 .zip 扩展名：ZipFile / Expand-Archive / tar 都认
  const tmpZip = join(binariesDir, `winml-${WINML_VERSION}.zip`);
  const extractDir = join(binariesDir, `extract-${WINML_VERSION}`);

  try {
    console.log(`[onnx] ⬇️  下载 ${NUPKG_URL}`);
    await download(NUPKG_URL, tmpZip);

    // ZipFile.ExtractToDirectory 要求目标目录为空或不存在
    rmSync(extractDir, { recursive: true, force: true });
    mkdirSync(extractDir, { recursive: true });
    extract(tmpZip, extractDir);

    // 按固定路径取文件（见 INNER_DIR 的注释，不用递归查找）
    for (const name of WANTED) {
      const src = join(extractDir, ...INNER_DIR, name);
      if (!existsSync(src)) {
        throw new Error(`包内未找到 ${INNER_DIR.join("/")}/${name}（解压目录: ${extractDir}）`);
      }
      copyFileSync(src, outPath(name));
    }
    for (const name of WANTED) {
      const size = statSync(outPath(name)).size;
      console.log(`[onnx] ✅ ${name} 就绪 (${(size / 1024 / 1024).toFixed(1)} MB)`);
    }

    // 下载脚本正常退出 ≠ 文件真的落盘（CI 上踩过：它自己打印了「就绪」，下一步却找不到）
    const gone = WANTED.filter((n) => !existsSync(outPath(n)));
    if (gone.length > 0) {
      throw new Error(
        `${gone.join(", ")} 不在 ${binariesDir}；当前内容: ${listDir(binariesDir)}`,
      );
    }
  } finally {
    // 成功失败都清理临时文件，避免半成品留在 binaries/ 里
    rmSync(extractDir, { recursive: true, force: true });
    rmSync(tmpZip, { force: true });
  }
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

/// 把两个 dll 放到 exe 同目录。目录不存在就建出来，让 init 一次到位：
/// init 通常早于首次构建，此时 target/ 还没生成，直接复制会让 dev 首启缺 dll。
function placeDlls() {
  for (const dir of TARGET_DIRS) {
    mkdirSync(dir, { recursive: true });
    for (const name of WANTED) {
      const src = outPath(name);
      if (!existsSync(src)) {
        throw new Error(`拷贝前 ${name} 已不在 ${binariesDir}；当前内容: ${listDir(binariesDir)}`);
      }
      copyTo(src, join(dir, name), name);
    }
    console.log(`[onnx] 已复制到 ${dir}`);
  }
}

async function main() {
  await fetchDlls();
  placeDlls();
}

main().catch((e) => {
  console.error("❌ onnxruntime 准备失败:", e.message);
  process.exit(1);
});
