/**
 * 从本地构建产物生成 latest.json（供手动 GitHub Release 使用）。
 *
 * 用法:
 *   node scripts/generate-latest-json.mjs <version> [notes]
 *
 * 示例:
 *   node scripts/generate-latest-json.mjs 0.4.7
 *   node scripts/generate-latest-json.mjs 0.4.7 "修复了若干bug"
 *
 * 输出: src-tauri/target/release/bundle/latest.json
 */

import { readFileSync, writeFileSync, existsSync, readdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, "..");

const version = process.argv[2];
const notes = process.argv[3] || "";

if (!version) {
  console.error("用法: node scripts/generate-latest-json.mjs <version> [notes]");
  process.exit(1);
}

const bundleDir = join(
  projectRoot,
  "src-tauri",
  "target",
  "release",
  "bundle",
  "nsis",
);

// 查找安装包和签名文件
const files = existsSync(bundleDir) ? readdirRecursive(bundleDir) : [];
const exe = files.find((f) => f.endsWith("_x64-setup.exe"));
const sig = files.find((f) => f.endsWith("_x64-setup.exe.sig"));

if (!exe) {
  console.error("❌ 找不到安装包 .exe，请先执行 pnpm tauri build");
  process.exit(1);
}
if (!sig) {
  console.error("❌ 找不到签名文件 .sig，请确认 tauri.conf.json 中 createUpdaterArtifacts 为 true");
  process.exit(1);
}

const sigContent = readFileSync(join(bundleDir, sig), "utf-8").trim();

const latestJson = {
  version,
  notes: notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: sigContent,
      url: `https://github.com/SlimeBoyOwO/LingChat/releases/download/v${version}/${exe}`,
    },
  },
};

const outPath = join(bundleDir, "..", "latest.json");
writeFileSync(outPath, JSON.stringify(latestJson, null, 2), "utf-8");
console.log(`✅ latest.json 已生成: ${outPath}`);
console.log(`   version: ${latestJson.version}`);
console.log(`   url: ${latestJson.platforms["windows-x86_64"].url}`);
console.log("");
console.log("📋 上传到 GitHub Release 时需要这 3 个文件:");
console.log(`   1. ${exe}`);
console.log(`   2. ${sig}`);
console.log(`   3. latest.json`);

function readdirRecursive(dir) {
  const results = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      results.push(...readdirRecursive(full));
    } else {
      results.push(entry);
    }
  }
  return results;
}
