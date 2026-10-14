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
//   2. cargo 下载缓存 target/sherpa-onnx-prebuilt/**（build.rs 的回落来源）
// 找不到必需库时非 0 退出，避免打出发车即崩溃的 APK。
//
// 另外，sherpa-onnx-sys 的 build.rs **链接期**也要这个目录：不设
// SHERPA_ONNX_LIB_DIR 时它会走自己那套有 bug 的下载/解压逻辑（上游 Android
// 归档没有顶层目录，解到 target/sherpa-onnx-prebuilt/jniLibs/<abi>，而
// build.rs 只在 .../sherpa-onnx-v<ver>-android/jniLibs/<abi> 找），于是直接
// panic。所以加 `--print-lib-dir`：本脚本把解析出的目录单独打到 stdout，
// package.json 的 android:* 脚本用 `SHERPA_ONNX_LIB_DIR=$(...)` 捕获并导出，
// 这样 `pnpm android:devbuild` 无需手工 export。人类可读日志在
// `--print-lib-dir` 模式下改走 stderr。

import { existsSync, mkdirSync, copyFileSync, readdirSync, statSync } from "fs";
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
  // 先看缓存根的常见布局，再递归兜底
  const direct = join(prebuiltRoot, "jniLibs", abi);
  if (requiredSo.every((r) => listSo(direct).includes(r))) return direct;
  const candidates = findCacheDirsWith(requiredSo, prebuiltRoot);
  return candidates[0] || null;
}

const source = pickSource();
if (!source) {
  console.error(
    [
      `[sherpa-libs] 找不到 Sherpa-ONNX Android 预编译库，无法打包。`,
      `  缺少: ${requiredSo.join(", ")}`,
      `  已查找: SHERPA_ONNX_LIB_DIR=${process.env.SHERPA_ONNX_LIB_DIR || "(未设置)"}`,
      `          ${prebuiltRoot}`,
      `  解决办法:`,
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

// 二次校验：目标目录里必须有运行时依赖，否则构建出的 APK 一启动就崩
const destSos = listSo(jniLibsDir);
const missing = requiredSo.filter((r) => !destSos.includes(r));
if (missing.length) {
  console.error(`[sherpa-libs] 打包后仍缺少必需库: ${missing.join(", ")}（来源: ${source}）`);
  process.exit(1);
}

log(
  `[sherpa-libs] ABI=${abi} 来源=${source} 新增=${copied} 现有=${destSos.length} -> ${jniLibsDir}`,
);

// `--print-lib-dir` 模式下，stdout 只输出这一行库目录（含末尾换行，
// 便于 $(...) 命令替换去掉换行后直接用作 SHERPA_ONNX_LIB_DIR）。
if (printLibDir) process.stdout.write(`${source}\n`);
