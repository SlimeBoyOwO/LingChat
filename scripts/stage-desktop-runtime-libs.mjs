// 桌面端安装包的运行时库暂存：把可执行文件真正需要的动态库放进
// src-tauri/.runtime-libs/，再由打包配置把它们塞进安装包的库搜索路径。
//
// 为什么需要它：
//   - Linux x86_64 的 ort 用 `webgpu` + `download-binaries`，可执行文件
//     DT_NEEDED 里有 `libwebgpu_dawn.so`，而这个库只躺在 target/<profile>/ 里
//     （ort `copy-dylibs` 的产物），没有任何打包配置带上它 —— deb/rpm 装完一启动
//     就是 `error while loading shared libraries: libwebgpu_dawn.so`。
//   - CUDA 变体（SHERPA_ONNX_GPU=cuda12|cuda13）走 shared 链接（官方 CUDA 包
//     只提供动态库），同理必须跟着安装包走。库由 scripts/fetch-sherpa-onnx-libs.mjs
//     下载到 target/sherpa-onnx-prebuilt/ 并通过 SHERPA_ONNX_LIB_DIR 交给 cargo。
//
// 落点：/usr/lib 是加载器的默认搜索目录。实测 cargo 产出的可执行文件里没有
// RPATH/RUNPATH（依赖 crate 的 build script 用 `cargo:rustc-link-arg` 加的
// 那套只作用于该 crate 自己的 target），所以「把库放在可执行文件旁边」在 Linux
// 上行不通。
//   - Linux：由 src-tauri/tauri.conf.json 的 bundle.linux.{deb,rpm,appimage}.files
//     把 .runtime-libs/ 映射到 /usr/lib（AppImage 里即 AppDir 的 usr/lib）。
//   - Windows：资源目录就是安装目录（exe 旁边，见 tauri.windows.conf.json 里
//     onnxruntime.dll 的映射方式）。CUDA 变体的 DLL 由
//     scripts/package-desktop-sherpa-gpu-libs.mjs 生成 --config 覆盖逐文件映射到
//     资源根目录；CPU 构建是 sherpa static + ort load-dynamic，不需要带任何 DLL。
//   - macOS：没有官方 CUDA 包，核心库静态链接，什么都不做。
//
// 由 tauri.conf.json 的 build.beforeBundleCommand 在每次打包前调用（cargo 编译
// 完成、bundling 之前，环境变量与 tauri CLI 一致，所以 SHERPA_ONNX_GPU 与本次
// 构建用的 sherpa-onnx 是同一个）。

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
export const tauriDir = join(projectRoot, "src-tauri");
/** 打包前的运行库暂存目录（单一目录：Tauri 合并配置时同 key 的 files 会互相覆盖）。 */
export const stageDir = join(tauriDir, ".runtime-libs");

const targetRoot = process.env.CARGO_TARGET_DIR || join(projectRoot, "target");
/** 官方预编译包的解压缓存（上游 build.rs 也用这个目录，两边共用）。 */
export const prebuiltRoot = join(targetRoot, "sherpa-onnx-prebuilt");

/** ort 的 WebGPU 后端（wgpu/Dawn），Linux x86_64 可执行文件直接依赖它。 */
const DAWN_LIB = "libwebgpu_dawn.so";

/** GPU 变体取值（scripts/fetch-sherpa-onnx-libs.mjs / 打包脚本共用同一套）。 */
export const GPU_VARIANTS = ["cuda12", "cuda13"];

/** 属于「我们自己打包的库」，缺了就是安装包坏了（用来判定必需库是否齐）。 */
const OWN_LIB_RE = /^(libwebgpu_dawn|libsherpa-onnx|libonnxruntime)/;

const isWindows = process.platform === "win32";
const libPattern = isWindows ? /\.dll$/i : /\.so(\..+)?$/;
// 官方包里带了命令行工具用的 cargs.dll，与应用无关。
const skipLibs = new Set(["cargs.dll"]);

