// Android 构建前：把 Sherpa-ONNX 的动态库(.so)打进 jniLibs。
//
// 背景：crates/ling-chat-main 对 Android 用 sherpa-onnx 的 `shared` 特性，
// sherpa-onnx-sys 的 build.rs 只负责链接（-L/-l），产出
// `DT_NEEDED: libsherpa-onnx-c-api.so`、`libonnxruntime.so`，
// 但**不会**把 .so 放进 APK。缺了它们，应用一启动 dlopen 就崩溃。
// 因此这里在 tauri android build 之前，把库拷到
// src-tauri/gen/android/app/src/main/jniLibs/<abi>/，交给 Gradle 打包
// （jniLibs/**/*.so 已被 .gitignore 忽略，属本地产物）。
//
// 库来源与 sherpa-onnx-sys 的 resolve_lib_dir 语义保持一致，按优先级：
//   1. 环境变量 SHERPA_ONNX_LIB_DIR（用户手工指定的预编译库目录）
//   2. 构建时自编的 **NNAPI 版**（scripts/build-android-sherpa-nnapi.mjs）——
//      Android 的「GPU 推理」开关只有在编了 NNAPI 的库里才有意义：官方
//      `-android` 包按 API 21 编，NNAPI 那段代码根本不在（见那个脚本的注释）。
//      这是**默认**行为；`SHERPA_ANDROID_CPU=1` 或 `--cpu` 可跳过它。
//   3. cargo 下载缓存 target/sherpa-onnx-prebuilt/**（官方预编译包，CPU）
// 找不到必需库时非 0 退出，避免打出发车即崩溃的 APK。
//
// 「回落到官方包」只发生在自编脚本**安静跳过**（前置条件不足、stdout 为空）时；
// 自编一旦真的产出了目录，产物缺库 / stdout 被污染都直接非 0 退出 —— 回落会打出
// 一个「GPU 开关是假的」APK（CI 首次冷构建踩过这个坑）。
//
// 另外，sherpa-onnx-sys 的 build.rs **链接期**也要这个目录：不设
// SHERPA_ONNX_LIB_DIR 时它会走自己那套有 bug 的下载/解压逻辑（上游 Android
// 归档没有顶层目录，解到 target/sherpa-onnx-prebuilt/jniLibs/<abi>，而
// build.rs 只在 .../sherpa-onnx-v<ver>-android/jniLibs/<abi> 找），于是直接
// panic。所以加 `--print-lib-dir`：本脚本把解析出的目录单独打到 stdout，
// package.json 的 android:* 脚本用 `SHERPA_ONNX_LIB_DIR=$(...)` 捕获并导出，
// 这样 `pnpm android:devbuild` 无需手工 export。人类可读日志在
// `--print-lib-dir` 模式下改走 stderr。

import { existsSync, mkdirSync, copyFileSync, readdirSync, rmSync, statSync } from "fs";
import { spawnSync } from "child_process";
import { join, dirname, sep } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, "..");

// 与 package.json 的 android:build/-devbuild 的 `--target aarch64` 对应；
// 需要别的 ABI 时用 SHERPA_ANDROID_ABI 覆盖。
const abi = process.env.SHERPA_ANDROID_ABI || "arm64-v8a";

// `--print-lib-dir`：stdout 只输出解析出的库目录（供 android:* 脚本捕获），
// 其余人类可读日志一律走 stderr，避免污染命令替换的结果。
const printLibDir = process.argv.includes("--print-lib-dir");
const log = printLibDir ? console.error : console.log;

const jniLibsDir = join(
  projectRoot,
  "src-tauri",
  "gen",
  "android",
  "app",
  "src",
  "main",
  "jniLibs",
  abi,
);
const prebuiltRoot = join(projectRoot, "target", "sherpa-onnx-prebuilt");

// 运行时真正需要的两个（libling_chat_lib.so → c-api → onnxruntime 的依赖链）；
// 同源目录里的其余 .so 一并拷入，保证传递依赖齐全。
const requiredSo = ["libsherpa-onnx-c-api.so", "libonnxruntime.so"];

/** 返回目录下所有 .so 文件名；目录不存在或异常则返回空数组。 */
function listSo(dir) {
  if (!existsSync(dir)) return [];
  try {
    return readdirSync(dir).filter((f) => f.endsWith(".so"));
  } catch {
    return [];
  }
}

