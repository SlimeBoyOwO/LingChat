// Sherpa-ONNX 运行库的准备脚本：**编译哪个版本，就下哪个版本**。
//
// 背景：上游 `sherpa-onnx-sys` 的 build.rs 只会下官方 **CPU** 包（包名由
// `CARGO_PKG_VERSION` 拼出，所以版本这块本来就是自动的），桌面 GPU 变体需要的
// **CUDA** 包它不知道。以前靠 vendored 一份 sherpa-onnx-sys 改 build.rs 实现，
// 现在改成「库在外准备、用 `SHERPA_ONNX_LIB_DIR` 告诉 cargo」：
//
//   1. 本脚本解析版本（优先 Cargo.lock 里 `sherpa-onnx-sys` 的锁定版本，也就是本次
//      真正会编译的那个版本；拿不到再回退到 Cargo.toml 里的 pin）；
//   2. 用 GitHub Release API 按版本找资产（`...-cuda-<主版本>.…-{linux-x64-gpu,
//      win-x64-cuda}.tar.bz2`），而不是把 `cudnn-9.x-onnxruntime1.27.1` 这类标签
//      硬编码进来 —— 上游换 ORT/cuDNN 版本时本脚本不用改；
//   3. 下载（失败重试 3 次）并解压到 `target/sherpa-onnx-prebuilt/<包名>/`，
//      与上游 build.rs 的缓存同目录，离线时可复用已下的 tar.bz2；
//   4. 打印/导出 `SHERPA_ONNX_LIB_DIR`。上游 build.rs 一见到它就跳过下载直接用，
//      于是「选的包」与「编的版本」天然一致。
//
// 用法：
//   node scripts/fetch-sherpa-onnx-libs.mjs --variant=cuda12
//   eval "$(node scripts/fetch-sherpa-onnx-libs.mjs --variant=cuda12 --print-env)"
//   cargo build -p ling_chat --release --features sherpa-shared
//   pnpm tauri:build:gpu                      # 打包入口，内部就是这么调的
//
//   自编（不走官方包）：`--from=cmake`，见本文件末尾 buildFromSource() 的说明。
//
// 注意：**必须**同时开 `--features sherpa-shared`。官方 CUDA 包只含动态库，
// 而上游 build.rs 里 static/shared 同时开是硬报错，所以链接方式只能由 Cargo
// feature 决定，不能像以前那样在 build.rs 里按环境变量改。
//
// 平台限制：官方 CUDA 包只有 linux-x64 / win-x64；macOS 走 CoreML、Android 走
// 官方 `-android.tar.bz2`（CPU 版，shared），都不需要本脚本。

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
} from "node:fs";
import { cpus } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";

import {
  GPU_VARIANTS,
  listLibs,
  prebuiltRoot,
  projectRoot,
  requiredSherpaLibs,
  resolveVariant,
} from "./stage-desktop-runtime-libs.mjs";

const RELEASE_TAG = "https://github.com/k2-fsa/sherpa-onnx/releases/tag";
const RELEASE_DOWNLOAD = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
const RELEASE_API = "https://api.github.com/repos/k2-fsa/sherpa-onnx/releases/tags";
const SOURCE_ARCHIVE = "https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags";

const DOWNLOAD_RETRIES = 3;

/**
 * GitHub 直连在部分网络下不可达（实测：api.github.com 通、github.com 超时），
 * 于是下载按「直连 -> 镜像」顺序退避。用 `SHERPA_ONNX_DOWNLOAD_MIRROR` 覆盖：
 * 给一个前缀（如 `https://ghfast.top`）或 `none` 关闭镜像。
 *
 * 走镜像时只要 Release API 给了 `digest`（sha256），下载完就校验，见 verifyDigest。
 */
const DEFAULT_MIRRORS = [
  "https://ghfast.top", // 用法：https://ghfast.top/https://github.com/...
  "https://gh-proxy.com",
  "https://ghproxy.net",
];

