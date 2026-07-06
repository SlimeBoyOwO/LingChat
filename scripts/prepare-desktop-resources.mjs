/**
 * 桌面端构建前脚本：准备干净的默认资源目录。
 *
 * 通过 `git ls-files data/` 获取 git 跟踪的文件，仅将这些文件复制到
 * src-tauri/.bundled_resources/ 供 Tauri bundle.resources 打包。
 * 这天然尊重 .gitignore —— 被忽略的文件（截图/语音/未发布角色等）不会进入安装包。
 *
 * 用法:
 *   node scripts/prepare-desktop-resources.mjs
 *
 * 由 tauri.conf.json 的 beforeBuildCommand 自动调用。
 */

import { createHash } from "node:crypto";
import { execSync } from "node:child_process";
import { existsSync, mkdirSync, copyFileSync, rmSync, readdirSync, statSync, writeFileSync, readFileSync } from "node:fs";
import { join, dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const srcTauri = join(projectRoot, "src-tauri");
const stagingDir = join(srcTauri, ".bundled_resources");

// ─── 清理旧的 staging 目录 ──────────────────────────────────

if (existsSync(stagingDir)) {
  rmSync(stagingDir, { recursive: true });
}
mkdirSync(stagingDir, { recursive: true });

// ─── 获取 git 跟踪的 data/ 文件 ─────────────────────────────

let gitFiles = [];
try {
  const output = execSync("git -c core.quotepath=false ls-files data/", {
    cwd: projectRoot,
    encoding: "utf-8",
  });
  gitFiles = output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
} catch (err) {
  console.error("❌ 无法运行 git ls-files");
  process.exit(1);
}

console.log(`📋 git 跟踪了 ${gitFiles.length} 个文件（data/ 下）`);

// ─── 分类文件 ───────────────────────────────────────────────

/** 需要跳过的文件（占位符、训练数据等） */
const SKIP_PATTERNS = [
  ".gitkeep",
  "model.onnx.data", // 391MB 训练数据，运行时不需要
];

/** third_party 不通过 git ls-files（模型文件被 .gitignore 排除），直接从磁盘复制 */

const gameDataFiles = [];

for (const relPath of gitFiles) {
  // 去掉 "data/" 前缀
  const subPath = relPath.slice("data/".length);
  if (!subPath) continue;

  // 跳过 third_party（单独从磁盘复制）和占位文件
  if (subPath.startsWith("third_party/")) continue;

  const fileName = subPath.split("/").pop();
  if (SKIP_PATTERNS.includes(fileName) || SKIP_PATTERNS.includes(subPath)) {
    console.log(`  ⏭ 跳过: ${subPath}`);
    continue;
  }

  gameDataFiles.push(subPath);
}

console.log(`   game_data: ${gameDataFiles.length} 个文件`);

// ─── 复制 game_data 文件 ────────────────────────────────────

let gameDataCount = 0;
for (const subPath of gameDataFiles) {
  const src = join(projectRoot, "data", subPath);
  // subPath 已经是 "game_data/backgrounds/白天.webp" 这种形式
  const dst = join(stagingDir, subPath);

  if (!existsSync(src)) {
    console.warn(`  ⚠ 文件不存在: ${subPath}`);
    continue;
  }

  mkdirSync(dirname(dst), { recursive: true });
  copyFileSync(src, dst);
  gameDataCount++;
}
console.log(`✅ 已复制 ${gameDataCount} 个 game_data 文件`);

// ─── 复制 third_party 文件 ──────────────────────────────────
//
// third_party 中的文件（如 model.onnx）被 .gitignore 的 *.onnx 规则排除，
// git ls-files 不会返回它们。因此像 Android 版一样，直接从磁盘复制整个目录。

const thirdPartySrc = join(projectRoot, "data", "third_party");
const thirdPartyDst = join(stagingDir, "third_party");

let thirdPartyCount = 0;
if (existsSync(thirdPartySrc)) {
  copyDirRecursive(thirdPartySrc, thirdPartyDst);
  thirdPartyCount = countFiles(thirdPartyDst);
}
console.log(`✅ 已复制 ${thirdPartyCount} 个 third_party 文件`);

// ─── 生成 data_manifest.json (仅 game_data) ─────────────────

const dataVersion = parseInt(process.env.DATA_VERSION, 10) || 1;

function sha256Hex(filePath) {
  const buf = readFileSync(filePath);
  return createHash("sha256").update(buf).digest("hex");
}

const manifest = {
  data_version: dataVersion,
  files: {},
};

// 遍历 staging dir 的 game_data 生成 checksum
function walkForManifest(baseDir, relDir, files) {
  const dir = join(baseDir, relDir);
  if (!existsSync(dir)) return;
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    const relPath = relDir ? `${relDir}/${entry}` : entry;
    if (statSync(fullPath).isDirectory()) {
      walkForManifest(baseDir, relPath, files);
    } else {
      files[relPath] = {
        sha256: sha256Hex(fullPath),
        size: statSync(fullPath).size,
      };
    }
  }
}

walkForManifest(stagingDir, "game_data", manifest.files);

const sortedKeys = Object.keys(manifest.files).sort();
const sortedFiles = {};
for (const key of sortedKeys) {
  sortedFiles[key] = manifest.files[key];
}
manifest.files = sortedFiles;

// 写入 staging dir（供 Tauri 打包进 .official/）
const manifestPath = join(stagingDir, "data_manifest.json");
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), "utf-8");
console.log(
  `📄 已生成 data_manifest.json (data_version=${dataVersion}, ${Object.keys(manifest.files).length} 个文件)`,
);

// 同时写入项目根目录（供 generate-data-manifest 兼容）
const projectManifestPath = join(projectRoot, "data", "data_manifest.json");
writeFileSync(projectManifestPath, JSON.stringify(manifest, null, 2), "utf-8");

const totalSize = Object.values(manifest.files).reduce((sum, f) => sum + f.size, 0);
const sizeMB = (totalSize / (1024 * 1024)).toFixed(1);
console.log(`   总大小: ${sizeMB} MB`);
console.log("✅ 桌面端资源准备完成");

// ─── 辅助函数 ────────────────────────────────────────────────

function copyDirRecursive(src, dst) {
  mkdirSync(dst, { recursive: true });
  for (const entry of readdirSync(src)) {
    const srcPath = join(src, entry);
    const dstPath = join(dst, entry);
    if (statSync(srcPath).isDirectory()) {
      copyDirRecursive(srcPath, dstPath);
    } else {
      copyFileSync(srcPath, dstPath);
    }
  }
}

function countFiles(dir) {
  let count = 0;
  if (!existsSync(dir)) return count;
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    if (statSync(fullPath).isDirectory()) {
      count += countFiles(fullPath);
    } else {
      count++;
    }
  }
  return count;
}
