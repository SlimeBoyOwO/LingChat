// Sherpa-ONNX「构建时下源码 + 打补丁」的公共逻辑。
//
// 两份自编脚本共用：
//   - scripts/build-desktop-sherpa-dml.mjs   （Windows DirectML，桌面）
//   - scripts/build-android-sherpa-nnapi.mjs （Android NNAPI）
//
// 共用同一条原则：仓库里**不**存改过的 sherpa 源码树，只在构建时下 tarball、解压到
// target/ 缓存、按 patches/sherpa-onnx/*.patch 顺序打补丁（补丁内容变化会自动让缓存
// 失效）。之所以必须在 cargo 之前做这件事：cargo 的 build script 只能把信息传给依赖它
// 的包，没法把自己的环境变量注入给依赖 crate 的 build.rs，而上游 sherpa-onnx-sys 只认
// `SHERPA_ONNX_LIB_DIR`（见 docs/inference-devices.md §9）。

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  createWriteStream,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { pipeline } from "node:stream/promises";

export const SHERPA_SOURCE_ARCHIVE = "https://github.com/k2-fsa/sherpa-onnx/archive/refs/tags";

/// GitHub 直连在国内常超时，下载按「直连 → 镜像」退避；`SHERPA_ONNX_DOWNLOAD_MIRROR`
/// 可指定单个前缀或 `none` 关闭。非 github.com 的 URL（如 nuget.org）不走镜像。
export const DEFAULT_MIRRORS = [
  "https://ghfast.top",
  "https://gh-proxy.com",
  "https://ghproxy.net",
];

/** 把 github.com 的 URL 展开成「直连 + 镜像」的候选列表。 */
export function resolveDownloadUrls(url) {
  if (!url.includes("github.com")) return [url];
  const override = (process.env.SHERPA_ONNX_DOWNLOAD_MIRROR ?? "").trim();
  let mirrors;
  if (override === "none") mirrors = [];
  else if (override) mirrors = [override.replace(/\/+$/, "")];
  else mirrors = DEFAULT_MIRRORS;
  return [url, ...mirrors.map((m) => `${m}/${url}`)];
}

/** 逐个候选源下载到 dest，全失败才抛错。 */
export async function download(url, dest, log) {
  mkdirSync(dirname(dest), { recursive: true });
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
      log?.(`下载失败（${candidate}）: ${error.message}；换下一个源`);
    }
  }
  throw new Error(`下载 ${url} 失败：${lastError?.message ?? lastError}`);
}

/** 同步执行命令，非 0 退出即抛错。 */
export function run(cmd, cmdArgs, opts = {}) {
  const result = spawnSync(cmd, cmdArgs, { stdio: "inherit", ...opts });
  if (result.error) throw new Error(`执行 ${cmd} 失败：${result.error.message}`);
  if (result.status !== 0) throw new Error(`${cmd} 退出码 ${result.status}`);
}

/// 编译并行度（线程数），按优先级：
///   1. `SHERPA_BUILD_JOBS`（本仓库约定）
///   2. `CMAKE_BUILD_PARALLEL_LEVEL`（cmake 自己的环境变量，`--parallel` 不带数值时它生效）
///   3. 都没设 → null，交给 cmake 默认（= 机器核数）
/// 值写成 `auto` 等价于没设；写了但不是正整数则直接报错（免得静默用默认值编半天）。
/// 大内存机器上编 sherpa 时可以调小（每个 clang 进程峰值可到 1~2 GB）。
export function resolveBuildJobs() {
  for (const name of ["SHERPA_BUILD_JOBS", "CMAKE_BUILD_PARALLEL_LEVEL"]) {
    const raw = (process.env[name] ?? "").trim();
    if (!raw || raw.toLowerCase() === "auto") continue;
    const jobs = Number.parseInt(raw, 10);
    if (!Number.isFinite(jobs) || jobs <= 0 || String(jobs) !== raw) {
      throw new Error(`${name}="${raw}" 不是正整数（留空或 auto 表示按机器核数）`);
    }
    return jobs;
  }
  return null;
}

/// `cmake --build` 的并行参数：设了并行度就显式传数值，否则交给 cmake 的默认。
export function parallelArgs(jobs) {
  return jobs ? ["--parallel", String(jobs)] : ["--parallel"];
}

/// 要编的版本 = Cargo.lock 里 sherpa-onnx-sys 的锁定版本（也就是真会被编的版本）。
/// 拿不到就直接报错：宁可明确失败，也不要编出一份与运行时不同的库。
export function resolveSherpaVersion(projectRoot) {
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
export function patchFingerprint(patchesDir) {
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

/** 读/写某个缓存目录里的 state.json（记录 version / patchHash 等）。 */
export function readCacheState(cacheRoot) {
  try {
    return JSON.parse(readFileSync(join(cacheRoot, "state.json"), "utf-8"));
  } catch {
    return null;
  }
}

export function writeCacheState(cacheRoot, state) {
  mkdirSync(cacheRoot, { recursive: true });
  writeFileSync(join(cacheRoot, "state.json"), `${JSON.stringify(state, null, 2)}\n`);
}

/// 按文件名顺序把 patches/sherpa-onnx/*.patch 打到新鲜源码上。
///
/// 这就是「构建时下载源码并修改」里「修改」的那一半：仓库只保存 diff，改的是临时源码树。
/// 优先用 git apply，没有 git 时退回 GNU patch。
export function applyPatches(srcDir, patchesDir, log) {
  if (!existsSync(patchesDir)) return;
  const patches = readdirSync(patchesDir)
    .filter((f) => f.endsWith(".patch"))
    .sort();
  if (patches.length === 0) return;
  for (const patch of patches) {
    const file = join(patchesDir, patch);
    log(`打补丁 ${patch}`);
    const viaGit = spawnSync("git", ["apply", "-p1", file], { cwd: srcDir, stdio: "inherit" });
    if (viaGit.status === 0) continue;
    const viaPatch = spawnSync("patch", ["-p1", "-i", file], { cwd: srcDir, stdio: "inherit" });
    if (viaPatch.status !== 0) throw new Error(`补丁 ${patch} 打不上（git apply / patch 都失败）`);
  }
}

/// 把源码下下来、解压、打补丁。返回源码目录。已有可用源码树且非 force 时直接复用。
///
/// 解压完成后会把 `{version, patchHash}` 记进 `<cacheRoot>/state.json`（与调用方写入的
/// 其它字段合并）。补丁集合变了 → 指纹变化 → 下次重新解压重打，避免「旧源码树 + 新
/// 补丁」这种半新半旧的状态。
export async function ensureSource({ cacheRoot, patchesDir, version, patchHash, force, log }) {
  const srcDir = join(cacheRoot, `sherpa-onnx-v${version}-src`);
  const srcArchive = join(cacheRoot, `sherpa-onnx-v${version}-src.tar.gz`);
  const ready = existsSync(join(srcDir, "CMakeLists.txt"));
  const state = readCacheState(cacheRoot);

  if (ready && state?.patchHash === patchHash && !force) {
    log(`复用已解压源码 ${srcDir}（补丁指纹 ${patchHash}）`);
    return srcDir;
  }

  if (force || !existsSync(srcArchive)) {
    log(`⬇️  源码 ${SHERPA_SOURCE_ARCHIVE}/v${version}.tar.gz`);
    await download(`${SHERPA_SOURCE_ARCHIVE}/v${version}.tar.gz`, srcArchive, log);
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

  applyPatches(srcDir, patchesDir, log);
  writeCacheState(cacheRoot, { ...(state ?? {}), version, patchHash });
  return srcDir;
}
