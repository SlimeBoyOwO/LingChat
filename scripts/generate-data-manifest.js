/**
 * 生成 data_manifest.json — 列出所有默认游戏资源文件及其 SHA-256 校验和。
 *
 * 通过 `git ls-files data/` 获取文件列表，天然尊重 .gitignore：
 *   - .gitignore 中 whitelist 的文件 → git 跟踪 → 收录进清单（由更新器管理）
 *   - .gitignore 中排除的文件（截图/语音/数据库/用户自定义资源等）→ 不在清单中 → 更新时绝不触碰
 *
 * 用法:
 *   node scripts/generate-data-manifest.js [--data-version <N>] [--output <path>]
 *
 * CI 中由 release workflow 调用，产物 data_manifest.json 上传为 GitHub Release asset。
 */

import { createHash } from "node:crypto";
import { execSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";

// ─── 参数解析 ────────────────────────────────────────────────

const args = process.argv.slice(2);
let dataVersion = 1;
let outputPath = "data_manifest.json";

for (let i = 0; i < args.length; i++) {
  if (args[i] === "--data-version" && args[i + 1]) {
    dataVersion = parseInt(args[i + 1], 10);
    i++;
  } else if (args[i] === "--output" && args[i + 1]) {
    outputPath = args[i + 1];
    i++;
  }
}

// ─── 常量 ────────────────────────────────────────────────────

const ROOT = resolve(import.meta.dirname, "..");
const DATA_DIR = join(ROOT, "data");

/** 相对于 data/ 额外排除的路径前缀（即使 git 跟踪也跳过——比如 .gitkeep 占位文件） */
const EXTRA_EXCLUDE_PREFIXES = [
  "game_database",          // SQLite 数据库（运行时数据）
  "screenshots/",           // 截图（用户数据）
  "voice/",                 // TTS 语音（用户数据）
  ".trash/",                // 软删除回收站
  ".gitkeep",               // 占位文件
];

// ─── 主流程 ──────────────────────────────────────────────────

console.log("🔍 通过 git ls-files 获取 data/ 下被跟踪的文件...");

let gitFiles;
try {
  const output = execSync("git -c core.quotepath=false ls-files data/", {
    cwd: ROOT,
    encoding: "utf-8",
  });
  gitFiles = output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
} catch (err) {
  console.error("❌ 无法运行 git ls-files，请确认你在 git 仓库中。");
  process.exit(1);
}

console.log(`   git 跟踪了 ${gitFiles.length} 个文件（整个 data/）`);

// 过滤：只保留 game_data/ 下的非排除文件
const filtered = gitFiles.filter((relPath) => {
  // 必须在 data/ 下
  if (!relPath.startsWith("data/")) return false;

  // 去掉 "data/" 前缀得到相对 data/ 的路径
  const subPath = relPath.slice("data/".length);

  // 额外排除运行时/用户目录
  for (const prefix of EXTRA_EXCLUDE_PREFIXES) {
    if (subPath === prefix || subPath.startsWith(prefix)) {
      return false;
    }
  }

  return true;
});

// 映射为 "相对 data/ 的路径" → 绝对路径
const fileMap = new Map(); // subPath → absPath
for (const relPath of filtered) {
  const subPath = relPath.slice("data/".length);
  fileMap.set(subPath, join(ROOT, relPath));
}

if (fileMap.size === 0) {
  console.warn("⚠ 未找到任何默认资源文件！将生成空清单。");
}

// 排序保证输出稳定
const sortedPaths = [...fileMap.keys()].sort();

console.log(`   收录 ${sortedPaths.length} 个默认资源文件`);

// ─── 计算 SHA-256 ───────────────────────────────────────────

function sha256Hex(filePath) {
  const buf = readFileSync(filePath);
  return createHash("sha256").update(buf).digest("hex");
}

const manifest = {
  data_version: dataVersion,
  files: {},
};

for (const subPath of sortedPaths) {
  const absPath = fileMap.get(subPath);
  const fileStat = statSync(absPath);

  manifest.files[subPath] = {
    sha256: sha256Hex(absPath),
    size: fileStat.size,
  };
}

// ─── 写入输出 ────────────────────────────────────────────────

writeFileSync(outputPath, JSON.stringify(manifest, null, 2), "utf-8");
console.log(
  `✅ 已生成 ${outputPath} (data_version=${dataVersion}, ${Object.keys(manifest.files).length} 个文件)`,
);

const totalSize = Object.values(manifest.files).reduce((sum, f) => sum + f.size, 0);
const sizeMB = (totalSize / (1024 * 1024)).toFixed(1);
console.log(`   总大小: ${sizeMB} MB`);
