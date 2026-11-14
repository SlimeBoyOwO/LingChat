// Windows 桌面专用：**构建时**自编一份支持 DirectML 的 sherpa-onnx，复用应用已经
// 随包的那份 onnxruntime.dll（情绪模型/嵌入模型走的那套），从而拿到 GPU，而不用背
// 官方 CUDA 包（win-x64 375 MB）那一坨。
//
// 为什么要自编：
//   - 官方预编译的 Windows sherpa **没开 DirectML**：CMake 里
//     `option(SHERPA_ONNX_ENABLE_DIRECTML ... OFF)` 默认关，上游 windows-x64.yaml /
//     build-wheels-win-x64.yaml 也都没传这个开关。于是运行期传 `dml` 只会打印
//     "DirectML is for Windows only. Fallback to cpu!" 然后静默回落 CPU（不报错）。
//   - 但应用**已经**有带 DML EP 的 onnxruntime.dll（WinML 2.4.89 内含 ORT 1.27.1）
//     和 DirectML.dll，sherpa 缺的只是「编译期把 DML 那段代码编进去」。
//
// 本脚本就是「在编译的时候下载源代码并修改」的落点（cargo 的 build.rs 做不到这件事：
// build script 只能把信息传给依赖它的包，**没法把自己的环境变量注入给依赖 crate 的
// build.rs** —— 而 sherpa-onnx-sys 只认 SHERPA_ONNX_LIB_DIR，所以准备动作必须在
// cargo 之前，由本脚本导出环境变量）：
//   1. 按 Cargo.lock 里 sherpa-onnx-sys 的锁定版本，下源码 tarball 到 target/ 缓存；
//   2. 解压后按 patches/sherpa-onnx/*.patch 顺序打补丁 —— 仓库里只留 .patch，不留改过的
//      源码树。DirectML 本身只要 cmake 开关、不需要改 C++，所以那个目录现在是空的；
//      将来要动 sherpa 的 C++ 时把 .patch 丢进去即可（补丁内容变化会自动让缓存失效）。
//   3. cmake 自编：-DSHERPA_ONNX_ENABLE_DIRECTML=ON -DBUILD_SHARED_LIBS=ON，
//      ORT 走「预装」分支指向 WinML 包抽出的头文件与 import lib（见
//      scripts/ensure-onnxruntime.mjs 的 winml-ort/），产物汇总成一个 lib 目录，
//      并写 sherpa-build.json 标记「这份是 DirectML 版」（build.rs 据此发射 provider）。
//
// 运行期不需要任何额外库：exe 旁边那份 onnxruntime.dll / DirectML.dll 就是应用本来
// 随包的那两个（见 src-tauri/tauri.windows.conf.json）。
//
// 用法：
//   pnpm tauri:build:win                                        # 准备 + tauri build
//   node scripts/build-desktop-sherpa-dml.mjs --print-lib-dir   # 只把库目录打到 stdout
//   node scripts/build-desktop-sherpa-dml.mjs --force           # 忽略缓存重新下/编
//
// 可调环境变量：SHERPA_BUILD_JOBS（编译线程数，默认 = 机器核数；也认
// CMAKE_BUILD_PARALLEL_LEVEL）、SHERPA_ONNX_DOWNLOAD_MIRROR（GitHub 下载镜像）。
//
// 非 Windows 平台直接跳过（退出码 0、stdout 无输出），所以可以安全地挂在跨平台入口。
// 需要：Visual Studio（MSVC + Windows 10 SDK，dml_provider_factory.h 要 d3d12.h）、
// cmake、网络。首次构建约十几分钟（sherpa 全量编译），之后走缓存。

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { cpus } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { stageDir, stageRuntimeLibs } from "./stage-desktop-runtime-libs.mjs";
import {
  download,
  ensureSource,
  parallelArgs,
  patchFingerprint,
  resolveBuildJobs,
  resolveSherpaVersion,
  run,
} from "./lib/sherpa-source.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const tauriDir = join(projectRoot, "src-tauri");
const binariesDir = join(tauriDir, "binaries");
const winmlOrtDir = join(binariesDir, "winml-ort");
const cacheRoot = join(projectRoot, "target", "sherpa-onnx-directml");
const patchesDir = join(projectRoot, "patches", "sherpa-onnx");
// Windows 的 --config 覆盖：把本次编出来的 DLL 逐文件映射到资源根目录（= 安装目录）。
const overrideConfigPath = join(tauriDir, "tauri.sherpa-dml.conf.json");

