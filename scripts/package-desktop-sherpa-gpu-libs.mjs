// 桌面端 CUDA 版（sherpa-onnx GPU）打包入口。
//
// 用法（跨平台，不依赖 POSIX 的 `VAR=... cmd` 语法）：
//   pnpm tauri:build:gpu                     # = node scripts/... --variant=cuda12 --build
//   node scripts/package-desktop-sherpa-gpu-libs.mjs --variant cuda13   # 只做准备、打印命令
//
// 背景：`SHERPA_ONNX_GPU=cuda12|cuda13` 时（见 src-tauri/patches/sherpa-onnx-sys/build.rs）
// sherpa-onnx 走 **shared** 链接，官方 CUDA 包只提供动态库：
//   - 可执行文件 `DT_NEEDED: libsherpa-onnx-c-api.so` / `onnxruntime.dll`；
//   - CUDA 执行提供方（`libonnxruntime_providers_cuda.so` / `.dll`）由 ONNX Runtime
//     在运行期从 libonnxruntime 所在目录加载。
// build.rs 只把这些库拷到 `target/<profile>/` 供开发机使用，`tauri build` 打的安装包
// 不会自动带上它们 —— 装完启动就会因为找不到 libonnxruntime.so 直接失败。
//
// 运行库的暂存与落点由 scripts/stage-desktop-runtime-libs.mjs 统一负责（它同时挂在
// tauri.conf.json 的 build.beforeBundleCommand 上，CPU 构建也跑）：
//   - Linux：tauri.conf.json 里 bundle.linux.*.files 已经把 .runtime-libs/ 映射到
//     /usr/lib，所以这里不需要任何配置覆盖。
//   - Windows：资源根目录就是安装目录（exe 旁边；与 tauri.windows.conf.json 里
//     onnxruntime.dll 的映射方式一致），逐文件映射由本脚本生成 --config 覆盖。
//   - macOS：没有官方 CUDA 包，直接拒绝。
//
// 注意：本流程只保证「库跟着安装包走」。CUDA 运行时（cudart/cublas/cudnn 等）仍需
// 目标机器自备 —— 缺了它 adapters/sherpa_onnx.rs 会探测到 provider 库加载失败并回退
// CPU（ORT 1.27 加载失败会 abort 进程，所以必须先探测）。

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

import {
  GPU_VARIANTS,
  listLibs,
  projectRoot,
  resolveSherpaSource,
  resolveVariant,
  stageDir,
  stageRuntimeLibs,
  tauriDir,
} from "./stage-desktop-runtime-libs.mjs";

const configPath = join(tauriDir, "tauri.sherpa-gpu.conf.json");
const windowsConfigPath = join(tauriDir, "tauri.windows.conf.json");

const args = process.argv.slice(2);
let variantArg;
const eqForm = args.find((a) => a.startsWith("--variant="));
if (eqForm) {
  variantArg = eqForm.slice("--variant=".length);
} else {
  const flagIndex = args.indexOf("--variant");
  if (flagIndex !== -1) {
    variantArg = args[flagIndex + 1];
    if (!variantArg || variantArg.startsWith("--")) {
      console.error("[sherpa-gpu] --variant 后面要跟 cuda12 或 cuda13");
      process.exit(1);
    }
  }
}
if (variantArg) process.env.SHERPA_ONNX_GPU = variantArg;
const shouldBuild = args.includes("--build");
const isWindows = process.platform === "win32";

let variant;
try {
  variant = resolveVariant();
} catch (error) {
  console.error(`[sherpa-gpu] ${error.message ?? error}`);
  process.exit(1);
}

if (!GPU_VARIANTS.includes(variant)) {
  console.error(
    [
      `[sherpa-gpu] 需要 CUDA 变体，当前是 ${variant}。请用 SHERPA_ONNX_GPU=cuda12|cuda13`,
      "  （该变量同时决定 sherpa-onnx-sys/build.rs 下载哪个预编译包）。",
      "  官方 CUDA 包只有 linux-x64 / win-x64，macOS 无 GPU 包。",
    ].join("\n"),
  );
  process.exit(1);
}

if (process.platform === "darwin") {
  console.error("[sherpa-gpu] macOS 没有官方 CUDA 包，桌面 GPU 打包只支持 Linux / Windows。");
  process.exit(1);
}

