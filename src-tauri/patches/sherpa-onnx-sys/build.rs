//! sherpa-onnx 预编译库的下载与链接脚本。
//!
//! 本文件是 crates.io 上 sherpa-onnx-sys 1.13.5 上游 build.rs 的 vendored 副本，
//! 与本仓库的差异只有一处：**支持按需下载 GPU（CUDA）版预编译包**。
//!
//! 上游按平台名拼出唯一的 CPU 版包名；这里新增环境变量 `SHERPA_ONNX_GPU`：
//!
//! | `SHERPA_ONNX_GPU` | 用途 | 支持的平台 |
//! | --- | --- | --- |
//! | 未设置 / `cpu` / `0` | 上游行为：CPU 版包（默认） | 全部上游支持的平台 |
//! | `cuda12` | 官方 CUDA 12.x + cuDNN 9 版包 | linux-x64、win-x64 |
//! | `cuda13` | 官方 CUDA 13.x + cuDNN 9 版包 | linux-x64、win-x64 |
//!
//! 注意：官方 CUDA 包只含动态库（`lib/*.so` / `lib/*.dll`），因此启用 GPU 时会强制
//! 走 shared 链接，即使 Cargo 里打开的是 `static` feature（会打一条 cargo:warning）。
//! 运行期还需要 NVIDIA 驱动 + 对应版本的 CUDA/cuDNN 运行时：
//!
//! - **CPU 包**：没编 `SHERPA_ONNX_ENABLE_GPU`，sherpa-onnx 自己会打
//!   `Please compile with -DSHERPA_ONNX_ENABLE_GPU=ON ... Fallback to cpu!` 并回退 CPU；
//! - **CUDA 包**：环境不齐（实测缺 CUDA 运行时时）请求 `cuda` 会由 ONNX Runtime 抛 C++
//!   异常 —— 该异常穿不过 FFI，Rust 侧 catch 不到，直接 abort 整个进程。所以用 CUDA 包
//!   时，调用方必须先确认运行时齐备再传 `provider="cuda"`。
//!
//! 其它平台（macOS / Android / linux-aarch64）官方没有对应 CUDA 包，设置该变量会直接
//! 报错。这些平台要用 GPU 只能自行编译，然后用 `SHERPA_ONNX_LIB_DIR` 指向产物目录。
//! 包名里带 ORT 版本号（`onnxruntime1.27.1`），升级 sherpa-onnx 版本时需要核对上游
//! release 是否仍用同一命名。

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::{collections::HashSet, ffi::OsString};

use bzip2::read::BzDecoder;
use tar::Archive;

const RELEASE_BASE_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download";
const SHERPA_ONNX_STATIC_LIBS: &[&str] = &[
    "sherpa-onnx-c-api",
    "sherpa-onnx-core",
    "kaldi-decoder-core",
    "sherpa-onnx-kaldifst-core",
    "sherpa-onnx-fstfar",
    "sherpa-onnx-fst",
    "kaldi-native-fbank-core",
    "kissfft-float",
    "piper_phonemize",
    "espeak-ng",
    "ucd",
    "onnxruntime",
    "ssentencepiece_core",
];

type DynError = Box<dyn Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinkMode {
    Static,
    Shared,
}

/// 预编译包的算力变体。上游只有 CPU 一种，本仓库补充了两种 CUDA 变体。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GpuVariant {
    Cpu,
    Cuda12,
    Cuda13,
}

impl GpuVariant {
    /// 读取环境变量 `SHERPA_ONNX_GPU`（缺省 = CPU）。
    fn from_env() -> Result<Self, DynError> {
        let raw = env::var("SHERPA_ONNX_GPU").unwrap_or_default();
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "0" | "off" | "false" | "cpu" | "none" => Ok(Self::Cpu),
            "12" | "cuda" | "cuda12" | "cuda-12" => Ok(Self::Cuda12),
            "13" | "cuda13" | "cuda-13" => Ok(Self::Cuda13),
            other => Err(format!(
                "SHERPA_ONNX_GPU 取值无法识别: `{other}`（可选：cpu / cuda12 / cuda13）"
            )
            .into()),
        }
    }

    fn is_gpu(self) -> bool {
        self != Self::Cpu
    }

    /// 官方 CUDA 包名中间的标签（CUDA/cuDNN/ONNX Runtime 版本组合）。
    fn archive_tag(self) -> Result<&'static str, DynError> {
        match self {
            Self::Cuda12 => Ok("cuda-12.x-cudnn-9.x-onnxruntime1.27.1"),
            Self::Cuda13 => Ok("cuda-13.x-cudnn-9.x-onnxruntime1.27.1"),
            Self::Cpu => {
                Err("CPU 变体没有 CUDA 标签（内部错误：archive_tag 不应作用于 Cpu）".into())
            },
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda12 => "cuda12",
            Self::Cuda13 => "cuda13",
        }
    }
}

