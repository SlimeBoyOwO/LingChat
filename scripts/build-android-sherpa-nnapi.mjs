// Android 专用：**构建时**自编一份支持 NNAPI 的 sherpa-onnx，让 Android 上的
// 「GPU 推理」开关真正落到设备后端（GPU/NPU），而不是静默回落 CPU。
//
// 为什么要自编：
//   - sherpa 的 provider 是**编译期**编死的（csrc/session.cc）：
//       #if __ANDROID_API__ >= 27
//         OrtSessionOptionsAppendExecutionProvider_Nnapi(...)   // 真 NNAPI
//       #elif defined(__ANDROID_API__)
//         SHERPA_ONNX_LOGE("Android NNAPI requires API level >= 27 ... Fallback to cpu!")
//       #endif
//     官方的 `sherpa-onnx-<ver>-android.tar.bz2` 用 `-DANDROID_PLATFORM=android-21` 编
//     （上游 build-android-arm64-v8a.sh 的默认值），于是 NNAPI 那段代码根本不存在，
//     运行期传 `nnapi` 只会打印上面那行 fallback 日志然后走 CPU —— 开关是假的。
//   - 上游 build-android*.sh 里就有一行注释：「Please use -DANDROID_PLATFORM=android-27
//     if you want to use Android NNAPI」。本脚本做的就是这件事（外加把 ORT 换成官方
//     Android 包，见下）。
//
// 与 Windows 那份（scripts/build-desktop-sherpa-dml.mjs）的区别：Android 可以**交叉**
// 编译，所以在 Linux/macOS 主机上就能编出来（Windows 主机也能），不需要在设备上编。
//
// 本脚本是「在编译的时候下载源代码并修改」的落点（cargo 的 build.rs 做不到这件事：
// build script 只能把信息传给依赖它的包，**没法把自己的环境变量注入给依赖 crate 的
// build.rs** —— 而 sherpa-onnx-sys 只认 SHERPA_ONNX_LIB_DIR，所以准备动作必须在 cargo
// 之前，由本脚本导出环境变量）：
//   1. 按 Cargo.lock 里 sherpa-onnx-sys 的锁定版本，下源码 tarball 到 target/ 缓存；
//   2. 解压后按 patches/sherpa-onnx/*.patch 顺序打补丁 —— 仓库里只留 .patch，不留改过的
//      源码树。NNAPI 本身只要 cmake 开关（API level），不需要改 C++，所以那个目录现在是
//      空的；将来要动 sherpa 的 C++ 时把 .patch 丢进去即可（补丁内容变化会自动让缓存失效）。
//   3. 下官方 ONNX Runtime 的 Android 包（含 `jni/<abi>/libonnxruntime.so` 与 `headers/`，
//      与上游 build-android-*.sh 用的同一份），走 cmake 的「预装 ORT」分支，省掉自己编 ORT；
//   4. cmake 自编 sherpa：`-DANDROID_ABI=<abi> -DANDROID_PLATFORM=android-27
//      -DBUILD_SHARED_LIBS=ON -DSHERPA_ONNX_ENABLE_C_API=ON -DSHERPA_ONNX_ENABLE_TTS=ON`，
//      产物汇总成一个 lib 目录，并写 sherpa-build.json 标记「这份是 NNAPI 版」
//      （crates/ling-chat-main/build.rs 据此发射 provider）。
//
// 运行期不需要额外库：ORT 用的是官方 Android 包那份 `libonnxruntime.so`（也就是应用
// 本来就在随包的那份，情绪模型/嵌入模型走的也是它 —— 见 scripts/package-android-sherpa-libs.mjs）。
//
// 用法：
//   node scripts/build-android-sherpa-nnapi.mjs --print-lib-dir   # 把库目录打到 stdout
//   node scripts/build-android-sherpa-nnapi.mjs --force           # 忽略缓存重下/重编
//   pnpm android:build                                           # 走这条路的正式打包
//   SHERPA_ANDROID_CPU=1 pnpm android:build                       # 退回官方 CPU 包（快，不用自编）
//
// 可调环境变量：SHERPA_BUILD_JOBS（编译线程数，默认 = 机器核数；也认
// CMAKE_BUILD_PARALLEL_LEVEL）、SHERPA_ANDROID_ABI（默认 arm64-v8a）、
// SHERPA_ANDROID_API_LEVEL（默认 27）、SHERPA_ONNXRUNTIME_VERSION（默认 1.27.1）、
// SHERPA_ONNX_DOWNLOAD_MIRROR（GitHub 下载镜像）。
//
// 前置条件缺失（没装 NDK / 没有 cmake）时**安静跳过**：退出码 0、stdout 无输出，让
// scripts/package-android-sherpa-libs.mjs 回落到官方预编译包（构建照样能成功，只是
// Android 的 GPU 开关会诚实地说「这份构建没编 NNAPI」）。真正的编译错误则非 0 退出。

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  copyFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { cpus } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { ndkHintLines, resolveNdk } from "./lib/android-ndk.mjs";
import {
  download,
  ensureSource,
  parallelArgs,
  patchFingerprint,
  readCacheState,
  resolveBuildJobs,
  resolveSherpaVersion,
  run,
  writeCacheState,
} from "./lib/sherpa-source.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");
const cacheRoot = join(projectRoot, "target", "sherpa-onnx-android-nnapi");
const patchesDir = join(projectRoot, "patches", "sherpa-onnx");

