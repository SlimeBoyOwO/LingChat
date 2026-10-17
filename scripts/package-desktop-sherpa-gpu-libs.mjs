// 桌面端 GPU 版打包前：把 CUDA 版 Sherpa-ONNX 运行库放到安装包里的正确位置。
//
// 背景：用 `SHERPA_ONNX_GPU=cuda12|cuda13` 构建时（见
// src-tauri/patches/sherpa-onnx-sys/build.rs），sherpa-onnx 走 **shared** 链接，
// 官方 CUDA 包只提供动态库，于是：
//   - 可执行文件 `DT_NEEDED: libsherpa-onnx-c-api.so` / `onnxruntime.dll`；
//   - CUDA 执行提供方（`libonnxruntime_providers_cuda.so` /
//     `onnxruntime_providers_cuda.dll`）由 ONNX Runtime 在运行期从
//     libonnxruntime 所在目录加载。
// build.rs 只把这些库拷到 `target/<profile>/` 供开发机使用，`tauri build` 打的
// 安装包不会带上它们 —— 装完启动就会因为找不到 libonnxruntime.so 直接失败。
//
// 所以本脚本：
//   1. 找到 CUDA 预编译包解压出来的 lib 目录（与 build.rs 的解析顺序一致）；
//   2. 把运行库拷到 src-tauri/.sherpa-gpu-libs/（.gitignore 忽略，属本地产物）；
//   3. 生成 src-tauri/tauri.sherpa-gpu.conf.json，把库塞到安装包的**默认库搜索目录**
//      `/usr/lib`（AppImage 里即 AppDir 的 usr/lib，AppRun 会把它加进
//      LD_LIBRARY_PATH）：
//        - Linux：deb/rpm/appimage 的 files 映射到 /usr/lib。**不能**放可执行文件
//          旁边：Cargo 里依赖 crate 的 build script 用 `cargo:rustc-link-arg` 加
//          `-Wl,-rpath,$ORIGIN` 对最终二进制无效（Cargo 只把它用在「该包自己的」
//          target 上），实测 cargo 出来的可执行文件里根本没有 RPATH/RUNPATH，
//          Linux 的动态加载器不会搜可执行文件所在目录，装完必然启动失败。
//        - Windows：resource 目录即安装目录（exe 旁边），Windows 会搜 exe 目录，
//          NSIS 装完直接可用。
//
// 与 package.json 的 `tauri:build:gpu` 配合使用（跨平台，不依赖 POSIX 的
// `VAR=... cmd` 语法，Windows 上也能直接跑）：
//   pnpm tauri:build:gpu                 # = node scripts/... --variant cuda12 --build
//   node scripts/package-desktop-sherpa-gpu-libs.mjs --variant cuda13   # 只暂存+生成配置
//
// 注意：本脚本只负责「库跟着安装包走」，CUDA 运行时（cudart/cublas/cudnn 等）
// 仍需目标机器自行安装 —— 缺了它 adapters/sherpa_onnx.rs 会探测到 provider 库
// 加载失败并回退 CPU（ORT 1.27 加载失败会 abort 进程，所以必须探测）。

