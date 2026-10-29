// 桌面端安装包的运行时库暂存：把可执行文件真正需要的动态库放进
// src-tauri/.runtime-libs/，再由打包配置把它们塞进安装包的库搜索路径。
//
// 为什么需要它：
//   - Linux x86_64 的 ort 用 `webgpu` + `download-binaries`，可执行文件
//     DT_NEEDED 里有 `libwebgpu_dawn.so`，而这个库只躺在 target/<profile>/ 里
//     （ort `copy-dylibs` 的产物），没有任何打包配置带上它 —— deb/rpm 装完一启动
//     就是 `error while loading shared libraries: libwebgpu_dawn.so`。
//   - Linux aarch64 的 sherpa-onnx 用官方 shared CPU 包：可执行文件会依赖
//     `libsherpa-onnx-c-api.so` 和 `libonnxruntime.so`，而 ort 也通过
//     `load-dynamic` 复用同一份 ORT。这两个库必须随包，否则启动即缺库。
//   - Linux x86_64 的 sherpa 仍是静态 CPU 库，除 Dawn 外没有别的动态库要带。
//
// 落点：/usr/lib 是加载器的默认搜索目录。依赖 crate 的 build script 用
// `cargo:rustc-link-arg` 加的 rpath 只作用于该 crate 自己的 target；桌面 bin 的
// RPATH 由 src-tauri/build.rs 单独写入（$ORIGIN、$ORIGIN/../lib 等），但安装包
// 仍统一把库放到 /usr/lib，避免依赖构建机上的绝对路径。
//   - Linux：由 src-tauri/tauri.conf.json 的 bundle.linux.{deb,rpm,appimage}.files
//     把 .runtime-libs/ 映射到 /usr/lib（AppImage 里即 AppDir 的 usr/lib）。
//   - Windows：资源目录就是安装目录（exe 旁边，见 tauri.windows.conf.json 里
//     onnxruntime.dll 的映射方式）。CPU 构建下 sherpa 静态链接、不需要带库；
//     DirectML 变体（设了 SHERPA_ONNX_LIB_DIR）下 sherpa 是动态库，DLL 由这里暂存，
//     再由打包入口生成的 --config 覆盖映射到资源根目录。
//   - macOS：核心库静态链接，什么都不做。
//
// 由 tauri.conf.json 的 build.beforeBundleCommand 在每次打包前调用（cargo 编译
// 完成、bundling 之前）。

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const projectRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
export const tauriDir = join(projectRoot, "src-tauri");
/** 打包前的运行库暂存目录（单一目录：Tauri 合并配置时同 key 的 files 会互相覆盖）。 */
export const stageDir = join(tauriDir, ".runtime-libs");

const targetRoot = process.env.CARGO_TARGET_DIR || join(projectRoot, "target");

/** ort 的 WebGPU 后端（wgpu/Dawn），Linux x86_64 可执行文件直接依赖它。 */
const DAWN_LIB = "libwebgpu_dawn.so";

/** Linux aarch64：sherpa shared 包提供的运行库（ORT 也由它提供给 ort）。 */
const SHERPA_SHARED_LIB_RE = /^lib(sherpa-onnx-c-api|onnxruntime).*\.so/;

/** 属于「我们自己打包的库」，缺了就是安装包坏了（用来判定必需库是否齐）。 */
const OWN_LIB_RE = /^(libwebgpu_dawn|libsherpa-onnx|libonnxruntime)/;

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

/** macOS 核心库是静态链接，无需任何暂存。 */
function stageForMac() {
  return { staged: [], notes: ["macOS：核心库静态链接，无需暂存运行时库"] };
}