/**
 * 本次构建用的 GPU 变体。
 *
 * 取值就是 `SHERPA_ONNX_GPU`（别名 `cuda`/`12`/`cuda-12` 等同 `cuda12`）。同一个
 * 变量还会被 `crates/ling-chat-main/build.rs`（编译期常量 `GPU_VARIANT`）和
 * `scripts/fetch-sherpa-onnx-libs.mjs`（决定下哪个预编译包）读取，三处别名表必须
 * 一致 —— 否则会出现「依赖按 CUDA 编、打包按 CPU 处理」这种半截状态。非法取值报错。
 */
export function resolveVariant(raw = process.env.SHERPA_ONNX_GPU) {
  const variant = (raw || "").trim().toLowerCase();
  if (["", "0", "off", "false", "cpu", "none"].includes(variant)) return "cpu";
  if (["12", "cuda", "cuda12", "cuda-12"].includes(variant)) return "cuda12";
  if (["13", "cuda13", "cuda-13"].includes(variant)) return "cuda13";
  throw new Error(`SHERPA_ONNX_GPU 取值无法识别: ${raw}（可用: cpu / cuda12 / cuda13）`);
}

/** 列出某个目录下的动态库（跳过无关注释过的库）。 */
export function listLibs(dir) {
  if (!existsSync(dir)) return [];
  try {
    return readdirSync(dir).filter((f) => libPattern.test(f) && !skipLibs.has(f.toLowerCase()));
  } catch {
    return [];
  }
}

/**
 * 找到可执行文件所在目录，用来读 DT_NEEDED。
 *
 * 优先 tauri CLI 传进来的 profile/triple（TAURI_ENV_*），拿不到时退回
 * 「target 下任意一层/两层里最新的同名文件」。
 */
function findArtifact(name) {
  const profile =
    process.env.TAURI_ENV_PROFILE || (process.env.TAURI_ENV_DEBUG === "true" ? "debug" : "release");
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
  const preferred = [
    triple ? join(targetRoot, triple, profile) : null,
    join(targetRoot, profile),
  ].filter(Boolean);
  for (const dir of preferred) {
    const candidate = join(dir, name);
    if (existsSync(candidate)) return candidate;
  }

  const hits = [];
  const walk = (dir, depth) => {
    if (depth > 2) return;
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      if (entry.name === "sherpa-onnx-prebuilt" || entry.name.startsWith(".")) continue;
      const full = join(dir, entry.name);
      const candidate = join(full, name);
      if (existsSync(candidate)) hits.push(candidate);
      walk(full, depth + 1);
    }
  };
  walk(targetRoot, 0);
  hits.sort((a, b) => mtime(b) - mtime(a));
  return hits[0] ?? null;
}

function mtime(path) {
  try {
    return statSync(path).mtimeMs;
  } catch {
    return 0;
  }
}

/**
 * 读 ELF 可执行文件的 DT_NEEDED（动态段里写死的依赖库名）。
 *
 * 纯 JS 解析，不依赖 readelf（别的机器上不一定有）。只做三件事：找到 PT_DYNAMIC
 * → 收集 DT_NEEDED 的字符串下标 → 在 DT_STRTAB 字符串表里按名取值（字符串表地址
 * 经 PT_LOAD 换算成文件偏移）。解析失败返回 null，调用方按「可能有依赖」处理。
 */