/// 与 sherpa 的 cmake/onnxruntime-win-x64-directml.cmake 保持一致：DirectML 包的
/// 版本和 sha256 写在它的 URL_HASH 里。预下到 build 目录可以让 cmake 直接用本地包
/// （URL_HASH 仍然会校验），绕开 nuget.org 在部分网络下的超时。
/// 源码下载 / 补丁 / 缓存状态见 scripts/lib/sherpa-source.mjs（与 Android NNAPI 版共用）。
const DIRECTML_VERSION = "1.15.0";
const DIRECTML_URL = `https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/${DIRECTML_VERSION}`;
const DIRECTML_LOCAL_NAME = `Microsoft.AI.DirectML.${DIRECTML_VERSION}.nupkg`;
const DIRECTML_SHA256 = "10d175f8e97447712b3680e3ac020bbb8eafdf651332b48f09ffee2eec801c23";

const args = process.argv.slice(2);
const printLibDirOnly = args.includes("--print-lib-dir");
const force = args.includes("--force");
const shouldBuild = args.includes("--build");

/// `--print-lib-dir` 模式下 stdout 只承载「库目录」这一条信息，子进程默认 `inherit`
/// 会把 cmake 的编译/安装输出一并写进 stdout，污染上层的 `$(...)` 解析（Android 那份
/// 脚本就在 CI 首次冷构建时踩过这个坑）。打印模式下一律把子进程的 stdout/stderr 并到
/// 本进程的 stderr（fd 2）。
const childStdio = printLibDirOnly ? ["ignore", 2, 2] : "inherit";

/// 诊断一律走 stderr：`--print-lib-dir` 模式下 stdout 只留机器可读的目录。
function log(msg) {
  console.error(`[sherpa-dml] ${msg}`);
}

function fail(msg) {
  console.error(`[sherpa-dml] ${msg}`);
  process.exit(1);
}

/** 非 Windows 直接跳过（退出码 0、无输出），让跨平台入口可以无条件调用。 */
if (process.platform !== "win32") {
  log(`非 Windows 平台（${process.platform}），跳过：DirectML 只有 Windows 有`);
  process.exit(0);
}

/// 自编需要的头文件与 import lib 由 ensure-onnxruntime.mjs 从 WinML 包里抽出，
/// 缺了就先跑它（幂等，已存在时几乎零开销）。
function ensureOrtBuildDeps() {
  const markers = [
    join(winmlOrtDir, "include", "onnxruntime_cxx_api.h"),
    join(winmlOrtDir, "include", "dml_provider_factory.h"),
    join(winmlOrtDir, "lib", "onnxruntime.lib"),
  ];
  if (markers.every((m) => existsSync(m))) {
    log(`ORT 头文件/import lib 已就绪: ${winmlOrtDir}`);
    return;
  }
  log("winml-ort/ 不全，先跑 scripts/ensure-onnxruntime.mjs");
  run(process.execPath, [join(__dirname, "ensure-onnxruntime.mjs")], {
    cwd: projectRoot,
    stdio: childStdio,
  });
  const missing = markers.filter((m) => !existsSync(m));
  if (missing.length > 0) {
    throw new Error(`ensure-onnxruntime 跑完仍缺少：${missing.join(", ")}`);
  }
}

/// 组一个「预装 ORT」目录交给 cmake：import lib + 运行时 dll（后者是给 sherpa 的
/// `file(GLOB $LIB_DIR/*.dll)` 用的，省得 install 列表为空；内容与应用随包的同一份）。
function composeOrtLibDir() {
  const dir = join(cacheRoot, "ort-libs");
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  copyFileSync(join(winmlOrtDir, "lib", "onnxruntime.lib"), join(dir, "onnxruntime.lib"));
  for (const dll of ["onnxruntime.dll", "DirectML.dll"]) {
    const src = join(binariesDir, dll);
    if (existsSync(src)) copyFileSync(src, join(dir, dll));
  }
  return dir;
}