/** GitHub raw/release 是直连还是走镜像，控制台里要说清楚（别让人以为静默换源了）。 */
function resolveDownloadUrls(url) {
  const override = (process.env.SHERPA_ONNX_DOWNLOAD_MIRROR ?? "").trim();
  let mirrors;
  if (override === "none") mirrors = [];
  else if (override) mirrors = [override.replace(/\/+$/, "")];
  else mirrors = DEFAULT_MIRRORS;
  return [url, ...mirrors.map((m) => `${m}/${url}`)];
}

const isWindows = process.platform === "win32";
const isMac = process.platform === "darwin";

function log(msg) {
  // 诊断信息一律走 stderr：stdout 只留给 `--print-env` / `--print-lib-dir` 的机器可读
  // 输出，否则 `eval "$(node ... --print-env)"` 会把日志行当命令执行。
  console.error(`[sherpa-libs] ${msg}`);
}

function fail(msg) {
  console.error(`[sherpa-libs] ${msg}`);
  process.exit(1);
}

/** 解析 `--key=value` / `--key value` / 布尔 `--key`。 */
function parseArgs(argv) {
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg.startsWith("--")) continue;
    const eq = arg.indexOf("=");
    if (eq !== -1) {
      opts[arg.slice(2, eq)] = arg.slice(eq + 1);
    } else if (i + 1 < argv.length && !argv[i + 1].startsWith("--")) {
      opts[arg.slice(2)] = argv[++i];
    } else {
      opts[arg.slice(2)] = true;
    }
  }
  return opts;
}

/**
 * 本次要准备的包对应的目标平台。
 *
 * 只区分 host：tauri 不支持从 Linux 交叉编 Windows，CI 也是各自跑在对应 runner 上。
 */
function resolveTarget() {
  if (isWindows) return "win-x64";
  return "linux-x64";
}

/**
 * 本次编译的 sherpa-onnx-sys 版本。
 *
 * Cargo.lock 里是**实际解析到**的版本（我们 pin 了 `=1.13.5`，所以它就是编译用的
 * 那个）；没有 lockfile 时回退到 ling-chat-main 的 pin。
 */
function resolveVersion() {
  const lock = join(projectRoot, "Cargo.lock");
  if (existsSync(lock)) {
    const text = readFileSync(lock, "utf-8");
    const hit = /\[\[package\]\]\s*\nname = "sherpa-onnx-sys"\s*\nversion = "([^"]+)"/.exec(text);
    if (hit) return hit[1];
  }
  const manifest = join(projectRoot, "crates/ling-chat-main/Cargo.toml");
  if (existsSync(manifest)) {
    const hit = /sherpa-onnx-sys = \{ version = "=([^"]+)"/.exec(readFileSync(manifest, "utf-8"));
    if (hit) return hit[1];
  }
  throw new Error("读不到 sherpa-onnx-sys 版本（既没有 Cargo.lock，也没有 Cargo.toml 里的 pin）");
}

/** 官方资产名是否匹配「某版本 + 某 CUDA 变体 + 某平台」的 GPU 包。 */
function isCudaAsset(name, version, variant, target) {
  const major = variant.slice("cuda".length); // cuda12 -> 12
  const suffix = target === "win-x64" ? "-win-x64-cuda.tar.bz2" : "-linux-x64-gpu.tar.bz2";
  return name.startsWith(`sherpa-onnx-v${version}-cuda-${major}.`) && name.endsWith(suffix);
}

/**
 * 通过 Release API 找官方 CUDA 包（返回 `{ name, digest }`）。
 *
 * 走 API 而不是拼字符串：标签里带着 ORT / cuDNN 版本（`cudnn-9.x-onnxruntime1.27.1`），
 * 硬编码的话上游一升级就失效。顺带拿到官方 `sha256`，下载后校验（走镜像时尤其重要）。
 * API 不可用（没网 / 触发限流）时返回 null，交给调用方用「本地已有的 tar.bz2」兜底。
 */