/// 目标 ABI（与 package.json 的 `--target aarch64` 对应）。模拟器用 x86_64。
const abi = process.env.SHERPA_ANDROID_ABI || "arm64-v8a";

/// NNAPI 的编译期门槛：`__ANDROID_API__ >= 27`（Android 8.1）。上游注释写的就是 27。
/// 注意这只是「把 NNAPI 那段编进去」的门槛，运行期在低于 27 的设备上 ORT 会拒绝创建
/// NNAPI EP，sherpa 打一行日志回落 CPU（不崩），所以不必抬高应用的 minSdk。
const apiLevel = process.env.SHERPA_ANDROID_API_LEVEL || "27";

/// 官方 ONNX Runtime Android 包（与上游 build-android-*.sh 用的一致）：
///   https://github.com/csukuangfj/onnxruntime-libs/releases/download/v<ver>/onnxruntime-android-<ver>.zip
/// 内含 `jni/<abi>/libonnxruntime.so` 与 `headers/`。默认版本与 Cargo.lock 里 ort 系列的
/// 应用现状对齐（Windows 的 WinML 包也是 1.27.1）。
const ortVersion = process.env.SHERPA_ONNXRUNTIME_VERSION || "1.27.1";
const ortZipUrl = `https://github.com/csukuangfj/onnxruntime-libs/releases/download/v${ortVersion}/onnxruntime-android-${ortVersion}.zip`;

const args = process.argv.slice(2);
const printLibDirOnly = args.includes("--print-lib-dir");
const force = args.includes("--force");

/// `--print-lib-dir` 模式下 stdout **只**承载「库目录」这一条信息：package.json 的
/// `SHERPA_ONNX_LIB_DIR=$(...)` 和 package-android-sherpa-libs.mjs 的 spawnSync 都靠它解析。
/// 子进程默认 `inherit` 会接着往本进程的 stdout 写，而 cmake 的 `[  3%] Built target …`
/// 与 `-- Installing: …` 全在 stdout —— 上层拿到的「目录」就变成一大段编译日志。CI 上
/// 首次冷构建就是这么炸的（上层判定「自编被跳过」→ 回落官方 CPU 包；本地有官方包缓存时
/// 更隐蔽：不报错，直接打出「GPU 开关是假的」APK）。打印模式下一律把子进程的
/// stdout/stderr 都并到本进程的 stderr（fd 2），stdout 只留最后那行目录。
const childStdio = printLibDirOnly ? ["ignore", 2, 2] : "inherit";

/// 产物目录（同时也是 SHERPA_ONNX_LIB_DIR 指向的目录）。
function libsDir(version) {
  return join(cacheRoot, `libs-v${version}-${abi}-api${apiLevel}`);
}

/// 诊断一律走 stderr：`--print-lib-dir` 模式下 stdout 只留机器可读的目录。
function log(msg) {
  console.error(`[sherpa-nnapi] ${msg}`);
}