/// 预下 DirectML 包到 build 目录：cmake 会在 ${PROJECT_BINARY_DIR} 找它（见
/// onnxruntime-win-x64-directml.cmake 的 possible_directml_file_locations）。
async function preseedDirectML(buildDir) {
  const dest = join(buildDir, DIRECTML_LOCAL_NAME);
  if (existsSync(dest)) {
    const hash = createHash("sha256").update(readFileSync(dest)).digest("hex");
    if (hash === DIRECTML_SHA256) {
      log(`DirectML 包已在缓存: ${dest}`);
      return;
    }
    rmSync(dest, { force: true });
  }
  log(`⬇️  DirectML ${DIRECTML_VERSION}（含 d3d12 侧头文件与 DirectML.lib）`);
  await download(DIRECTML_URL, dest);
  const hash = createHash("sha256").update(readFileSync(dest)).digest("hex");
  if (hash !== DIRECTML_SHA256) {
    rmSync(dest, { force: true });
    throw new Error(`DirectML 包 sha256 不匹配：期望 ${DIRECTML_SHA256}，实际 ${hash}`);
  }
}

/// cmake 配置 + 编译。用 Visual Studio 生成器：sherpa 的 directml cmake 文件硬性要求
/// `CMAKE_VS_PLATFORM_NAME=X64`（Ninja 下这个变量是空的，会直接 FATAL_ERROR）。
async function buildSherpa({ version, srcDir }) {
  const buildDir = join(cacheRoot, `build-v${version}`);
  const outDir = join(cacheRoot, `libs-v${version}`);
  mkdirSync(buildDir, { recursive: true });

  await preseedDirectML(buildDir);

  const ortLibDir = composeOrtLibDir();
  const cmakeEnv = {
    ...process.env,
    SHERPA_ONNXRUNTIME_INCLUDE_DIR: join(winmlOrtDir, "include"),
    SHERPA_ONNXRUNTIME_LIB_DIR: ortLibDir,
  };
  log(`SHERPA_ONNXRUNTIME_INCLUDE_DIR=${cmakeEnv.SHERPA_ONNXRUNTIME_INCLUDE_DIR}`);
  log(`SHERPA_ONNXRUNTIME_LIB_DIR=${cmakeEnv.SHERPA_ONNXRUNTIME_LIB_DIR}`);

  const configureArgs = [
    "-S",
    srcDir,
    "-B",
    buildDir,
    "-A",
    "x64",
    "-DCMAKE_BUILD_TYPE=Release",
    // 上游 sherpa-onnx-sys 的 shared 路径就是 `dylib=sherpa-onnx-c-api`，所以必须
    // BUILD_SHARED_LIBS=ON；DIRECTML 开关则会强制打开它（见 CMakeLists）。
    "-DBUILD_SHARED_LIBS=ON",
    "-DSHERPA_ONNX_ENABLE_DIRECTML=ON",
    "-DSHERPA_ONNX_ENABLE_C_API=ON",
    "-DSHERPA_ONNX_ENABLE_TTS=ON",
    "-DSHERPA_ONNX_ENABLE_PYTHON=OFF",
    "-DSHERPA_ONNX_ENABLE_TESTS=OFF",
    "-DSHERPA_ONNX_ENABLE_BINARY=OFF",
    "-DSHERPA_ONNX_ENABLE_JNI=OFF",
    "-DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF",
    "-DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF",
  ];
  if (force || !existsSync(join(buildDir, "CMakeCache.txt"))) {
    log("cmake 配置…");
    run("cmake", configureArgs, { env: cmakeEnv, stdio: childStdio });
  } else {
    log(`复用已有构建目录 ${buildDir}`);
  }

  const jobs = resolveBuildJobs();
  log(
    `cmake 编译（-j ${jobs ?? cpus().length}${jobs ? "，来自 SHERPA_BUILD_JOBS" : ""}，首次约十几分钟）…`,
  );
  run("cmake", ["--build", buildDir, "--config", "Release", ...parallelArgs(jobs)], {
    env: cmakeEnv,
    stdio: childStdio,
  });

  collectDlls(buildDir, outDir);
  const dlls = readdirSync(outDir).filter((f) => f.endsWith(".dll"));
  if (!dlls.some((d) => d.toLowerCase() === "sherpa-onnx-c-api.dll")) {
    throw new Error(
      `产物里没有 sherpa-onnx-c-api.dll（${buildDir}）；现有: ${dlls.join(", ") || "(空)"}`,
    );
  }
  // 标记「这份库是 DirectML 版」：crates/ling-chat-main/build.rs 读它决定 provider。
  writeFileSync(
    join(outDir, "sherpa-build.json"),
    `${JSON.stringify(
      {
        provider: "dml",
        directml: true,
        sherpaOnnxVersion: version,
        builtAt: new Date().toISOString(),
      },
      null,
      2,
    )}\n`,
  );
  log(`产物 ${dlls.length} 个 DLL → ${outDir}`);
  return outDir;
}