export function elfNeededLibs(file) {
  let buf;
  try {
    buf = readFileSync(file);
  } catch {
    return null;
  }
  if (buf.length < 64 || buf[0] !== 0x7f || buf.toString("ascii", 1, 4) !== "ELF") return null;
  const is64 = buf[4] === 2;
  // 我们的目标平台都是小端；大端直接放弃解析。
  if (buf[5] !== 1) return null;

  const u16 = (off) => buf.readUInt16LE(off);
  const u32 = (off) => buf.readUInt32LE(off);
  const u64 = (off) => Number(buf.readBigUInt64LE(off));

  const phoff = is64 ? u64(0x20) : u32(0x1c);
  const phentsize = u16(is64 ? 0x36 : 0x2a);
  const phnum = u16(is64 ? 0x38 : 0x2c);

  const loads = [];
  let dynamic = null;
  for (let i = 0; i < phnum; i++) {
    const ph = phoff + i * phentsize;
    const type = u32(ph);
    if (type === 1) {
      // PT_LOAD：vaddr → 文件偏移的换算表
      loads.push({
        offset: is64 ? u64(ph + 8) : u32(ph + 4),
        vaddr: is64 ? u64(ph + 16) : u32(ph + 8),
        filesz: is64 ? u64(ph + 32) : u32(ph + 16),
      });
    } else if (type === 2) {
      // PT_DYNAMIC
      dynamic = {
        offset: is64 ? u64(ph + 8) : u32(ph + 4),
        filesz: is64 ? u64(ph + 32) : u32(ph + 16),
      };
    }
  }
  if (!dynamic) return null;

  const vaddrToOffset = (vaddr) => {
    for (const seg of loads) {
      if (vaddr >= seg.vaddr && vaddr < seg.vaddr + seg.filesz) {
        return seg.offset + (vaddr - seg.vaddr);
      }
    }
    return null;
  };

  const step = is64 ? 16 : 8;
  let strtabVaddr = null;
  const needed = [];
  for (let off = dynamic.offset; off + step <= dynamic.offset + dynamic.filesz; off += step) {
    const tag = is64 ? u64(off) : u32(off);
    const val = is64 ? u64(off + 8) : u32(off + 4);
    if (tag === 0) break; // DT_NULL
    if (tag === 5) {
      strtabVaddr = val; // DT_STRTAB
    } else if (tag === 1) {
      needed.push(val); // DT_NEEDED
    }
  }
  if (strtabVaddr === null) return null;
  const strtab = vaddrToOffset(strtabVaddr);
  if (strtab === null) return null;

  return needed.map((rel) => {
    const start = strtab + rel;
    let end = start;
    while (end < buf.length && buf[end] !== 0) end++;
    return buf.toString("utf8", start, end);
  });
}