fn main() {
    if let Err(err) = try_main() {
        panic!("{err}");
    }
}

fn try_main() -> Result<(), DynError> {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_ARCHIVE_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_GPU");
    println!("cargo:rerun-if-env-changed=DOCS_RS");

    if env::var_os("DOCS_RS").is_some() {
        // docs.rs sets DOCS_RS=1; skip downloading/linking native libraries
        // so that `cargo doc` can succeed without the real C artifacts.
        return Ok(());
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS")?;
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")?;
    let gpu = GpuVariant::from_env()?;
    let link_mode = resolve_link_mode(gpu)?;
    let lib_dir = resolve_lib_dir(link_mode, &target_os, &target_arch, gpu)?;

    // 供 Rust 侧读取（`sherpa_onnx_sys::GPU_VARIANT`）：调用方据此决定要不要真的请求
    // GPU provider —— CUDA 包里 provider 会被交给 ONNX Runtime，加载失败会 abort 进程。
    println!("cargo:rustc-env=SHERPA_ONNX_GPU_VARIANT={}", gpu.label());

    if gpu.is_gpu() {
        eprintln!(
            "sherpa-onnx: 使用 {} 预编译包（{} 链接）",
            gpu.label(),
            match link_mode {
                LinkMode::Static => "static",
                LinkMode::Shared => "shared",
            }
        );
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    if link_mode == LinkMode::Shared && matches!(target_os.as_str(), "linux" | "macos" | "android")
    {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
        emit_relative_rpath(&target_os);
        copy_unix_runtime_libs(&lib_dir, &target_os)?;
    }

    if link_mode == LinkMode::Shared && target_os == "windows" {
        copy_windows_runtime_dlls(&lib_dir)?;
    }

    match link_mode {
        LinkMode::Static => emit_static_link_directives(&target_os),
        LinkMode::Shared => emit_shared_link_directives(),
    }

    Ok(())
}

fn resolve_link_mode(gpu: GpuVariant) -> Result<LinkMode, DynError> {
    let static_enabled = env::var_os("CARGO_FEATURE_STATIC").is_some();
    let shared_enabled = env::var_os("CARGO_FEATURE_SHARED").is_some();

    if static_enabled && shared_enabled {
        return Err("Features `static` and `shared` cannot be enabled at the same time".into());
    }

    // 官方 CUDA 包只提供动态库，启用 GPU 时统一按 shared 处理；两种情况下的
    // Rust 侧代码完全相同（C API 不变），所以这里覆盖 feature 是安全的。
    if gpu.is_gpu() {
        if static_enabled {
            println!(
                "cargo:warning=SHERPA_ONNX_GPU={} 已启用，但 `static` feature 同时打开；\
                 CUDA 预编译包只含动态库，已改为 shared 链接。",
                gpu.label()
            );
        }
        return Ok(LinkMode::Shared);
    }

    if shared_enabled {
        Ok(LinkMode::Shared)
    } else {
        Ok(LinkMode::Static)
    }
}

fn resolve_lib_dir(
    link_mode: LinkMode,
    target_os: &str,
    target_arch: &str,
    gpu: GpuVariant,
) -> Result<PathBuf, DynError> {
    if let Some(path) = env::var_os("SHERPA_ONNX_LIB_DIR") {
        let path = PathBuf::from(path);
        if !path.is_dir() {
            return Err(format!(
                "SHERPA_ONNX_LIB_DIR does not exist or is not a directory: {}",
                path.display()
            )
            .into());
        }
        return Ok(path);
    }

    download_prebuilt_libs(link_mode, target_os, target_arch, gpu)
}

fn download_prebuilt_libs(
    link_mode: LinkMode,
    target_os: &str,
    target_arch: &str,
    gpu: GpuVariant,
) -> Result<PathBuf, DynError> {
    let archive_name = archive_name(link_mode, target_os, target_arch, gpu)?;
    let archive_stem = archive_name.trim_end_matches(".tar.bz2");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let cache_root = target_dir_from_out_dir(&out_dir)?.join("sherpa-onnx-prebuilt");
    let extracted_dir = cache_root.join(archive_stem);
    let lib_dir = extracted_dir.join("lib");

    if lib_dir.is_dir() {
        return Ok(lib_dir);
    }

    // Android archives use jniLibs/{abi}/ instead of lib/. Check both.
    let android_lib_dir = extracted_dir.join("jniLibs").join(android_abi(target_arch));
    if android_lib_dir.is_dir() {
        return Ok(android_lib_dir);
    }

    fs::create_dir_all(&cache_root)?;

    let archive_path = cache_root.join(&archive_name);
    if !archive_path.is_file() {
        if let Some(local_archive_dir) = env::var_os("SHERPA_ONNX_ARCHIVE_DIR") {
            let local_archive_path = PathBuf::from(local_archive_dir).join(&archive_name);
            if !local_archive_path.is_file() {
                return Err(format!(
                    "SHERPA_ONNX_ARCHIVE_DIR does not contain expected archive: {}",
                    local_archive_path.display()
                )
                .into());
            }

            copy_file_atomically(&local_archive_path, &archive_path)?;
        } else {
            let version = env!("CARGO_PKG_VERSION");
            let url = format!("{RELEASE_BASE_URL}/v{version}/{archive_name}");
            download_with_retries(&url, &archive_path, DOWNLOAD_ATTEMPTS)?;
        }
    }

    if extracted_dir.exists() {
        fs::remove_dir_all(&extracted_dir)?;
    }

    let unpack_result: Result<(), DynError> = (|| {
        let tar_file = File::open(&archive_path)?;
        let decoder = BzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);
        archive.unpack(&cache_root)?;
        Ok(())
    })();
    if let Err(err) = unpack_result {
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_dir_all(&extracted_dir);
        return Err(format!(
            "Failed to unpack cached archive {}: {err}",
            archive_path.display()
        )
        .into());
    }

    if !lib_dir.is_dir() {
        // Android archives use jniLibs/{abi}/ instead of lib/.
        let android_lib_dir = extracted_dir.join("jniLibs").join(android_abi(target_arch));
        if android_lib_dir.is_dir() {
            eprintln!(
                "Downloaded sherpa-onnx Android libs to {}",
                android_lib_dir.display()
            );
            return Ok(android_lib_dir);
        }
        return Err(format!(
            "Downloaded archive did not contain a lib directory: {}",
            lib_dir.display()
        )
        .into());
    }

    eprintln!("Downloaded sherpa-onnx libs to {}", extracted_dir.display());

    Ok(lib_dir)
}

/// 下载失败时的重试次数。GPU 包有 200~400MB，网络抖动一次就白下，所以多试几次。
const DOWNLOAD_ATTEMPTS: u32 = 3;

/// 下载预编译包；失败时重试（见 `DOWNLOAD_ATTEMPTS`），并提示离线兜底做法。
fn download_with_retries(url: &str, archive_path: &Path, attempts: u32) -> Result<(), DynError> {
    let mut last_err = String::new();

    for attempt in 1..=attempts {
        if attempts > 1 {
            eprintln!("Downloading sherpa-onnx libs from {url} (第 {attempt}/{attempts} 次)");
        } else {
            eprintln!("Downloading sherpa-onnx libs from {url}");
        }

        let response = ureq::builder()
            .try_proxy_from_env(true)
            .build()
            .get(url)
            .call()
            .map_err(|e| format!("Failed to download sherpa-onnx archive from {url}: {e}"));

        match response {
            Ok(response) => {
                let mut reader = response.into_reader();
                match write_reader_atomically(&mut reader, archive_path) {
                    Ok(()) => return Ok(()),
                    Err(err) => {
                        last_err = format!("写入 {} 失败: {err}", archive_path.display());
                    },
                }
            },
            Err(err) => last_err = err.to_string(),
        }
    }

    Err(format!(
        "{last_err}（已重试 {attempts} 次）。可自行下载该包后用 SHERPA_ONNX_ARCHIVE_DIR \
         指定所在目录，或直接放到 {} 再重新编译。",
        archive_path.display()
    )
    .into())
}

/// Map a Rust target architecture to the Android ABI directory name used
/// in the prebuilt jniLibs/ layout.
fn android_abi(target_arch: &str) -> &str {
    match target_arch {
        "aarch64" => "arm64-v8a",
        "arm" => "armeabi-v7a",
        "x86" => "x86",
        "x86_64" => "x86_64",
        _ => "arm64-v8a",
    }
}

fn archive_name(
    link_mode: LinkMode,
    target_os: &str,
    target_arch: &str,
    gpu: GpuVariant,
) -> Result<String, DynError> {
    let version = env!("CARGO_PKG_VERSION");

    if gpu.is_gpu() {
        let tag = gpu.archive_tag()?;
        let name = match (target_os, target_arch) {
            ("linux", "x86_64") => {
                format!("sherpa-onnx-v{version}-{tag}-linux-x64-gpu.tar.bz2")
            }
            ("windows", "x86_64") => {
                format!("sherpa-onnx-v{version}-{tag}-win-x64-cuda.tar.bz2")
            }
            // 上游还给 linux-aarch64 出过 CUDA 包（`...-linux-aarch64-shared-gpu-onnxruntime-<ver>-lib.tar.bz2`），
            // 但包名里没有 CUDA/cuDNN 标签，只按 ONNX Runtime 版本区分，选哪个版本得由使用者决定，
            // 所以这里不猜；需要时下载后设 SHERPA_ONNX_LIB_DIR 指过去即可。
            _ => {
                return Err(format!(
                    "官方只为 linux-x64 / win-x64 提供 CUDA 预编译包，os={target_os} arch={target_arch} \
                     无法用 SHERPA_ONNX_GPU 启用 GPU；请自行编译 sherpa-onnx 后设置 SHERPA_ONNX_LIB_DIR。"
                )
                .into())
            }
        };
        return Ok(name);
    }

    let name = match (link_mode, target_os, target_arch) {
        (LinkMode::Static, "linux", "x86_64") => {
            format!("sherpa-onnx-v{version}-linux-x64-static-lib.tar.bz2")
        },
        (LinkMode::Static, "linux", "aarch64") => {
            format!("sherpa-onnx-v{version}-linux-aarch64-static-lib.tar.bz2")
        },
        (LinkMode::Static, "macos", "x86_64") => {
            format!("sherpa-onnx-v{version}-osx-x64-static-lib.tar.bz2")
        },
        (LinkMode::Static, "macos", "aarch64") => {
            format!("sherpa-onnx-v{version}-osx-arm64-static-lib.tar.bz2")
        },
        (LinkMode::Static, "windows", "x86_64") => {
            format!("sherpa-onnx-v{version}-win-x64-static-MT-Release-lib.tar.bz2")
        },
        (LinkMode::Shared, "linux", "x86_64") => {
            format!("sherpa-onnx-v{version}-linux-x64-shared-lib.tar.bz2")
        },
        (LinkMode::Shared, "linux", "aarch64") => {
            format!("sherpa-onnx-v{version}-linux-aarch64-shared-cpu-lib.tar.bz2")
        },
        (LinkMode::Shared, "macos", "x86_64") => {
            format!("sherpa-onnx-v{version}-osx-x64-shared-lib.tar.bz2")
        },
        (LinkMode::Shared, "macos", "aarch64") => {
            format!("sherpa-onnx-v{version}-osx-arm64-shared-lib.tar.bz2")
        },
        (LinkMode::Shared, "windows", "x86_64") => {
            format!("sherpa-onnx-v{version}-win-x64-shared-MT-Release-lib.tar.bz2")
        },
        // Android: one archive with all ABIs under jniLibs/{abi}/.
        (LinkMode::Shared, "android", "aarch64" | "arm" | "x86" | "x86_64") => {
            format!("sherpa-onnx-v{version}-android.tar.bz2")
        },
        _ => return Err(format!(
            "Unsupported target for sherpa-onnx prebuilt libs: os={target_os}, arch={target_arch}"
        )
        .into()),
    };

    Ok(name)
}

fn emit_shared_link_directives() {
    println!("cargo:rustc-link-lib=dylib=sherpa-onnx-c-api");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");
}

fn emit_static_link_directives(target_os: &str) {
    for lib in SHERPA_ONNX_STATIC_LIBS {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    match target_os {
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=dl");
        },
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=Foundation");
        },
        _ => {},
    }
}