import {
  existsSync,
  mkdirSync,
  copyFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "fs";
import { join, dirname, basename } from "path";
import { fileURLToPath } from "url";
import { spawnSync } from "child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, "..");
const tauriDir = join(projectRoot, "src-tauri");
const stageDir = join(tauriDir, ".sherpa-gpu-libs");
const configPath = join(tauriDir, "tauri.sherpa-gpu.conf.json");
const prebuiltRoot = join(
  process.env.CARGO_TARGET_DIR || join(projectRoot, "target"),
  "sherpa-onnx-prebuilt",
);

const args = process.argv.slice(2);
const variantArg = args.find((a) => a.startsWith("--variant="))?.slice("--variant=".length);
if (variantArg) process.env.SHERPA_ONNX_GPU = variantArg;
const shouldBuild = args.includes("--build");

const variant = (process.env.SHERPA_ONNX_GPU || "").trim().toLowerCase();
const isWindows = process.platform === "win32";

// 运行时必需：c-api 与 libonnxruntime 是链接期就写进 DT_NEEDED 的；providers_cuda
// 是运行时才加载的执行提供方（缺它 GPU 用不了，脚本据此确认拿到的是 CUDA 包）。
const requiredLibs = isWindows
  ? ["sherpa-onnx-c-api.dll", "onnxruntime.dll", "onnxruntime_providers_cuda.dll"]
  : ["libsherpa-onnx-c-api.so", "libonnxruntime.so", "libonnxruntime_providers_cuda.so"];

const libPattern = isWindows ? /\.dll$/i : /\.so(\..+)?$/;
// Windows 包里带了命令行工具用的 cargs.dll，与应用无关。
const skipLibs = new Set(["cargs.dll"]);

function listLibs(dir) {
  if (!existsSync(dir)) return [];
  try {
    return readdirSync(dir).filter((f) => libPattern.test(f) && !skipLibs.has(f.toLowerCase()));
  } catch {
    return [];
  }
}

/** 在 target/sherpa-onnx-prebuilt 下递归找含必需库的目录（优先路径里带 cuda 的）。 */
function findCacheDirs(root) {
  const found = [];
  if (!existsSync(root)) return found;
  const stack = [root];
  while (stack.length) {
    const cur = stack.pop();
    let entries;
    try {
      entries = readdirSync(cur, { withFileTypes: true });
    } catch {
      continue;
    }
    const libs = new Set(listLibs(cur));
    if (requiredLibs.every((r) => libs.has(r))) found.push(cur);
    for (const e of entries) {
      if (e.isDirectory()) stack.push(join(cur, e.name));
    }
  }
  found.sort((a, b) => {
    const aHit = a.toLowerCase().includes("cuda") ? 0 : 1;
    const bHit = b.toLowerCase().includes("cuda") ? 0 : 1;
    if (aHit !== bHit) return aHit - bHit;
    return a.length - b.length;
  });
  return found;
}

function pickSource() {
  const fromEnv = process.env.SHERPA_ONNX_LIB_DIR;
  if (fromEnv) {
    const libs = listLibs(fromEnv);
    if (requiredLibs.every((r) => libs.includes(r))) return fromEnv;
    console.warn(`[sherpa-gpu] SHERPA_ONNX_LIB_DIR 存在但缺少必需库，忽略并回落到缓存: ${fromEnv}`);
  }
  return findCacheDirs(prebuiltRoot)[0] || null;
}

if (!["cuda12", "cuda13"].includes(variant)) {
  console.error(
    [
      "[sherpa-gpu] 请先设置 SHERPA_ONNX_GPU=cuda12 或 cuda13。",
      "  该变量同时决定 sherpa-onnx-sys/build.rs 下载哪个预编译包；",
      "  这里只是把已下载的 CUDA 包里的运行库塞进安装包。",
      "  官方 CUDA 包只有 linux-x64 / win-x64，macOS 无 GPU 包。",
    ].join("\n"),
  );
  process.exit(1);
}

if (process.platform === "darwin") {
  console.error("[sherpa-gpu] macOS 没有官方 CUDA 包，桌面 GPU 打包只支持 Linux / Windows。");
  process.exit(1);
}

const source = pickSource();
if (!source) {
  console.error(
    [
      "[sherpa-gpu] 找不到 CUDA 版 Sherpa-ONNX 运行库。",
      `  缺少: ${requiredLibs.join(", ")}`,
      `  已查找: SHERPA_ONNX_LIB_DIR=${process.env.SHERPA_ONNX_LIB_DIR || "(未设置)"}`,
      `          ${prebuiltRoot}`,
      "  先跑一次带 SHERPA_ONNX_GPU 的构建（例如",
      `    SHERPA_ONNX_GPU=${variant} cargo build -p ling-chat-main）`,
      "  让它把官方 CUDA 包下载解压到 target/sherpa-onnx-prebuilt/，或直接把",
      "  SHERPA_ONNX_LIB_DIR 指向已解压包的 lib/ 目录。",
    ].join("\n"),
  );
  process.exit(1);
}

// 暂存目录每次重建，避免上一次（比如换了 CUDA 12 → 13）的旧库残留进安装包。
rmSync(stageDir, { recursive: true, force: true });
mkdirSync(stageDir, { recursive: true });

const libs = listLibs(source);
let copied = 0;
for (const lib of libs) {
  const src = join(source, lib);
  // 官方包里 libonnxruntime.so 可能是指向带版本号文件的符号链接，一并解决。
  const info = statSync(src);
  if (!info.isFile()) continue;
  copyFileSync(src, join(stageDir, basename(lib)));
  copied++;
}

const staged = listLibs(stageDir);
const missing = requiredLibs.filter((r) => !staged.includes(r));
if (missing.length) {
  console.error(`[sherpa-gpu] 暂存后仍缺少必需库: ${missing.join(", ")}（来源: ${source}）`);
  process.exit(1);
}

// Windows：resource 目录就是安装目录（exe 旁边，Windows 会搜 exe 目录）；
// Linux：装到 /usr/lib（glibc 默认搜索目录之一）。实测 cargo 产出的可执行文件里
// 没有 RPATH/RUNPATH，所以「放可执行文件旁边」这条路在 Linux 上走不通。
const config = isWindows
  ? {
      bundle: {
        resources: { ".sherpa-gpu-libs/": "sherpa-gpu-libs/" },
      },
    }
  : {
      bundle: {
        linux: {
          deb: { files: { "/usr/lib": ".sherpa-gpu-libs/" } },
          rpm: { files: { "/usr/lib": ".sherpa-gpu-libs/" } },
          appimage: { files: { "/usr/lib": ".sherpa-gpu-libs/" } },
        },
      },
    };

writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`);

console.log(
  [
    `[sherpa-gpu] variant=${variant} 来源=${source}`,
    `[sherpa-gpu] ${copied} 个运行库 -> ${stageDir}`,
    `[sherpa-gpu] 已写出 ${configPath}`,
    "[sherpa-gpu] 目标机器仍需自备 NVIDIA 驱动 + CUDA 12.8+/13 + cuDNN 9，",
    "            否则应用会探测到 provider 加载失败并回退 CPU。",
  ].join("\n"),
);

if (shouldBuild) {
  // --config 用绝对路径，避开 CLI「相对当前目录还是相对 src-tauri」的歧义。
  console.log(`[sherpa-gpu] 开始 tauri build（SHERPA_ONNX_GPU=${variant}）…`);
  const result = spawnSync("pnpm", ["exec", "tauri", "build", "--config", configPath], {
    cwd: projectRoot,
    stdio: "inherit",
    env: process.env,
    shell: isWindows,
  });
  process.exit(result.status ?? 1);
} else {
  console.log(
    [
      "[sherpa-gpu] 打包命令：",
      `  SHERPA_ONNX_GPU=${variant} pnpm tauri build --config src-tauri/tauri.sherpa-gpu.conf.json`,
      "  （Windows 可用 `pnpm tauri:build:gpu`，无需手写环境变量）",
    ].join("\n"),
  );
}
