// 打包前把 ort 的 WebGPU 运行时库暂存到 src-tauri/.runtime-libs/，
// 由 tauri.conf.json 的 bundle.linux.{deb,rpm,appimage}.files 映射到 /usr/lib。
//
// 背景：Linux x86_64 的 ort 启用了 `webgpu` + `download-binaries`，可执行文件的
// DT_NEEDED 里有 libwebgpu_dawn.so（Dawn 只有动态库，无法静态链接）。该库由 ort 的
// copy-dylibs 落在 target/<profile>/，而 tauri-bundler 默认不会把它打进安装包；
// 可执行文件又没有 RPATH/RUNPATH，所以必须放到加载器的默认搜索目录 /usr/lib，
// 否则 deb 装完一启动就是 `error while loading shared libraries: libwebgpu_dawn.so`。

import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const targetRoot = process.env.CARGO_TARGET_DIR || join(projectRoot, "target");
const stageDir = join(projectRoot, "src-tauri", ".runtime-libs");
const libName = "libwebgpu_dawn.so";

function findLib() {
  const profile =
    process.env.TAURI_ENV_PROFILE ||
    (process.env.TAURI_ENV_DEBUG === "true" ? "debug" : "release");
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
  const candidates = [
    triple ? join(targetRoot, triple, profile, libName) : null,
    join(targetRoot, profile, libName),
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

function main() {
  // 移动端与 macOS/Windows 的打包不使用 bundle.linux 的映射，直接跳过。
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
  const platform = process.env.TAURI_ENV_PLATFORM || "";
  if (process.platform !== "linux" || triple.includes("android") || platform === "android") {
    return;
  }

  // 目录必须先建出来：tauri-bundler 对不存在的源目录会直接报错；
  // 空目录等于不带任何文件（非 x86_64 的 Linux 构建没有 Dawn）。
  rmSync(stageDir, { recursive: true, force: true });
  mkdirSync(stageDir, { recursive: true });

  const lib = findLib();
  if (!lib) {
    console.log(`[stage-libs] 未找到 ${libName}（非 x86_64 Linux 构建？），跳过`);
    return;
  }
  copyFileSync(lib, join(stageDir, libName));
  console.log(`[stage-libs] 已暂存 ${libName} -> ${stageDir}`);
}

main();