/** 递归收集包含必需库的候选目录（用于 cargo 下载缓存）。 */
function findCacheDirsWith(required, root) {
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
    const sos = new Set(
      entries.filter((e) => e.isFile() && e.name.endsWith(".so")).map((e) => e.name),
    );
    if (required.every((r) => sos.has(r))) {
      found.push(cur);
    }
    for (const e of entries) {
      if (e.isDirectory()) stack.push(join(cur, e.name));
    }
  }
  // 优先命中路径中带目标 ABI 的目录（如 jniLibs/arm64-v8a）
  found.sort((a, b) => {
    const aHit = a.includes(`${sep}${abi}`) ? 0 : 1;
    const bHit = b.includes(`${sep}${abi}`) ? 0 : 1;
    if (aHit !== bHit) return aHit - bHit;
    return a.length - b.length;
  });
  return found;
}

function pickSource() {
  const fromEnv = process.env.SHERPA_ONNX_LIB_DIR;
  if (fromEnv) {
    const sos = listSo(fromEnv);
    if (requiredSo.every((r) => sos.includes(r))) return fromEnv;
    console.warn(
      `[sherpa-libs] SHERPA_ONNX_LIB_DIR 存在但缺少必需库，忽略并回落到缓存: ${fromEnv}`,
    );
  }

  // 默认走自编的 NNAPI 版（Android 上真正能用设备后端的那份）。
  const fromNnapiBuild = ensureNnapiBuild();
  if (fromNnapiBuild) {
    const sos = listSo(fromNnapiBuild);
    if (requiredSo.every((r) => sos.includes(r))) return fromNnapiBuild;
    // 自编脚本自己就会校验产物（缺 c-api / 没编进 NNAPI 都会直接失败），所以走到这里
    // 说明是自编环节出了意外。**不能**回落官方 CPU 包：那会打出「GPU 开关是假的」APK。
    console.error(
      [
        `[sherpa-libs] 自编 NNAPI 产物目录里缺必需库: ${fromNnapiBuild}`,
        `  缺少: ${requiredSo.filter((r) => !sos.includes(r)).join(", ")}`,
        `  现有: ${sos.join(", ") || "(空)"}`,
        `  这是自编环节的问题，不回落官方包（回落会打出「GPU 开关是假的」APK）。`,
      ].join("\n"),
    );
    process.exit(1);
  }

  // 先看缓存根的常见布局，再递归兜底
  const direct = join(prebuiltRoot, "jniLibs", abi);
  if (requiredSo.every((r) => listSo(direct).includes(r))) return direct;
  const candidates = findCacheDirsWith(requiredSo, prebuiltRoot);
  return candidates[0] || null;
}

