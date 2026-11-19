// iOS 构建前：下载 Sherpa-ONNX 的 xcframework 到 src-tauri/。
//
// 背景：crates/ling-chat-main 对 iOS 用 sherpa-onnx 的 `shared` 特性，官方只提供
// 「sherpa 动态库 + 静态 ORT」的 xcframework。它必须在 `tauri ios init` **之前**
// 就位：tauri.ios.conf.json 的 bundle.iOS.frameworks 指向它，XcodeGen 生成工程时
// 就要求路径存在（此时还没跑到 cargo build，build.rs 来不及补）。
//
// 落地形态：
//   src-tauri/sherpa-onnx.xcframework/           ← Xcode 按 framework 链接并嵌入
//   src-tauri/.sherpa-onnx-xcframework-version   ← build.rs 用它判断是否需要重新拷贝
//
// 两个文件都是本地产物（gitignore）。已存在且版本一致时直接跳过，不联网。
//
// 用法:
//   node scripts/setup-ios-sherpa.mjs           # 缺什么补什么
//   node scripts/setup-ios-sherpa.mjs --force   # 强制重新下载
//
// 离线构建：把同名 zip 放进 SHERPA_ONNX_ARCHIVE_DIR，脚本会直接用它。

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
  copyFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, "..");
const tauriDir = join(projectRoot, "src-tauri");

// 与 crates/ling-chat-main/Cargo.toml 里 sherpa-onnx 的版本保持一致（升级时一起改）。
const SHERPA_VERSION = "2.0.1";
const ARCHIVE_STEM = `sherpa-onnx-v${SHERPA_VERSION}-ios-shared-onnxruntime-static`;
const ARCHIVE_NAME = `${ARCHIVE_STEM}.xcframework.zip`;
const ARCHIVE_URL = `https://github.com/k2-fsa/sherpa-onnx/releases/download/xcframework/${ARCHIVE_NAME}`;

const destDir = join(tauriDir, "sherpa-onnx.xcframework");
const markerPath = join(tauriDir, ".sherpa-onnx-xcframework-version");
const force = process.argv.includes("--force");

const log = (message) => console.log(`[ios-sherpa] ${message}`);

/** 在解压目录里找 .xcframework（归档可能带一层顶层目录）。 */
function findXcframework(root) {
  if (root.endsWith(".xcframework") && existsSync(root)) return root;
  const stack = [root];
  while (stack.length > 0) {
    const dir = stack.pop();
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      const path = join(dir, entry.name);
      if (entry.name.endsWith(".xcframework")) return path;
      stack.push(path);
    }
  }
  return null;
}

async function downloadArchive(archivePath) {
  const local = process.env.SHERPA_ONNX_ARCHIVE_DIR
    ? join(process.env.SHERPA_ONNX_ARCHIVE_DIR, ARCHIVE_NAME)
    : null;
  if (local && existsSync(local)) {
    log(`使用离线归档: ${local}`);
    copyFileSync(local, archivePath);
    return true;
  }

  log(`下载 xcframework: ${ARCHIVE_URL}`);
  try {
    const response = await fetch(ARCHIVE_URL, {
      redirect: "follow",
      // 归档约 32 MB，网络卡住时别让构建一直挂着。
      signal: AbortSignal.timeout(5 * 60 * 1000),
    });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} ${response.statusText}`);
    }
    writeFileSync(archivePath, Buffer.from(await response.arrayBuffer()));
    return true;
  } catch (error) {
    rmSync(archivePath, { force: true });
    console.error(`[ios-sherpa] 下载失败: ${error.message}`);
    return false;
  }
}

/** 清掉旧的 xcframework / marker，失败时保持原状（先解压到临时目录再替换）。 */
function installXcframework(archivePath) {
  const work = join(tmpdir(), `${ARCHIVE_STEM}-${process.pid}`);
  rmSync(work, { recursive: true, force: true });
  mkdirSync(work, { recursive: true });

  // macOS 自带 unzip；Windows/Linux 也无法构建 iOS，这里不额外兼容。
  const result = spawnSync("unzip", ["-q", "-o", archivePath, "-d", work], {
    stdio: ["ignore", "ignore", "inherit"],
  });
  if (result.error || result.status !== 0) {
    console.error(
      `[ios-sherpa] 解压失败: ${result.error?.message ?? `unzip 退出码 ${result.status}`}`,
    );
    rmSync(work, { recursive: true, force: true });
    return false;
  }

  const extracted = findXcframework(work);
  if (!extracted) {
    console.error(`[ios-sherpa] 归档里没有 .xcframework: ${archivePath}`);
    rmSync(work, { recursive: true, force: true });
    return false;
  }

  rmSync(destDir, { recursive: true, force: true });
  renameSync(extracted, destDir);
  writeFileSync(markerPath, `${ARCHIVE_STEM}\n`);
  rmSync(work, { recursive: true, force: true });
  log(`已就绪: ${destDir}`);
  return true;
}

async function main() {
  // 与 build.rs 的标记一致：版本不变就不重复下载。
  const installed = existsSync(markerPath) ? readFileSync(markerPath, "utf8").trim() : "";
  if (!force && existsSync(destDir) && installed === ARCHIVE_STEM) {
    log("xcframework 已是最新，跳过");
    return 0;
  }

  const archivePath = join(tmpdir(), `${ARCHIVE_STEM}-${process.pid}.zip`);
  if (!(await downloadArchive(archivePath))) {
    console.error(
      [
        "[ios-sherpa] 没有可用的 Sherpa-ONNX iOS xcframework，无法构建 iOS。",
        `  期望归档: ${ARCHIVE_NAME}`,
        "  可设置 SHERPA_ONNX_ARCHIVE_DIR 指向含该归档的目录做离线构建。",
      ].join("\n"),
    );
    return 1;
  }

  const ok = installXcframework(archivePath);
  rmSync(archivePath, { force: true });
  return ok ? 0 : 1;
}

process.exit(await main());
