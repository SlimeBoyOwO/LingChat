// download-onnxruntime.mjs
//
// 下载 onnxruntime.dll + DirectML.dll（从 WinML NuGet 包中提取）。
//
// ⚠️ 别改回 ONNX Runtime 的 GitHub Releases：官方自 1.24.4 起不再发布 DirectML 版
// （GitHub / NuGet / PyPI 均已停更）。ort 的 `directml` feature 只是编译期声明，EP 实体
// 必须编在 dll 里 —— 不含 DML EP 时选 GPU 会**静默回落 CPU 且不报错**。
//
// 用法: node scripts/download-onnxruntime.mjs
// 输出: src-tauri/binaries/{onnxruntime,DirectML}.dll
//
// WinML 版本号跟的是 Windows ML 而非 ORT，升级前须重验 api 版本匹配
// （不匹配会被 ort::init_from 以 BadVersion 拒绝）。

import { createWriteStream, existsSync, mkdirSync, copyFileSync, rmSync, statSync } from "node:fs";
import { execSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const outDir = join(projectRoot, "src-tauri", "binaries");

// WinML 包版本（非 ORT 版本；2.4.89 内含 ORT 1.27.1）
const WINML_VERSION = process.env.WINML_VERSION || "2.4.89";
const NUPKG_URL = `https://api.nuget.org/v3-flatcontainer/microsoft.windows.ai.machinelearning/${WINML_VERSION}/microsoft.windows.ai.machinelearning.${WINML_VERSION}.nupkg`;

// 包内固定路径。不能按文件名递归查找：包内另有 win-arm64 / win-arm64ec 两份同名 dll。
const WANTED = ["onnxruntime.dll", "DirectML.dll"];
const INNER_DIR = ["runtimes", "win-x64", "native"];

const outPath = (name) => join(outDir, name);

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

async function main() {
  const missing = WANTED.filter((n) => !existsSync(outPath(n)));
  if (missing.length === 0) {
    const sizes = WANTED.map(
      (n) => `${n} (${(statSync(outPath(n)).size / 1024 / 1024).toFixed(1)} MB)`,
    );
    console.log(`✅ 已存在，跳过下载: ${sizes.join(", ")}`);
    return;
  }

  mkdirSync(outDir, { recursive: true });
  // 临时文件用 .zip 扩展名：ZipFile / Expand-Archive / tar 都认
  const tmpZip = join(outDir, `winml-${WINML_VERSION}.zip`);
  const extractDir = join(outDir, `extract-${WINML_VERSION}`);

  try {
    console.log(`⬇️  下载 ${NUPKG_URL}`);
    await download(NUPKG_URL, tmpZip);
    console.log("✅ 下载完成，解压 ...");

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
      console.log(`✅ ${name} 就绪 (${(size / 1024 / 1024).toFixed(1)} MB)`);
    }
    console.log("   开发运行：ensure-onnxruntime.mjs 会自动复制到 exe 同目录");
  } finally {
    // 成功失败都清理临时文件，避免半成品留在 binaries/ 里
    rmSync(extractDir, { recursive: true, force: true });
    rmSync(tmpZip, { force: true });
  }
}

main().catch((e) => {
  console.error("❌ 下载 onnxruntime 失败:", e.message);
  process.exit(1);
});