/** 在 target/sherpa-onnx-prebuilt 下递归找含必需库的目录（优先路径里带 cuda 的）。 */
function findCacheDirs(requiredLibs) {
  const found = [];
  if (!existsSync(prebuiltRoot)) return found;
  const stack = [prebuiltRoot];
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

/**
 * CUDA 变体运行库的必需文件：链接期写进 DT_NEEDED 的库 + 运行期由 ONNX Runtime
 * 加载的执行提供方（provider）。校验「一个目录是不是一份可用的 CUDA 包」用。
 */
export function requiredSherpaLibs() {
  return isWindows
    ? ["sherpa-onnx-c-api.dll", "onnxruntime.dll", "onnxruntime_providers_cuda.dll"]
    : ["libsherpa-onnx-c-api.so", "libonnxruntime.so", "libonnxruntime_providers_cuda.so"];
}

/**
 * CUDA 变体运行库的来源目录。
 *
 * 解析顺序与 scripts/fetch-sherpa-onnx-libs.mjs 一致：先 `SHERPA_ONNX_LIB_DIR`
 * （fetch 脚本导出的就是这个），再 `target/sherpa-onnx-prebuilt/` 缓存。
 *
 * 抛错而不是返回 null：调用方（GPU 打包入口）希望尽早失败。
 */
export function resolveSherpaSource(variant) {
  if (!GPU_VARIANTS.includes(variant)) {
    throw new Error(`resolveSherpaSource 只接受 ${GPU_VARIANTS.join(" / ")}，收到 ${variant}`);
  }
  const required = requiredSherpaLibs();

  const fromEnv = process.env.SHERPA_ONNX_LIB_DIR;
  if (fromEnv) {
    const libs = listLibs(fromEnv);
    if (required.every((r) => libs.includes(r))) return fromEnv;
    console.warn(`[stage-libs] SHERPA_ONNX_LIB_DIR 存在但缺少必需库，忽略并回落到缓存: ${fromEnv}`);
  }
  const cached = findCacheDirs(required)[0];
  if (cached) return cached;

  throw new Error(
    [
      `找不到 CUDA 版 Sherpa-ONNX 运行库（缺 ${required.join(", ")}）。`,
      `  已查找: SHERPA_ONNX_LIB_DIR=${process.env.SHERPA_ONNX_LIB_DIR || "(未设置)"}`,
      `          ${prebuiltRoot}`,
      "  先跑一次下载（版本按 Cargo.lock 里 pin 的 sherpa-onnx-sys 自动取）：",
      `    node scripts/fetch-sherpa-onnx-libs.mjs --variant=${variant}`,
      "  或把 SHERPA_ONNX_LIB_DIR 指向已解压包的 lib/ 目录。",
    ].join("\n"),
  );
}

function stageLibsFrom(source) {
  const staged = [];
  for (const lib of listLibs(source)) {
    const src = join(source, lib);
    // 官方包里 libonnxruntime.so 可能是指向带版本号文件的符号链接，一并解决。
    if (!statSync(src).isFile()) continue;
    copyFileSync(src, join(stageDir, basename(lib)));
    staged.push(basename(lib));
  }
  return staged;
}

/** 检查可执行文件与已暂存的库，列出「我们自己打包但还缺」的库。 */
function missingOwnLibs(binary, stagedFiles) {
  const present = new Set(stagedFiles);
  const missing = new Set();
  const check = (file) => {
    for (const name of elfNeededLibs(file) ?? []) {
      if (OWN_LIB_RE.test(name) && !present.has(name)) missing.add(name);
    }
  };
  if (binary) check(binary);
  for (const lib of present) check(join(stageDir, lib));
  return [...missing];
}

/** macOS 没有官方 CUDA 包，核心库也是静态链接，无需任何暂存。 */
function stageForMac() {
  return { staged: [], notes: ["macOS：核心库静态链接，无需暂存运行时库"] };
}

function stageForWindows(variant) {
  if (variant === "cpu") {
    // sherpa static + ort load-dynamic：onnxruntime.dll 由 tauri.windows.conf.json
    // 从 src-tauri/binaries/ 映射（缺它时本地推理优雅降级），CUDA 库不需要。
    return {
      staged: [],
      notes: ["Windows CPU 构建：sherpa 静态链接、ort load-dynamic，无需额外 DLL"],
    };
  }
  rmSync(stageDir, { recursive: true, force: true });
  mkdirSync(stageDir, { recursive: true });
  const source = resolveSherpaSource(variant);
  const staged = stageLibsFrom(source);
  return {
    staged,
    notes: [`Windows CUDA 变体：${staged.length} 个 DLL 来自 ${source}`],
  };
}

function stageForLinux(variant, log) {
  // 目录必须先建出来：tauri-bundler 的 copy_dir 对不存在的源目录直接报错
  // （空目录则等于「不带任何文件」，是合法且期望的——比如 aarch64 不需要 Dawn）。
  rmSync(stageDir, { recursive: true, force: true });
  mkdirSync(stageDir, { recursive: true });

  const staged = [];
  const notes = [];

  const binary = findArtifact("ling_chat");
  const needed = binary ? elfNeededLibs(binary) : null;
  if (binary && needed) {
    const own = needed.filter((n) => OWN_LIB_RE.test(n));
    if (own.length) log(`可执行文件 ${binary} 直接依赖: ${own.join(", ")}`);
  }

  // ort 的 WebGPU 后端（ort-sys 会 emit `rustc-link-lib=webgpu_dawn`）。它是否留在
  // 可执行文件的 DT_NEEDED 里取决于链接器怎么处理「没被直接引用的动态库」：本机实测
  // CUDA 变体（onnxruntime 是 .so，整张未定义符号表都要满足）会写进 DT_NEEDED，
  // 而 CPU 变体（onnxruntime 静态）在有些工具链下会被丢掉；运行期 WebGPU EP 也可能
  // 按名字 dlopen 它。所以只要 target 下有就一并带上，只有可执行文件明确依赖它、
  // 却找不到时才算错误。
  const dawn = findArtifact(DAWN_LIB);
  const needsDawn = needed ? needed.includes(DAWN_LIB) : true;
  if (dawn) {
    copyFileSync(dawn, join(stageDir, DAWN_LIB));
    staged.push(DAWN_LIB);
    if (!needsDawn) {
      notes.push(`补带 ${DAWN_LIB}（不在 DT_NEEDED 里，但 ort 的 webgpu 后端可能需要）`);
    }
  } else if (needsDawn) {
    throw new Error(
      [
        `可执行文件依赖 ${DAWN_LIB}，但在 target/ 下找不到它。`,
        "  它由 ort 的 `webgpu` feature + `copy-dylibs` 产出（Linux x86_64 专用）。",
        "  检查 crates/ling-chat-main/Cargo.toml 里的 ort features，并确认 cargo 编译已完成。",
      ].join("\n"),
    );
  } else {
    notes.push(`target/ 下没有 ${DAWN_LIB}（非 x86_64 的 Linux 构建），跳过`);
  }

  if (variant !== "cpu") {
    const source = resolveSherpaSource(variant);
    const fromSource = stageLibsFrom(source);
    staged.push(...fromSource);
    notes.push(`CUDA 变体：${fromSource.length} 个库来自 ${source}`);
  }

  const missing = missingOwnLibs(binary, staged);
  if (missing.length) {
    throw new Error(
      [
        `以下库被可执行文件（或它的依赖）引用，但没有随安装包带上: ${missing.join(", ")}`,
        `  可执行文件: ${binary ?? "(未找到)"}`,
        `  暂存目录: ${stageDir}（打包后会落到安装包的 /usr/lib）`,
        "  最常见的原因是 target 下的可执行文件是上一次「另一种变体」构建的产物",
        "  （比如刚用 cuda12 编过、接着跑 CPU 构建），重新编译一次即可；",
        "  否则请在本脚本里把缺的库补进暂存。",
      ].join("\n"),
    );
  }

  return { staged, notes };
}

/**
 * 暂存本次构建需要的运行时库；返回 `{ staged, notes }`。
 *
 * 由 beforeBundleCommand 调用（CLI），也被 package-desktop-sherpa-gpu-libs.mjs 复用。
 */
export function stageRuntimeLibs({ variant = resolveVariant(), log = () => {} } = {}) {
  const platform = process.platform;
  if (platform === "darwin") return stageForMac();
  if (platform === "win32") return stageForWindows(variant);
  return stageForLinux(variant, log);
}

function main() {
  // 移动端的动态库由 Gradle 打包，与这里无关（Android 的 .so 由
  // package-android-sherpa-libs.mjs 负责）；顺手跳过，免得白复制一份。
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
  const platform = process.env.TAURI_ENV_PLATFORM || "";
  if (triple.includes("android") || platform === "android" || platform === "ios") {
    console.log("[stage-libs] 移动端构建，跳过桌面运行时库暂存");
    return;
  }

  const variant = resolveVariant();
  const { staged, notes } = stageRuntimeLibs({
    variant,
    log: (msg) => console.log(`[stage-libs] ${msg}`),
  });

  for (const note of notes) console.log(`[stage-libs] ${note}`);
  console.log(
    `[stage-libs] variant=${variant}，${staged.length} 个库已暂存到 ${stageDir}` +
      (staged.length ? `: ${staged.join(", ")}` : ""),
  );
}

// 直接 `node scripts/stage-desktop-runtime-libs.mjs` 时才执行；被 import 时不执行。
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(`[stage-libs] ${error.message ?? error}`);
    process.exit(1);
  }
}