fn target_dir_from_out_dir(out_dir: &Path) -> Result<PathBuf, DynError> {
    if let Ok(explicit_target_dir) = env::var("CARGO_TARGET_DIR") {
        return Ok(PathBuf::from(explicit_target_dir));
    }

    if let Some(target_dir) = out_dir
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new("target")))
    {
        return Ok(target_dir.to_path_buf());
    }

    Ok(out_dir.to_path_buf())
}

fn emit_relative_rpath(target_os: &str) {
    match target_os {
        "linux" | "android" => println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN"),
        "macos" => println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path"),
        _ => {},
    }
}

fn profile_output_dirs() -> Result<[PathBuf; 2], DynError> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let profile = env::var("PROFILE")?;
    let profile_dir = out_dir
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new(&profile)))
        .ok_or_else(|| {
            format!(
                "Could not locate Cargo profile directory from {}",
                out_dir.display()
            )
        })?
        .to_path_buf();

    Ok([profile_dir.clone(), profile_dir.join("examples")])
}

fn copy_unix_runtime_libs(lib_dir: &Path, target_os: &str) -> Result<(), DynError> {
    let runtime_libs: Vec<PathBuf> = fs::read_dir(lib_dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .map(|name| match target_os {
                    "linux" | "android" => name.contains(".so"),
                    "macos" => name.ends_with(".dylib"),
                    _ => false,
                })
                .unwrap_or(false)
        })
        .collect();

    if runtime_libs.is_empty() {
        return Err(format!("No shared runtime libraries found in {}", lib_dir.display()).into());
    }

    let mut copy_plan = Vec::<(PathBuf, OsString)>::new();
    let mut planned_names = HashSet::<OsString>::new();

    for lib in runtime_libs {
        if !lib.exists() {
            continue;
        }

        let lib_name = lib
            .file_name()
            .ok_or_else(|| format!("Invalid runtime library path: {}", lib.display()))?
            .to_os_string();

        let source = fs::canonicalize(&lib).unwrap_or(lib.clone());
        if planned_names.insert(lib_name.clone()) {
            copy_plan.push((source.clone(), lib_name));
        }

        if let Some(source_name) = source.file_name() {
            let source_name = source_name.to_os_string();
            if planned_names.insert(source_name.clone()) {
                copy_plan.push((source.clone(), source_name));
            }
        }
    }

    if copy_plan.is_empty() {
        return Err(format!(
            "No usable shared runtime libraries found in {}",
            lib_dir.display()
        )
        .into());
    }

    for dest_dir in profile_output_dirs()? {
        fs::create_dir_all(&dest_dir)?;
        for (source, dest_name) in &copy_plan {
            let dest = dest_dir.join(dest_name);
            fs::copy(source, &dest)?;
        }
    }

    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut temp_name = path
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsString::from("tmp"));
    temp_name.push(".part");
    path.with_file_name(temp_name)
}