/// 触发一次「构建时自编 NNAPI 版 sherpa」并拿到它的库目录。
///
/// - `SHERPA_ANDROID_CPU=1`（或 `--cpu`）跳过，直接用官方预编译包（快，CPU）。
/// - 前置条件缺失（没装 NDK / 没 cmake）时自编脚本会安静退出（0 + 空 stdout），
///   这里返回 null → 回落官方包，构建照样能成功。
/// - 真的编译失败（非 0 退出）**不**静默回落：把错误暴露出来，免得打出假的 GPU 包。
function ensureNnapiBuild() {
  if (process.env.SHERPA_ANDROID_CPU === "1" || process.argv.includes("--cpu")) {
    log("[sherpa-libs] SHERPA_ANDROID_CPU=1：跳过 NNAPI 自编，用官方预编译包（CPU）");
    return null;
  }
  const builder = join(__dirname, "build-android-sherpa-nnapi.mjs");
  if (!existsSync(builder)) return null;
  const result = spawnSync(process.execPath, [builder, "--print-lib-dir"], {
    cwd: projectRoot,
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.error) {
    console.error(`[sherpa-libs] 调动 ${builder} 失败: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    console.error(`[sherpa-libs] NNAPI 自编失败（退出码 ${result.status}）`);
    process.exit(result.status ?? 1);
  }
  const dir = result.stdout.toString().trim();
  if (!dir) {
    log("[sherpa-libs] 自编 NNAPI 版被跳过（前置条件不足），回落到官方预编译包（CPU）");
    return null;
  }
  // stdout 协议：自编脚本只输出一行库目录。被 cmake 的编译日志污染时（子进程 stdout
  // 混了进来，CI 首次冷构建就是这么炸的）必须**报错**，不能当成「自编被跳过」回落
  // CPU 包 —— 回落会静默打出「GPU 开关是假的」APK。
  const lines = dir
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length > 1) {
    console.error(
      [
        `[sherpa-libs] 自编脚本的 stdout 被污染：期望一行库目录，实际 ${lines.length} 行。`,
        `  首行: ${lines[0]}`,
        `  末行: ${lines[lines.length - 1]}`,
        `  常见原因是子进程（cmake 的编译/安装输出）混进了 stdout。`,
      ].join("\n"),
    );
    process.exit(1);
  }
  if (!existsSync(dir)) {
    console.error(`[sherpa-libs] 自编脚本返回的库目录不存在: ${dir}`);
    process.exit(1);
  }
  return dir;
}

const source = pickSource();
if (!source) {
  console.error(
    [
      `[sherpa-libs] 找不到 Sherpa-ONNX Android 预编译库，无法打包。`,
      `  缺少: ${requiredSo.join(", ")}`,
      `  已查找: SHERPA_ONNX_LIB_DIR=${process.env.SHERPA_ONNX_LIB_DIR || "(未设置)"}`,
      `          自编 NNAPI 版（scripts/build-android-sherpa-nnapi.mjs）`,
      `          ${prebuiltRoot}`,
      `  解决办法:`,
      `    先跑 node scripts/build-android-sherpa-nnapi.mjs --print-lib-dir 看看自编`,
      `    卡在哪一步（没装 NDK / 没有 cmake 会安静跳过并回落到官方包），或者`,
      `    导出 SHERPA_ONNX_LIB_DIR 指向含上述 .so 的目录，例如`,
      `      export SHERPA_ONNX_LIB_DIR=~/sherpa-onnx-android/${abi}/lib`,
      `    （官方 Android 包解压后形如 sherpa-onnx-*/jniLibs/${abi}/，`,
      `      取其中含 .so 的那个目录即可）`,
    ].join("\n"),
  );
  process.exit(1);
}

const sos = listSo(source);
mkdirSync(jniLibsDir, { recursive: true });
let copied = 0;
for (const so of sos) {
  const src = join(source, so);
  const dst = join(jniLibsDir, so);
  // 源即目标（例如 SHERPA_ONNX_LIB_DIR 本就指向 jniLibs）时跳过，避免自我拷贝
  if (src === dst) continue;
  const same =
    existsSync(dst) &&
    (() => {
      try {
        return statSync(src).size === statSync(dst).size;
      } catch {
        return false;
      }
    })();
  if (same) continue;
  copyFileSync(src, dst);
  copied++;
}

/// 清掉来源里已经没有的 sherpa/ORT 库：换了来源（官方 CPU 包 ↔ 自编 NNAPI 版）之后
// 旧文件会一直留在 jniLibs 里。只动 `libsherpa-onnx-*` / `libonnxruntime*` 这几个前缀，
// jniLibs 里还有 cargo 产出的 libling_chat_lib.so 与 NDK 的 libc++_shared.so（都是符号
// 链接），绝不能碰。
let removed = 0;
const sourceSet = new Set(sos);
for (const so of listSo(jniLibsDir)) {
  if (sourceSet.has(so)) continue;
  if (!/^(libsherpa-onnx-|libonnxruntime)/.test(so)) continue;
  rmSync(join(jniLibsDir, so), { force: true });
  removed++;
}

// 二次校验：目标目录里必须有运行时依赖，否则构建出的 APK 一启动就崩
const destSos = listSo(jniLibsDir);
const missing = requiredSo.filter((r) => !destSos.includes(r));
if (missing.length) {
  console.error(`[sherpa-libs] 打包后仍缺少必需库: ${missing.join(", ")}（来源: ${source}）`);
  process.exit(1);
}

log(
  `[sherpa-libs] ABI=${abi} 来源=${source} 新增=${copied} 清理=${removed} 现有=${destSos.length} -> ${jniLibsDir}`,
);

// `--print-lib-dir` 模式下，stdout 只输出这一行库目录（含末尾换行，
// 便于 $(...) 命令替换去掉换行后直接用作 SHERPA_ONNX_LIB_DIR）。
if (printLibDir) process.stdout.write(`${source}\n`);