function fail(msg) {
  console.error(`[sherpa-nnapi] ${msg}`);
  process.exit(1);
}

/** 前置条件缺失时安静跳过（退出码 0、无输出），让上层回落到官方预编译包。 */
function skip(msg) {
  log(msg);
  process.exit(0);
}

/// 有 cmake 才能编。这里只做「有没有」的检查，版本交给 cmake 自己报错。
function hasCmake() {
  const probe = spawnSync("cmake", ["--version"], { stdio: "ignore" });
  return !probe.error && probe.status === 0;
}

/// 解压 zip：优先 unzip（Unix 常见），没有就退回 tar（Windows 10+ 的 bsdtar 能解 zip）。
function extractZip(zip, dir) {
  const candidates = [
    ["unzip", ["-q", "-o", zip, "-d", dir]],
    ["tar", ["-xf", zip, "-C", dir]],
  ];
  let lastError;
  for (const [cmd, cmdArgs] of candidates) {
    const result = spawnSync(cmd, cmdArgs, { stdio: childStdio });
    if (result.error) {
      lastError = result.error;
      continue;
    }
    if (result.status === 0) return;
    lastError = new Error(`${cmd} 退出码 ${result.status}`);
  }
  throw new Error(`解压 ${zip} 失败：${lastError?.message ?? lastError}`);
}

/// 在解压结果里找 ORT 包的根（含 `headers/` 与 `jni/<abi>/libonnxruntime.so`）。
/// 官方 zip 里可能多一层版本目录，所以这里搜一遍而不是写死路径。
function findOrtRoot(dir) {
  if (!existsSync(dir)) return null;
  const marker = join("headers", "onnxruntime_cxx_api.h");
  const lib = join("jni", abi, "libonnxruntime.so");
  const stack = [dir];
  while (stack.length > 0) {
    const cur = stack.pop();
    if (existsSync(join(cur, marker)) && existsSync(join(cur, lib))) return cur;
    for (const entry of readdirSync(cur, { withFileTypes: true })) {
      if (entry.isDirectory()) stack.push(join(cur, entry.name));
    }
  }
  return null;
}

if (!hasCmake()) skip("找不到 cmake，跳过自编 NNAPI 版（回落到官方预编译包）");

const ndk = (() => {
  const found = resolveNdk();
  if (!found) {
    log("找不到 Android NDK，跳过自编 NNAPI 版（回落到官方预编译包）：");
    for (const line of ndkHintLines()) log(line);
    process.exit(0);
  }
  return found;
})();

/// 下 ONNX Runtime 的 Android 包并解压到缓存（含 libonnxruntime.so 与头文件）。
async function ensureOrtAndroid() {
  const dir = join(cacheRoot, `onnxruntime-android-${ortVersion}`);
  const cached = findOrtRoot(dir);
  if (cached && !force) {
    log(`复用 ONNX Runtime Android 包 ${cached}`);
    return cached;
  }
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  const zip = join(cacheRoot, `onnxruntime-android-${ortVersion}.zip`);
  if (force || !existsSync(zip)) {
    log(`⬇️  ONNX Runtime Android ${ortVersion}（含 jni/${abi} 与 headers/）`);
    await download(ortZipUrl, zip, log);
  }
  log(`解压 ONNX Runtime → ${dir}`);
  extractZip(zip, dir);
  const root = findOrtRoot(dir);
  if (!root) {
    throw new Error(
      `ONNX Runtime 包里找不到 headers/ 与 jni/${abi}/libonnxruntime.so（ABI 写错了？）`,
    );
  }
  return root;
}

