// 在 tauri 构建之前，将 git 追踪的 data/ 文件打包为 data.zip
// 直接放入 Android 原生 assets 目录（src-tauri/gen/android/app/src/main/assets/data/）
// .gitignore 决定哪些是默认资源、哪些是用户自定义内容。
// third_party/ 作为例外始终包含（运行时需要的模型文件）。

import { execSync } from 'child_process';
import { existsSync, mkdirSync, copyFileSync, rmSync, readdirSync, statSync, readFileSync, writeFileSync } from 'fs';
import { createHash } from 'crypto';
import { join, dirname, sep } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, '..');
const srcTauri = join(projectRoot, 'src-tauri');

// 临时构建目录
const buildDir = join(srcTauri, '.bundled_build');

// Android assets 目标目录
const androidAssetsDir = join(srcTauri, 'gen', 'android', 'app', 'src', 'main', 'assets', 'data');

// 清理
if (existsSync(buildDir)) {
  rmSync(buildDir, { recursive: true });
}
mkdirSync(buildDir, { recursive: true });

// --- 复制 git 追踪的 data/ 文件 ---
let count = 0;
try {
  const output = execSync('git ls-files -z data/', {
    cwd: projectRoot,
    encoding: 'buffer',
    env: { ...process.env, GIT_CONFIG_PARAMETERS: "'core.quotepath=false'" },
  });

  const files = output.toString('utf8').split('\0').filter(Boolean);
  for (const relative of files) {
    const src = join(projectRoot, relative);
    // 跳过 model.onnx.data (391MB 训练数据)
    if (relative.includes('model.onnx.data')) {
      console.log(`  Skipping ${relative}`);
      continue;
    }
    // 去掉 data/ 前缀
    const rel = relative.replace(/^data[\\/]/, '');
    const dst = join(buildDir, rel);
    mkdirSync(dirname(dst), { recursive: true });
    copyFileSync(src, dst);
    count++;
  }
  console.log(`Bundled ${count} git-tracked files from data/`);
} catch (e) {
  console.warn('git ls-files failed, skipping data bundle:', e.message);
}

// --- 例外：始终包含 data/third_party/ ---
const thirdParty = join(projectRoot, 'data', 'third_party');
if (existsSync(thirdParty)) {
  copyDirRecursive(thirdParty, join(buildDir, 'third_party'));
  console.log('Bundled data/third_party/ (exception)');
}

// --- 生成 data_manifest.json（文件清单 + SHA256） ---
{
  const manifest = { data_version: 1, files: {} };
  walkDir(buildDir, '', manifest);
  const manifestPath = join(buildDir, 'data_manifest.json');
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), 'utf-8');
  console.log(`Generated data_manifest.json with ${Object.keys(manifest.files).length} entries`);
}

// --- 打包为 data.zip（跨平台支持） ---
const zipPath = join(buildDir, 'data.zip');
try {
  if (process.platform === 'win32') {
    // Windows: 使用 PowerShell Compress-Archive
    execSync(
      `powershell -Command "Compress-Archive -Path '${buildDir}\\*' -DestinationPath '${zipPath}' -Force"`,
      { cwd: projectRoot, stdio: 'inherit' }
    );
  } else {
    // Linux / macOS: 使用系统 zip 命令
    execSync(`zip -r "${zipPath}" .`, { cwd: buildDir, stdio: 'inherit' });
  }
  console.log('Created data.zip');
} catch (e) {
  console.error('Failed to create data.zip:', e.message);
  if (process.platform !== 'win32') {
    console.error('Make sure "zip" is installed (e.g., apt-get install zip).');
  }
  process.exit(1);
}

// --- 复制到 Android assets 目录 ---
mkdirSync(androidAssetsDir, { recursive: true });
copyFileSync(zipPath, join(androidAssetsDir, 'data.zip'));
console.log(`Copied data.zip to ${androidAssetsDir}`);

// --- 清理临时目录 ---
rmSync(buildDir, { recursive: true });
console.log('Cleaned up build directory');

// --- 辅助函数 ---

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

function walkDir(base, relDir, manifest) {
  const dir = join(base, relDir);
  for (const entry of readdirSync(dir)) {
    const fullPath = join(dir, entry);
    const relPath = relDir ? `${relDir}${sep}${entry}` : entry;
    if (statSync(fullPath).isDirectory()) {
      walkDir(base, relPath, manifest);
    } else {
      // 不在清单中列 data.zip 自身
      if (relPath === 'data.zip') continue;
      const content = readFileSync(fullPath);
      const sha256 = createHash('sha256').update(content).digest('hex');
      manifest.files[relPath] = { sha256, size: content.length };
    }
  }
}