function stageForWindows() {
  // 两种形态：
  //   - CPU（默认）：sherpa 静态链接、ort load-dynamic —— onnxruntime.dll /
  //     DirectML.dll 由 tauri.windows.conf.json 从 src-tauri/binaries/ 映射，无需暂存。
  //   - DirectML 变体（SHERPA_ONNX_LIB_DIR 指向 scripts/build-desktop-sherpa-dml.mjs
  //     的产物）：sherpa 走动态链接（--features sherpa-shared），DLL 必须跟着安装包走，
  //     否则装完启动就缺库。这里把 sherpa-onnx-*.dll 拷进 .runtime-libs/，再由打包入口
  //     生成的 --config 覆盖映射到资源根目录（Windows 只在 exe 同目录找 DLL）。
  //     onnxruntime.dll / DirectML.dll 仍旧由 tauri.windows.conf.json 提供。
  rmSync(stageDir, { recursive: true, force: true });
  mkdirSync(stageDir, { recursive: true });

  const libDir = process.env.SHERPA_ONNX_LIB_DIR;
  if (!libDir) {
    return {
      staged: [],
      notes: ["Windows 构建：sherpa 静态链接、ort load-dynamic，无需额外 DLL"],
    };
  }
  if (!existsSync(libDir)) {
    throw new Error(`SHERPA_ONNX_LIB_DIR 指向的目录不存在: ${libDir}`);
  }

  const sherpaDlls = readdirSync(libDir).filter((f) => /^sherpa-onnx-.*\.dll$/i.test(f));
  if (sherpaDlls.length === 0) {
    throw new Error(
      [
        `SHERPA_ONNX_LIB_DIR=${libDir} 里没有 sherpa-onnx-*.dll。`,
        "  这个目录应该来自 scripts/build-desktop-sherpa-dml.mjs（DirectML 变体）。",
      ].join("\n"),
    );
  }
  for (const dll of sherpaDlls) {
    copyFileSync(join(libDir, dll), join(stageDir, dll));
  }
  return {
    staged: sherpaDlls,
    notes: ["Windows DirectML 变体：sherpa 动态库随包（ORT/DirectML 仍来自 src-tauri/binaries/）"],
  };
}

function stageForLinux(log) {
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

  // aarch64 的 sherpa 走 shared，ORT 也由同一份 libonnxruntime.so 提供。
  // 从可执行文件的 DT_NEEDED 出发做闭包，只带真正需要的库；不能扫 target
  // 目录里的所有 libonnxruntime*，否则旧 CUDA/TensorRT 构建残留会被误打进包。
  const pending = (needed ?? []).filter((name) => SHERPA_SHARED_LIB_RE.test(name));
  const stagedShared = new Set();
  while (pending.length) {
    const name = pending.shift();
    if (stagedShared.has(name)) continue;
    stagedShared.add(name);
    const src = findArtifact(name);
    if (!src) continue;
    copyFileSync(src, join(stageDir, name));
    staged.push(name);
    for (const dep of elfNeededLibs(src) ?? []) {
      if (SHERPA_SHARED_LIB_RE.test(dep)) pending.push(dep);
    }
  }
  if (stagedShared.size) {
    notes.push(`sherpa shared 运行库随包: ${[...stagedShared].join(", ")}`);
  }

  // ort 的 WebGPU 后端（ort-sys 会 emit `rustc-link-lib=webgpu_dawn`）。它是否留在
  // 可执行文件的 DT_NEEDED 里取决于链接器怎么处理「没被直接引用的动态库」：本机实测
  // onnxruntime 静态链接时有些工具链会把它丢掉；运行期 WebGPU EP 也可能按名字
  // dlopen 它。所以只要 target 下有就一并带上，只有可执行文件明确依赖它、
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

  const missing = missingOwnLibs(binary, staged);
  if (missing.length) {
    throw new Error(
      [
        `以下库被可执行文件（或它的依赖）引用，但没有随安装包带上: ${missing.join(", ")}`,
        `  可执行文件: ${binary ?? "(未找到)"}`,
        `  暂存目录: ${stageDir}（打包后会落到安装包的 /usr/lib）`,
        "  最常见的原因是 target 下的可执行文件是上一次其它配置构建的产物，重新编译一次即可；",
        "  否则请在本脚本里把缺的库补进暂存。",
      ].join("\n"),
    );
  }

  return { staged, notes };
}

/** 暂存本次构建需要的运行时库；返回 `{ staged, notes }`。 */
export function stageRuntimeLibs({ log = () => {} } = {}) {
  const platform = process.platform;
  if (platform === "darwin") return stageForMac();
  if (platform === "win32") return stageForWindows();
  return stageForLinux(log);
}

function main() {
  // 移动端的动态库由 Gradle/Xcode 打包，与这里无关（Android 的 .so 由
  // package-android-sherpa-libs.mjs 负责）；顺手跳过，免得白复制一份。
  const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
  const platform = process.env.TAURI_ENV_PLATFORM || "";
  if (triple.includes("android") || platform === "android" || platform === "ios") {
    console.log("[stage-libs] 移动端构建，跳过桌面运行时库暂存");
    return;
  }

  const { staged, notes } = stageRuntimeLibs({
    log: (msg) => console.log(`[stage-libs] ${msg}`),
  });

  for (const note of notes) console.log(`[stage-libs] ${note}`);
  console.log(
    `[stage-libs] ${staged.length} 个库已暂存到 ${stageDir}` +
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