/// cmake 配置 + 编译 + 安装（strip 过）。返回汇总好的产物目录。
async function buildSherpa({ version, srcDir, ortDir }) {
  // 目录名带上 apiLevel/abi：换了 API level 就是另一份构建，不能复用旧的 CMakeCache。
  const buildDir = join(cacheRoot, `build-v${version}-${abi}-api${apiLevel}`);
  const installDir = join(cacheRoot, `install-v${version}-${abi}-api${apiLevel}`);
  mkdirSync(buildDir, { recursive: true });

  const cmakeEnv = {
    ...process.env,
    // 走 sherpa 的「预装 ORT」分支（cmake/onnxruntime.cmake）：Android + ANDROID_ABI
    // 时它 glob `$SHERPA_ONNXRUNTIME_LIB_DIR/libonnxruntime.so`，并 install 到 lib/。
    SHERPA_ONNXRUNTIME_LIB_DIR: join(ortDir, "jni", abi),
    SHERPA_ONNXRUNTIME_INCLUDE_DIR: join(ortDir, "headers"),
  };
  const jobs = resolveBuildJobs();
  log(`SHERPA_ONNXRUNTIME_LIB_DIR=${cmakeEnv.SHERPA_ONNXRUNTIME_LIB_DIR}`);
  log(`SHERPA_ONNXRUNTIME_INCLUDE_DIR=${cmakeEnv.SHERPA_ONNXRUNTIME_INCLUDE_DIR}`);

  // 与上游 build-android-arm64-v8a.sh 对齐（少一步 ORT 下载，多了 ANDROID_PLATFORM）。
  // JNI 关掉：应用走 C API（sherpa-onnx-sys 链接 libsherpa-onnx-c-api.so），不需要
  // libsherpa-onnx-jni.so，也就不在 jniLibs 里多背一个 .so。
  const configureArgs = [
    "-S",
    srcDir,
    "-B",
    buildDir,
    `-DCMAKE_TOOLCHAIN_FILE=${join(ndk, "build", "cmake", "android.toolchain.cmake")}`,
    `-DANDROID_ABI=${abi}`,
    `-DANDROID_PLATFORM=android-${apiLevel}`,
    "-DCMAKE_BUILD_TYPE=Release",
    "-DBUILD_SHARED_LIBS=ON",
    "-DSHERPA_ONNX_ENABLE_TTS=ON",
    "-DSHERPA_ONNX_ENABLE_C_API=ON",
    // 与官方 Android 包保持同一套算子/符号（应用链接的就是这份 c-api），
    // 唯一的差别是 ANDROID_PLATFORM。
    "-DSHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION=ON",
    "-DSHERPA_ONNX_ENABLE_BINARY=OFF",
    "-DSHERPA_ONNX_ENABLE_JNI=OFF",
    "-DSHERPA_ONNX_ENABLE_PYTHON=OFF",
    "-DSHERPA_ONNX_ENABLE_TESTS=OFF",
    "-DSHERPA_ONNX_ENABLE_CHECK=OFF",
    "-DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF",
    "-DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF",
    "-DSHERPA_ONNX_ENABLE_RKNN=OFF",
    "-DSHERPA_ONNX_ENABLE_QNN=OFF",
    "-DSHERPA_ONNX_LINK_LIBSTDCPP_STATICALLY=OFF",
    "-DBUILD_PIPER_PHONMIZE_EXE=OFF",
    "-DBUILD_PIPER_PHONMIZE_TESTS=OFF",
    "-DBUILD_ESPEAK_NG_EXE=OFF",
    "-DBUILD_ESPEAK_NG_TESTS=OFF",
    `-DCMAKE_INSTALL_PREFIX=${installDir}`,
  ];
  if (force || !existsSync(join(buildDir, "CMakeCache.txt"))) {
    log("cmake 配置…");
    run("cmake", configureArgs, { env: cmakeEnv, stdio: childStdio });
  } else {
    log(`复用已有构建目录 ${buildDir}（--force 可重新配置）`);
  }

  log(
    `cmake 编译（-j ${jobs ?? cpus().length}${jobs ? "，来自 SHERPA_BUILD_JOBS" : ""}，首次约十几分钟）…`,
  );
  run("cmake", ["--build", buildDir, ...parallelArgs(jobs)], { env: cmakeEnv, stdio: childStdio });
  log("安装（strip）…");
  run("cmake", ["--build", buildDir, "--target", "install/strip"], {
    env: cmakeEnv,
    stdio: childStdio,
  });

  return collectLibs({ installDir, outDir: libsDir(version), version, ortDir });
}