fn copy_file_atomically(src: &Path, dst: &Path) -> Result<(), DynError> {
    let temp_path = temp_path_for(dst);
    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }
    fs::copy(src, &temp_path)?;
    fs::rename(&temp_path, dst)?;
    Ok(())
}

fn write_reader_atomically(reader: &mut dyn io::Read, dst: &Path) -> Result<(), DynError> {
    let temp_path = temp_path_for(dst);
    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }

    {
        let mut file = File::create(&temp_path)?;
        io::copy(reader, &mut file)?;
        file.sync_all()?;
    }

    fs::rename(&temp_path, dst)?;
    Ok(())
}

fn copy_windows_runtime_dlls(lib_dir: &Path) -> Result<(), DynError> {
    // 官方 CUDA 包（`...-win-x64-cuda.tar.bz2`）把运行期 DLL 同时放在 lib/ 与 bin/，
    // 这里两处都扫，避免只认其中一种布局。
    let mut dirs = vec![lib_dir.to_path_buf()];
    if let Some(parent) = lib_dir.parent() {
        let bin_dir = parent.join("bin");
        if bin_dir.is_dir() {
            dirs.push(bin_dir);
        }
    }

    let mut dlls: Vec<PathBuf> = Vec::new();
    for dir in &dirs {
        dlls.extend(
            fs::read_dir(dir)?
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| path.extension() == Some(OsStr::new("dll"))),
        );
    }

    if dlls.is_empty() {
        println!(
            "cargo:warning=No runtime DLLs found in {}",
            dirs.iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Ok(());
    }

    let [profile_dir, examples_dir] = profile_output_dirs()?;
    for dest_dir in [profile_dir.clone(), examples_dir] {
        fs::create_dir_all(&dest_dir)?;
        for dll in &dlls {
            let dest = dest_dir.join(
                dll.file_name()
                    .ok_or_else(|| format!("Invalid DLL path: {}", dll.display()))?,
            );
            fs::copy(dll, &dest)?;
        }
    }

    println!(
        "cargo:warning=Copied Windows runtime DLLs to {} and {}/examples",
        profile_dir.display(),
        profile_dir.display()
    );

    Ok(())
}