/// 把构建树里的 DLL 汇总到一个目录（上游 build.rs 只认「一个目录里放着所有库」）。
/// onnxruntime.dll / DirectML.dll 不收集：应用已经随包了同一份（tauri.windows.conf.json）。
function collectDlls(buildDir, outDir) {
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  const skip = new Set(["onnxruntime.dll", "directml.dll"]);
  const walk = (dir, depth) => {
    if (depth > 4) return;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full, depth + 1);
      } else if (entry.isFile() && entry.name.toLowerCase().endsWith(".dll")) {
        if (skip.has(entry.name.toLowerCase())) continue;
        const dest = join(outDir, entry.name);
        if (!existsSync(dest)) copyFileSync(full, dest);
      }
    }
  };
  walk(buildDir, 0);
}

/// Windows 打包：DLL 暂存交给 stage-desktop-runtime-libs.mjs（单一来源），这里只把
/// 暂存结果写成 --config 覆盖，逐文件映射到资源根目录（= 安装目录）。逐文件而不是整目录，
/// 是因为 Windows 只在 exe 同目录（以及 PATH）里找 DLL。Tauri 对 `--config` 的合并语义是
/// 「同名 key 覆盖、其余保留」，所以把 tauri.windows.conf.json 里已有的
/// onnxruntime.dll / DirectML.dll 映射原样带上。
function stageAndWriteConfig(outDir) {
  process.env.SHERPA_ONNX_LIB_DIR = outDir;
  const { staged } = stageRuntimeLibs({ log });
  if (staged.length === 0) {
    throw new Error("sherpa 动态库没有暂存到任何 DLL，检查构建产物");
  }

  let baseResources = {};
  const windowsConfig = join(tauriDir, "tauri.windows.conf.json");
  if (existsSync(windowsConfig)) {
    baseResources = JSON.parse(readFileSync(windowsConfig, "utf-8")).bundle?.resources ?? {};
  }
  const resources = { ...baseResources };
  for (const dll of staged) {
    resources[`.runtime-libs/${dll}`] = dll;
  }
  writeFileSync(overrideConfigPath, `${JSON.stringify({ bundle: { resources } }, null, 2)}\n`);
  log(`暂存 ${staged.length} 个 DLL → ${stageDir}`);
  log(`已写出 ${overrideConfigPath}（映射到资源根目录）`);
}

async function main() {
  const version = resolveSherpaVersion(projectRoot);
  const patchHash = patchFingerprint(patchesDir);

  ensureOrtBuildDeps();
  const srcDir = await ensureSource({
    cacheRoot,
    patchesDir,
    version,
    patchHash,
    force,
    log,
  });
  const outDir = await buildSherpa({ version, srcDir });

  if (printLibDirOnly) {
    process.stdout.write(`${outDir}\n`);
    return;
  }

  stageAndWriteConfig(outDir);

  if (!shouldBuild) {
    log("只准备了库。打包命令：");
    log(
      `  SHERPA_ONNX_LIB_DIR=${outDir} pnpm tauri build --features sherpa-shared` +
        ` --config ${overrideConfigPath}`,
    );
    log("  （Windows 上直接用 `pnpm tauri:build:win`，不用手写环境变量）");
    return;
  }

  log("开始 tauri build（--features sherpa-shared，SHERPA_ONNX_LIB_DIR 指向本次产物）…");
  const result = spawnSync(
    "pnpm",
    ["exec", "tauri", "build", "--features", "sherpa-shared", "--config", overrideConfigPath],
    {
      cwd: projectRoot,
      stdio: "inherit",
      shell: process.platform === "win32",
      env: { ...process.env, SHERPA_ONNX_LIB_DIR: outDir },
    },
  );
  process.exit(result.status ?? 1);
}

try {
  await main();
} catch (error) {
  fail(error?.message ?? String(error));
}