/// 把 install/lib 下的 .so 汇总到一个目录（上游 build.rs 只认「一个目录里放着所有库」），
/// 并写 sherpa-build.json 标记。
function collectLibs({ installDir, outDir, version, ortDir }) {
  const libDir = join(installDir, "lib");
  if (!existsSync(libDir)) throw new Error(`没有安装目录 ${libDir}`);
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  const sos = readdirSync(libDir).filter((f) => f.endsWith(".so"));
  for (const so of sos) {
    copyFileSync(join(libDir, so), join(outDir, so));
  }

  // ORT 的 .so 必须一起走：cmake 那边会 glob 进来，但宁可显式确认一次
  // （应用的情绪模型/嵌入模型也加载这一份 libonnxruntime.so）。
  const ortSo = "libonnxruntime.so";
  if (!readdirSync(outDir).includes(ortSo)) {
    copyFileSync(join(ortDir, "jni", abi, ortSo), join(outDir, ortSo));
    log(`${ortSo} 不在 install/lib 里，已从 ORT 包再拷一份`);
  }

  const cApi = "libsherpa-onnx-c-api.so";
  if (!readdirSync(outDir).includes(cApi)) {
    throw new Error(`产物里没有 ${cApi}；现有: ${readdirSync(outDir).join(", ") || "(空)"}`);
  }

  // 编译期自检：这份 .so 里**不该**再有「需要 API 27」的回落分支字符串，而应该有
  // 「Use nnapi」。缺了说明 ANDROID_PLATFORM 没生效，会静默回落 CPU —— 直接失败，
  // 免得打出一个「开关是假的」包。
  const raw = readFileSync(join(outDir, cApi), "latin1");
  const cpuFallback = raw.includes("Android NNAPI requires API level >= 27");
  const nnapiBranch = raw.includes("Use nnapi");
  if (cpuFallback || !nnapiBranch) {
    throw new Error(
      `这份 ${cApi} 没编进 NNAPI（API level ${apiLevel} 没生效？）` +
        `[fallback 字符串=${cpuFallback} nnapi 字符串=${nnapiBranch}]`,
    );
  }
  log(`✅ NNAPI 分支已编入（ANDROID_PLATFORM=android-${apiLevel}）`);

  writeFileSync(
    join(outDir, "sherpa-build.json"),
    `${JSON.stringify(
      {
        provider: "nnapi",
        nnapi: true,
        androidApiLevel: Number(apiLevel),
        androidAbi: abi,
        onnxruntimeVersion: ortVersion,
        sherpaOnnxVersion: version,
        builtAt: new Date().toISOString(),
      },
      null,
      2,
    )}\n`,
  );
  log(`产物 ${readdirSync(outDir).filter((f) => f.endsWith(".so")).length} 个 .so → ${outDir}`);
  return outDir;
}

async function main() {
  const version = resolveSherpaVersion(projectRoot);
  const patchHash = patchFingerprint(patchesDir);
  const state = readCacheState(cacheRoot);
  log(`sherpa-onnx ${version} / ${abi} / android-${apiLevel} / NDK ${ndk}`);

  const srcDir = await ensureSource({ cacheRoot, patchesDir, version, patchHash, force, log });
  const ortDir = await ensureOrtAndroid();

  const outDir = libsDir(version);
  const cached =
    existsSync(join(outDir, "sherpa-build.json")) &&
    existsSync(join(outDir, "libsherpa-onnx-c-api.so")) &&
    state?.builtAbi === abi &&
    state?.builtApiLevel === Number(apiLevel);
  if (cached && !force) {
    log(`复用已有产物 ${outDir}`);
  } else {
    await buildSherpa({ version, srcDir, ortDir });
    writeCacheState(cacheRoot, {
      ...(state ?? {}),
      version,
      patchHash,
      builtAbi: abi,
      builtApiLevel: Number(apiLevel),
    });
  }

  if (printLibDirOnly) {
    process.stdout.write(`${outDir}\n`);
    return;
  }

  log("库已就绪。打包方式（package.json 里的 android:* 已串好，不用手写）：");
  log(`  SHERPA_ONNX_LIB_DIR=${outDir} pnpm android:build`);
}

try {
  await main();
} catch (error) {
  fail(error?.message ?? String(error));
}