async function findAssetFromApi(version, variant, target) {
  const url = `${RELEASE_API}/v${version}`;
  let payload;
  try {
    const res = await fetch(url, {
      headers: { accept: "application/vnd.github+json", "user-agent": "lingchat-build" },
    });
    if (!res.ok) {
      log(`Release API 返回 ${res.status}，改用本地/兜底查找（${url}）`);
      return null;
    }
    payload = await res.json();
  } catch (error) {
    log(`Release API 不可用（${error.message ?? error}），改用本地/兜底查找`);
    return null;
  }
  const assets = payload.assets ?? [];
  const hit = assets.find((a) => isCudaAsset(a.name, version, variant, target));
  if (!hit) {
    log(
      `v${version} 的 release 里没有匹配 ${variant}/${target} 的 CUDA 包（共 ${assets.length} 个资产）`,
    );
    return null;
  }
  const digest = typeof hit.digest === "string" ? hit.digest.replace(/^sha256:/, "") : null;
  return { name: hit.name, digest };
}

/** 在 SHERPA_ONNX_ARCHIVE_DIR / 缓存目录里找已下好的 tar.bz2（离线可用）。 */
function findLocalArchive(version, variant, target) {
  const dirs = [process.env.SHERPA_ONNX_ARCHIVE_DIR, prebuiltRoot].filter(Boolean);
  for (const dir of dirs) {
    if (!existsSync(dir)) continue;
    for (const name of readdirSync(dir)) {
      if (isCudaAsset(name, version, variant, target)) return join(dir, name);
    }
  }
  return null;
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/** 流式算 sha256（包有 200~400MB，不整个读进内存）。 */
async function sha256Of(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}

/** 下载到 `<dest>.part` 再改名；支持断点续传、单个 URL 失败重试、直连失败换镜像。 */
async function download(url, dest, { sha256, retries = DOWNLOAD_RETRIES } = {}) {
  if (existsSync(dest)) {
    log(`已有 ${dest}，跳过下载`);
    return dest;
  }
  mkdirSync(dirname(dest), { recursive: true });
  const part = `${dest}.part`;

  const urls = resolveDownloadUrls(url);
  for (const [index, candidate] of urls.entries()) {
    const viaMirror = index > 0;
    if (viaMirror) log(`直连失败，换镜像 ${candidate}`);
    for (let attempt = 1; attempt <= retries; attempt++) {
      // 续传：包有 200~400MB，链路不稳时从头发太浪费。服务器不支持 Range（不是 206）
      // 就丢掉半截文件重来。
      const have = existsSync(part) ? statSync(part).size : 0;
      try {
        const res = await fetch(candidate, {
          redirect: "follow",
          headers: have > 0 ? { range: `bytes=${have}-` } : {},
        });
        if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`);
        const resuming = have > 0 && res.status === 206;
        if (have > 0 && !resuming) rmSync(part, { force: true });
        const bodyLength = Number(res.headers.get("content-length") ?? 0);
        const expected = (resuming ? have : 0) + bodyLength;
        log(
          bodyLength
            ? `下载 ${candidate}${resuming ? `（续传 ${(have / 1048576).toFixed(1)}MB 起）` : ""}（${(
                bodyLength / 1048576
              ).toFixed(1)} MB，第 ${attempt} 次）`
            : `下载 ${candidate}（第 ${attempt} 次）`,
        );
        await pipeline(
          Readable.fromWeb(res.body),
          createWriteStream(part, { flags: resuming ? "a" : "w" }),
        );
        const size = statSync(part).size;
        if (expected && size !== expected) {
          throw new Error(`大小不符：期望 ${expected}，实际 ${size}`);
        }
        if (sha256) {
          const actual = await sha256Of(part);
          if (actual !== sha256) {
            throw new Error(`sha256 不符（期望 ${sha256}，实际 ${actual}）——镜像可能被改过`);
          }
          log("sha256 校验通过");
        } else if (viaMirror) {
          log("注意：官方未提供 sha256，镜像下载无法校验");
        }
        renameSync(part, dest);
        log(`下载完成 ${dest}（${(size / 1048576).toFixed(1)} MB）`);
        return dest;
      } catch (error) {
        const why = error.message ?? error;
        if (attempt === retries) {
          rmSync(part, { force: true });
          log(`第 ${attempt} 次失败（${why}）`);
          break;
        }
        log(`下载失败（${why}），${attempt}/${retries} 次后重试…`);
        await sleep(2000 * attempt);
      }
    }
  }
  throw new Error(
    [
      `下载 ${url} 失败（直连 + ${urls.length - 1} 个镜像都试过）。`,
      "  可手动下载后放进 target/sherpa-onnx-prebuilt/（或 SHERPA_ONNX_ARCHIVE_DIR）再重跑。",
    ].join("\n"),
  );
}

function run(cmd, args, { cwd } = {}) {
  log(`$ ${cmd} ${args.join(" ")}`);
  const result = spawnSync(cmd, args, { cwd, stdio: "inherit" });
  if (result.error) throw new Error(`执行 ${cmd} 失败：${result.error.message}`);
  if (result.status !== 0) throw new Error(`${cmd} 退出码 ${result.status}`);
}

/** 解压 tar.bz2 到 <prebuiltRoot>/（包内自带顶层目录）。 */
function extractArchive(archive, { force = false } = {}) {
  const stem = basename(archive).replace(/\.tar\.bz2$/, "");
  const extracted = join(prebuiltRoot, stem);
  const libDir = join(extracted, "lib");
  if (!force && existsSync(libDir)) {
    log(`已解压，复用 ${libDir}`);
    return libDir;
  }
  if (existsSync(extracted)) rmSync(extracted, { recursive: true, force: true });
  log(`解压 ${basename(archive)} -> ${prebuiltRoot}`);
  run("tar", ["-xjf", archive, "-C", prebuiltRoot]);
  if (!existsSync(libDir)) {
    throw new Error(`解压后没有 lib/ 目录：${extracted}（包结构可能变了）`);
  }
  return libDir;
}

/** 一个目录要算「一份可用的包」，至少得有这些文件（cpu 变体不需要 CUDA provider）。 */
function requiredLibsFor(variant) {
  const base = isWindows
    ? ["sherpa-onnx-c-api.dll", "onnxruntime.dll"]
    : ["libsherpa-onnx-c-api.so", "libonnxruntime.so"];
  return variant === "cpu" ? base : [...base, requiredSherpaLibs().at(-1)];
}

/** 校验 lib 目录里必需的文件齐不齐（缺了就别指望链接能过）。 */
function ensureComplete(libDir, { what, variant }) {
  const required = requiredLibsFor(variant);
  const libs = listLibs(libDir);
  const missing = required.filter((r) => !libs.includes(r));
  if (missing.length) {
    throw new Error(
      [
        `${what} 缺少必需库: ${missing.join(", ")}`,
        `  目录: ${libDir}`,
        `  实际内容: ${libs.join(", ") || "(空)"}`,
      ].join("\n"),
    );
  }
  return libDir;
}

/**
 * 准备官方预编译包（默认路径）。
 *
 * cpu 变体不需要做任何事：上游 build.rs 自己会下官方 CPU 包（static 或 shared 由
 * Cargo feature 决定），我们不掺和。
 */
async function fetchPrebuilt({ version, variant, target, force }) {
  if (variant === "cpu") {
    log("cpu 变体不需要预下载：上游 build.rs 会自己下官方 CPU 包（static/shared 由 feature 决定）");
    return null;
  }
  if (isMac) throw new Error("macOS 没有官方 CUDA 包（CoreML 在 ort 侧，不走 sherpa）");

  const asset = await findAssetFromApi(version, variant, target);
  let archive = findLocalArchive(version, variant, target);

  if (!archive) {
    if (!asset) {
      throw new Error(
        [
          `没找到 v${version} 的 ${variant} 官方包（API 不可用且本地没有），可以手动下载后重跑：`,
          `  1. 打开 ${RELEASE_TAG}/v${version}`,
          `  2. 下载 sherpa-onnx-v${version}-cuda-<…>-${
            target === "win-x64" ? "win-x64-cuda" : "linux-x64-gpu"
          }.tar.bz2`,
          `  3. 放进 ${prebuiltRoot}/（或设 SHERPA_ONNX_ARCHIVE_DIR 指向它所在目录），再跑一次本脚本`,
        ].join("\n"),
      );
    }
    log(`v${version} / ${variant} / ${target} -> ${asset.name}`);
    archive = await download(
      `${RELEASE_DOWNLOAD}/v${version}/${asset.name}`,
      join(prebuiltRoot, asset.name),
      {
        sha256: asset.digest,
      },
    );
  } else {
    log(`用本地已有的包：${archive}`);
  }
  return ensureComplete(extractArchive(archive, { force }), {
    what: `预编译包 ${basename(archive)}`,
    variant,
  });
}

/**
 * 自编走官方源码（`--from=cmake`）。
 *
 * 相比下载官方包的区别只有「sherpa 本体那层 C++ 怎么来」：GPU 侧的执行提供方
 * （CUDA EP）仍然是 ONNX Runtime 官方的预编译产物 —— sherpa 的 cmake 里
 * `SHERPA_ONNX_ENABLE_GPU=ON` 会去下 `onnxruntime-linux-x64-gpu_cuda12-<ver>-patched.zip`，
 * 和官方 CUDA 包里那份是同一个东西。所以自编的价值在「版本/编译选项自控」，
 * 不在「多出 GPU 能力」。
 *
 * 代价：要本机有 cmake + C++ 工具链，首次构建要下 ORT 包（~200MB）并编 sherpa
 * 及其内置依赖（分钟到几十分钟）；Windows 需要 MSVC（不能在 Linux 上交叉编）。
 */
async function buildFromSource({ version, variant, target, force }) {
  if (target !== "linux-x64" && !isWindows) {
    throw new Error(`自编目前只支持 linux-x64 / win-x64，当前是 ${target}`);
  }
  if (isMac) throw new Error("macOS 自编请用 Xcode 工具链手工构建，本脚本未覆盖");
  if (variant === "cuda13") {
    // sherpa 1.13.5 的 cmake 里 GPU 分支写死了 CUDA 12 的 ORT 包（见
    // cmake/onnxruntime-linux-x86_64-gpu.cmake：onnxruntime-linux-x64-gpu_cuda12-*.zip），
    // 自编 cuda13 会得到一份「CUDA 12 的库 + cuda13 的 GPU_VARIANT」，运行期探测不到
    // cuda13 的运行时后静默退回 CPU —— 不如直接在这里说清楚。要 CUDA 13 请用官方包。
    throw new Error(
      "自编不支持 cuda13：sherpa 的 cmake 里 GPU 用的 ONNX Runtime 包固定是 CUDA 12 版。" +
        "要用 CUDA 13 请走官方预编译包（不带 --from=cmake）。",
    );
  }

  for (const cmd of ["cmake"]) {
    const probe = spawnSync(cmd, ["--version"], { stdio: "ignore" });
    if (probe.error) throw new Error(`自编需要 ${cmd}，当前 PATH 里没有`);
  }

  const srcDir = join(prebuiltRoot, `sherpa-onnx-v${version}-src`);
  const srcArchive = join(prebuiltRoot, `sherpa-onnx-v${version}-src.tar.gz`);
  if (force || !existsSync(join(srcDir, "CMakeLists.txt"))) {
    if (!existsSync(srcArchive)) {
      await download(`${SOURCE_ARCHIVE}/v${version}.tar.gz`, srcArchive);
    }
    rmSync(srcDir, { recursive: true, force: true });
    mkdirSync(srcDir, { recursive: true });
    log(`解压源码 -> ${srcDir}`);
    run("tar", ["-xzf", srcArchive, "-C", srcDir, "--strip-components=1"]);
  } else {
    log(`复用已解压源码 ${srcDir}`);
  }

  const buildDir = join(srcDir, `build-${variant}`);
  const gpuArgs = variant === "cpu" ? [] : ["-DSHERPA_ONNX_ENABLE_GPU=ON"];
  if (force || !existsSync(join(buildDir, "CMakeCache.txt"))) {
    await preseedOnnxRuntime({ srcDir, buildDir, variant, target });
    const args = [
      "-B",
      buildDir,
      "-S",
      srcDir,
      "-DCMAKE_BUILD_TYPE=Release",
      // 上游 build.rs 的 shared 路径就是 `dylib=sherpa-onnx-c-api` + `dylib=onnxruntime`，
      // 所以这里必须 BUILD_SHARED_LIBS=ON。
      "-DBUILD_SHARED_LIBS=ON",
      ...gpuArgs,
      "-DSHERPA_ONNX_ENABLE_C_API=ON",
      "-DSHERPA_ONNX_ENABLE_TESTS=OFF",
      "-DSHERPA_ONNX_ENABLE_PYTHON=OFF",
      "-DSHERPA_ONNX_ENABLE_BINARY=OFF",
      "-DSHERPA_ONNX_ENABLE_JNI=OFF",
      "-DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF",
      "-DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF",
    ];
    if (spawnSync("ninja", ["--version"], { stdio: "ignore" }).status === 0) {
      args.push("-G", "Ninja");
    }
    run("cmake", args);
  } else {
    log(`复用已有构建目录 ${buildDir}`);
  }
  run("cmake", ["--build", buildDir, "-j", String(Math.max(1, cpus().length))]);

  // sherpa 的产物在 <build>/lib，ONNX Runtime 是 cmake 下载的独立包（在 build 树的
  // _deps 里），上游 build.rs 期望两者在同一个目录，所以这里汇总一份。
  const outDir = join(prebuiltRoot, `sherpa-onnx-v${version}-${variant}-cmake-libs`);
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  const collected = [];
  const wanted = (name) =>
    /^(libsherpa-onnx.*\.so(\..+)?|libonnxruntime.*\.so(\..+)?|.*\.dll)$/i.test(name);
  const walk = (dir, depth) => {
    if (depth > 4) return;
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === ".git") continue;
        walk(full, depth + 1);
      } else if (entry.isFile() && wanted(entry.name) && !statSync(full).isSymbolicLink()) {
        const dest = join(outDir, entry.name);
        if (!existsSync(dest)) {
          copyFileSync(full, dest);
          collected.push(entry.name);
        }
      }
    }
  };
  walk(buildDir, 0);
  log(`自编产物汇总到 ${outDir}（${collected.length} 个库）`);
  return ensureComplete(outDir, { what: "自编产物", variant });
}

/**
 * 预先把 ONNX Runtime 的预编译包下到 build 目录里。
 *
 * sherpa 的 cmake 用 FetchContent 从 github.com 下它（部分网络下不可达）。它的
 * `cmake/onnxruntime-*.cmake` 里列了「本地已有就直接用」的搜索路径，其中包含
 * `${CMAKE_BINARY_DIR}`，所以在 configure 之前把包按名字放进 build 目录即可；
 * 文件名与 URL 直接从它的 cmake 文件里读，避免我们这边硬编码版本。
 * cmake 自己的 `URL_HASH` 会校验完整性，所以走镜像也安全。
 */
async function preseedOnnxRuntime({ srcDir, buildDir, variant, target }) {
  const gpu = variant !== "cpu";
  const candidates =
    target === "win-x64"
      ? [gpu ? "onnxruntime-win-x64-gpu.cmake" : "onnxruntime-win-x64.cmake"]
      : [gpu ? "onnxruntime-linux-x86_64-gpu.cmake" : "onnxruntime-linux-x86_64.cmake"];
  const file = candidates.map((n) => join(srcDir, "cmake", n)).find((p) => existsSync(p));
  if (!file) {
    log(`没找到 ORT 的 cmake 文件（${candidates.join(" / ")}），交给 cmake 自己下载`);
    return;
  }
  const content = readFileSync(file, "utf-8");
  const hit = /set\(onnxruntime_URL\s+"([^"]+)"\)/.exec(content);
  if (!hit) {
    log(`没解析出 ${basename(file)} 里的 onnxruntime_URL，交给 cmake 自己下载`);
    return;
  }
  const url = hit[1];
  const dest = join(buildDir, basename(url));
  if (existsSync(dest)) {
    log(`已有 ${dest}`);
    return;
  }
  // 同一个文件里的 `set(onnxruntime_HASH "SHA256=...")` 就是 cmake 要用的校验值，
  // 顺手拿来自己验证一遍（走镜像时尤其需要）。对本地文件 cmake 未必再校验一次。
  const hash = /set\(onnxruntime_HASH\s+"SHA256=([0-9a-f]+)"/i.exec(content)?.[1];
  log(`预下 ONNX Runtime 包（${basename(url)}${hash ? "，用 cmake 里写的 URL_HASH 校验" : ""}）`);
  await download(url, dest, { sha256: hash?.toLowerCase() });
}

/**
 * 库准备主流程（供本文件的 CLI 与 scripts/package-desktop-sherpa-gpu-libs.mjs 复用）。
 *
 * 返回 `{ variant, libDir }`：`libDir` 为 null 表示**不需要**设 `SHERPA_ONNX_LIB_DIR`
 * —— 也就是 cpu 变体走官方预编译包的情况（上游 build.rs 自己会下）。
 *
 * 幂等：`SHERPA_ONNX_LIB_DIR` 已经指向一份可用的库、或缓存目录里已有同名包时直接复用。
 */
export async function ensureSherpaLibs({
  variant: requested,
  from = "prebuilt",
  force = false,
} = {}) {
  if (requested) process.env.SHERPA_ONNX_GPU = String(requested);
  if (!["prebuilt", "cmake"].includes(from)) {
    throw new Error(`--from 只支持 prebuilt / cmake，收到 ${from}`);
  }

  const variant = resolveVariant();
  if (variant !== "cpu" && !GPU_VARIANTS.includes(variant)) {
    throw new Error(`内部错误：变体 ${variant} 不认识`);
  }

  const target = resolveTarget();
  const version = resolveVersion();
  log(`sherpa-onnx v${version}，变体 ${variant}，目标 ${target}，来源 ${from}`);

  // 已经有人准备好了就直接用（也尊重手工指定的 SHERPA_ONNX_LIB_DIR）。
  const existing = process.env.SHERPA_ONNX_LIB_DIR;
  if (!force && existing && existsSync(existing)) {
    const libs = listLibs(existing);
    if (requiredLibsFor(variant).every((r) => libs.includes(r))) {
      log(`SHERPA_ONNX_LIB_DIR 已指向可用的库目录，直接复用：${existing}`);
      return { variant, libDir: existing };
    }
  }

  const libDir =
    from === "cmake"
      ? await buildFromSource({ version, variant, target, force })
      : await fetchPrebuilt({ version, variant, target, force });
  return { variant, libDir };
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const showEnv = Boolean(opts["print-env"]);
  const showLibDir = Boolean(opts["print-lib-dir"]);

  let result;
  try {
    result = await ensureSherpaLibs({
      variant: opts.variant,
      from: String(opts.from ?? "prebuilt"),
      force: Boolean(opts.force),
    });
  } catch (error) {
    fail(error.message ?? error);
  }
  const { variant, libDir } = result;

  if (!libDir) {
    // cpu + 官方预编译包：什么都不用设。
    if (showEnv) console.log("export SHERPA_ONNX_GPU=cpu");
    if (showLibDir) console.log("");
    log("无需导出 SHERPA_ONNX_LIB_DIR（cpu 变体由上游 build.rs 自行下载）");
    return;
  }

  log(`SHERPA_ONNX_LIB_DIR=${libDir}`);
  if (showLibDir) console.log(libDir);
  if (showEnv) {
    console.log(`export SHERPA_ONNX_GPU=${variant}`);
    console.log(`export SHERPA_ONNX_LIB_DIR=${libDir}`);
  }
  if (!showEnv && !showLibDir) {
    console.error(
      [
        "[sherpa-libs] 下一步：让 cargo 用上它（**必须**开 sherpa-shared，官方 CUDA 包只有动态库）：",
        `  eval "$(node scripts/fetch-sherpa-onnx-libs.mjs --variant=${variant} --print-env)"`,
        "  cargo build -p ling_chat --release --features sherpa-shared",
        "  打包：pnpm tauri:build:gpu（内部已串联这些步骤）",
      ].join("\n"),
    );
  }
}

// 直接 `node scripts/fetch-sherpa-onnx-libs.mjs` 时才跑 CLI；被 import 时不跑。
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => fail(error.message ?? error));
}