// 尽早失败：别等 cargo 编完才发现 CUDA 包还没下载/解压。
try {
  const source = resolveSherpaSource(variant);
  console.log(`[sherpa-gpu] variant=${variant}，CUDA 运行库来源: ${source}`);
} catch (error) {
  console.error(`[sherpa-gpu] ${error.message ?? error}`);
  process.exit(1);
}

/**
 * Windows：生成 --config 覆盖，把 CUDA DLL 逐文件映射到资源根目录（= 安装目录）。
 *
 * 逐文件而不是整目录，是因为 Windows 只在 exe 同目录（以及 PATH）里找 DLL，
 * 放进 `sherpa-gpu-libs/` 这种子目录等于没放；ort 的 `load-dynamic` 也是先看
 * exe 同目录、再看 resource_dir（见 utils/onnx.rs）。
 *
 * `bundle.resources` 同时带上 tauri.windows.conf.json 里已有的条目：Tauri 对
 * `--config` 的合并语义是「同名 key 覆盖、其余保留」，把已有条目原样带上就不会
 * 因为合并方式不同而丢掉 onnxruntime.dll / DirectML.dll 的映射。
 */
function writeWindowsConfig() {
  const { staged } = stageRuntimeLibs({ variant });
  if (!staged.length) {
    console.error("[sherpa-gpu] Windows CUDA 变体没有暂存到任何 DLL，检查 CUDA 包内容");
    process.exit(1);
  }
  const required = ["sherpa-onnx-c-api.dll", "onnxruntime.dll", "onnxruntime_providers_cuda.dll"];
  const missing = required.filter((r) => !staged.includes(r));
  if (missing.length) {
    console.error(`[sherpa-gpu] 暂存后仍缺少必需库: ${missing.join(", ")}`);
    process.exit(1);
  }

  let baseResources = {};
  if (existsSync(windowsConfigPath)) {
    try {
      baseResources = JSON.parse(readFileSync(windowsConfigPath, "utf-8")).bundle?.resources ?? {};
    } catch (error) {
      console.error(`[sherpa-gpu] 读取 ${windowsConfigPath} 失败: ${error.message ?? error}`);
      process.exit(1);
    }
  }

  const resources = { ...baseResources };
  for (const dll of listLibs(stageDir)) {
    resources[`.runtime-libs/${dll}`] = dll;
  }
  writeFileSync(configPath, `${JSON.stringify({ bundle: { resources } }, null, 2)}\n`);
  console.log(
    [
      `[sherpa-gpu] ${staged.length} 个 DLL -> ${stageDir}`,
      `[sherpa-gpu] 已写出 ${configPath}（映射到资源根目录）`,
    ].join("\n"),
  );
  return configPath;
}

const overrideConfig = isWindows ? writeWindowsConfig() : null;
if (!isWindows) {
  console.log(
    "[sherpa-gpu] Linux：tauri.conf.json 已把 .runtime-libs/ 映射到 /usr/lib，无需配置覆盖",
  );
}

console.log(
  [
    "[sherpa-gpu] 目标机器仍需自备 NVIDIA 驱动 + CUDA 12.8+ / 13 + cuDNN 9，",
    "            否则应用会探测到 provider 加载失败并回退 CPU。",
  ].join("\n"),
);

if (shouldBuild) {
  const tauriArgs = ["exec", "tauri", "build"];
  if (overrideConfig) {
    // 用绝对路径，避开 CLI「相对当前目录还是相对 src-tauri」的歧义。
    tauriArgs.push("--config", overrideConfig);
  }
  console.log(`[sherpa-gpu] 开始 tauri build（SHERPA_ONNX_GPU=${variant}）…`);
  const result = spawnSync("pnpm", tauriArgs, {
    cwd: projectRoot,
    stdio: "inherit",
    env: process.env,
    shell: isWindows,
  });
  process.exit(result.status ?? 1);
}

console.log(
  [
    "[sherpa-gpu] 打包命令：",
    `  SHERPA_ONNX_GPU=${variant} pnpm tauri build` +
      (overrideConfig ? ` --config ${overrideConfig}` : ""),
    "  （Windows 可用 `pnpm tauri:build:gpu`，无需手写环境变量）",
  ].join("\n"),
);
