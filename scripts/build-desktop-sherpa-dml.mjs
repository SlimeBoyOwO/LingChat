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
// 非 Windows 平台直接跳过（退出码 0、stdout 无输出），所以可以安全地挂在跨平台入口。
// 需要：Visual Studio（MSVC + Windows 10 SDK，dml_provider_factory.h 要 d3d12.h）、
// cmake、网络。首次构建约十几分钟（sherpa 全量编译），之后走缓存。

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { cpus } from "node:os";
import { dirname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

import { stageDir, stageRuntimeLibs } from "./stage-desktop-runtime-libs.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const tauriDir = join(projectRoot, "src-tauri");
const binariesDir = join(tauriDir, "binaries");
const winmlOrtDir = join(binariesDir, "winml-ort");
const cacheRoot = join(projectRoot, "target", "sherpa-onnx-directml");
const patchesDir = join(projectRoot, "patches", "sherpa-onnx");
// Windows 的 --config 覆盖：把本次编出来的 DLL 逐文件映射到资源根目录（= 安装目录）。
const overrideConfigPath = join(tauriDir, "tauri.sherpa-dml.conf.json");

const SOURCE_ARCHIVE = "https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags";
/// 与 sherpa 的 cmake/onnxruntime-win-x64-directml.cmake 保持一致：DirectML 包的
/// 版本和 sha256 写在它的 URL_HASH 里。预下到 build 目录可以让 cmake 直接用本地包
/// （URL_HASH 仍然会校验），绕开 nuget.org 在部分网络下的超时。
const DIRECTML_VERSION = "1.15.0";
const DIRECTML_URL = `https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/${DIRECTML_VERSION}`;
const DIRECTML_LOCAL_NAME = `Microsoft.AI.DirectML.${DIRECTML_VERSION}.nupkg`;
const DIRECTML_SHA256 = "10d175f8e97447712b3680e3ac020bbb8eafdf651332b48f09ffee2eec801c23";

/// GitHub 直连在国内常超时，下载按「直连 → 镜像」退避；`SHERPA_ONNX_DOWNLOAD_MIRROR`
/// 可指定单个前缀或 `none` 关闭。nuget.org 不走镜像（它自己会重定向到 azure 镜像）。
const DEFAULT_MIRRORS = ["https://ghfast.top", "https://gh-proxy.com", "https://ghproxy.net"];

const args = process.argv.slice(2);
const printLibDirOnly = args.includes("--print-lib-dir");
const force = args.includes("--force");
const shouldBuild = args.includes("--build");

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

function resolveDownloadUrls(url) {
  if (!url.includes("github.com")) return [url];
  const override = (process.env.SHERPA_ONNX_DOWNLOAD_MIRROR ?? "").trim();
  let mirrors;
  if (override === "none") mirrors = [];
  else if (override) mirrors = [override.replace(/\/+$/, "")];
  else mirrors = DEFAULT_MIRRORS;
  return [url, ...mirrors.map((m) => `${m}/${url}`)];
}

async function download(url, dest) {
  const urls = resolveDownloadUrls(url);
  let lastError;
  for (const candidate of urls) {
    try {
      const res = await fetch(candidate);
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`);
      await pipeline(res.body, createWriteStream(dest));
      return;
    } catch (error) {
      lastError = error;
      rmSync(dest, { force: true });
      log(`下载失败（${candidate}）: ${error.message}；换下一个源`);
    }
  }
  throw new Error(`下载 ${url} 失败：${lastError?.message ?? lastError}`);
}

function run(cmd, cmdArgs, opts = {}) {
  const result = spawnSync(cmd, cmdArgs, { stdio: "inherit", ...opts });
  if (result.error) throw new Error(`执行 ${cmd} 失败：${result.error.message}`);
  if (result.status !== 0) throw new Error(`${cmd} 退出码 ${result.status}`);
}

/// 本次要编的版本 = Cargo.lock 里 sherpa-onnx-sys 的锁定版本（也就是真会被编的版本）。
/// 拿不到就直接报错：宁可明确失败，也不要编出一份与运行时不同的库。
function resolveSherpaVersion() {
  const lock = join(projectRoot, "Cargo.lock");
  if (!existsSync(lock)) throw new Error(`找不到 ${lock}，无法确定 sherpa-onnx 版本`);
  const lines = readFileSync(lock, "utf-8").split("\n");
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() !== 'name = "sherpa-onnx-sys"') continue;
    for (let j = i + 1; j < Math.min(i + 6, lines.length); j++) {
      const match = /^version = "(.+)"$/.exec(lines[j].trim());
      if (match) return match[1];
    }
  }
  throw new Error("Cargo.lock 里没有 sherpa-onnx-sys，先跑一次 cargo fetch");
}

/// 补丁集合的指纹：文件名 + 内容。变了就重新解压重打，避免「旧源码树 + 新补丁」这种
/// 半新半旧的状态。
function patchFingerprint() {
  if (!existsSync(patchesDir)) return "none";
  const files = readdirSync(patchesDir)
    .filter((f) => f.endsWith(".patch"))
    .sort();
  const hash = createHash("sha256");
  for (const f of files) {
    hash.update(f);
    hash.update(readFileSync(join(patchesDir, f)));
  }
  return `${files.length}:${hash.digest("hex").slice(0, 16)}`;
}

function cacheStatePath() {
  return join(cacheRoot, "state.json");
}

function readCacheState() {
  try {
    return JSON.parse(readFileSync(cacheStatePath(), "utf-8"));
  } catch {
    return null;
  }
}

function writeCacheState(state) {
  mkdirSync(cacheRoot, { recursive: true });
  writeFileSync(cacheStatePath(), `${JSON.stringify(state, null, 2)}\n`);
}

/// 把源码下下来、解压、打补丁。返回源码目录。
async function ensureSource({ version, patchHash }) {
  const srcDir = join(cacheRoot, `sherpa-onnx-v${version}-src`);
  const srcArchive = join(cacheRoot, `sherpa-onnx-v${version}-src.tar.gz`);
  const ready = existsSync(join(srcDir, "CMakeLists.txt"));

  if (ready && !force) {
    log(`复用已解压源码 ${srcDir}（补丁指纹 ${patchHash}）`);
    return srcDir;
  }

  if (force || !existsSync(srcArchive)) {
    log(`⬇️  源码 ${SOURCE_ARCHIVE}/v${version}.tar.gz`);
    await download(`${SOURCE_ARCHIVE}/v${version}.tar.gz`, srcArchive);
  }

  rmSync(srcDir, { recursive: true, force: true });
  mkdirSync(srcDir, { recursive: true });
  log(`解压源码 → ${srcDir}`);
  run("tar", ["-xzf", srcArchive, "-C", srcDir, "--strip-components=1"]);

  // 便宜的对账：tarball 里的版本必须与要编的版本一致（不做 sha256，因为源码包由
  // GitHub 动态打包；版本号对得上就足以发现「下错了 tag / 镜像返回了别的东西」）。
  const cmakeLists = readFileSync(join(srcDir, "CMakeLists.txt"), "utf-8");
  const declared = /set\(SHERPA_ONNX_VERSION\s+"([^"]+)"\)/.exec(cmakeLists)?.[1];
  if (declared !== version) {
    throw new Error(`源码版本不匹配：Cargo.lock=${version}，CMakeLists=${declared}`);
  }

  applyPatches(srcDir);
  return srcDir;
}

/// 按文件名顺序把 patches/sherpa-onnx/*.patch 打到新鲜源码上。
///
/// 这就是「构建时下载源码并修改」里「修改」的那一半：仓库只保存 diff，改的是临时源码树。
/// 优先用 git apply（Git for Windows 一定有），没有 git 时退回 GNU patch。
function applyPatches(srcDir) {
  if (!existsSync(patchesDir)) return;
  const patches = readdirSync(patchesDir)
    .filter((f) => f.endsWith(".patch"))
    .sort();
  if (patches.length === 0) {
    log("patches/sherpa-onnx/ 为空：DirectML 只需要 cmake 开关，不需要改源码");
    return;
  }
  for (const patch of patches) {
    const file = join(patchesDir, patch);
    log(`打补丁 ${patch}`);
    const viaGit = spawnSync("git", ["apply", "-p1", file], { cwd: srcDir, stdio: "inherit" });
    if (viaGit.status === 0) continue;
    const viaPatch = spawnSync("patch", ["-p1", "-i", file], { cwd: srcDir, stdio: "inherit" });
    if (viaPatch.status !== 0) throw new Error(`补丁 ${patch} 打不上（git apply / patch 都失败）`);
  }
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
  run(process.execPath, [join(__dirname, "ensure-onnxruntime.mjs")], { cwd: projectRoot });
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
    run("cmake", configureArgs, { env: cmakeEnv });
  } else {
    log(`复用已有构建目录 ${buildDir}`);
  }

  log(`cmake 编译（-j ${Math.max(1, cpus().length)}，首次约十几分钟）…`);
  run("cmake", ["--build", buildDir, "--config", "Release", "--parallel"], {
    env: cmakeEnv,
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
  const version = resolveSherpaVersion();
  const patchHash = patchFingerprint();
  const state = readCacheState();
  if (force || state?.version !== version || state?.patchHash !== patchHash) {
    writeCacheState({ version, patchHash });
  }

  ensureOrtBuildDeps();
  const srcDir = await ensureSource({ version, patchHash });
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
