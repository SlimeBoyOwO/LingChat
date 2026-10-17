use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value as JsonValue, json};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKittenModelConfig,
    OfflineTtsKokoroModelConfig, OfflineTtsMatchaModelConfig, OfflineTtsModelConfig,
    OfflineTtsPocketModelConfig, OfflineTtsSupertonicModelConfig, OfflineTtsVitsModelConfig,
    OfflineTtsZipvoiceModelConfig,
};

use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::tts::TtsConfig;

/// 本次编译链接进来的 sherpa-onnx 预编译包变体：`cpu` / `cuda12` / `cuda13`。
///
/// 由本 crate 的 build.rs 按 `SHERPA_ONNX_GPU`（以及 `SHERPA_ONNX_LIB_DIR` 实际
/// 指向的库目录）决定 —— 见 `crates/ling-chat-main/build.rs` 的
/// `emit_sherpa_gpu_variant`。判定依据是「真的链了哪套库」：CPU 包里 sherpa 自己会把
/// GPU provider 降级到 CPU，而 CUDA 包里 provider 会被真的交给 ONNX Runtime。
#[cfg(any(
    target_os = "windows",
    all(target_os = "linux", target_arch = "x86_64")
))]
const GPU_VARIANT: &str = env!("SHERPA_ONNX_GPU_VARIANT");

pub struct SherpaOnnxAdapter {
    tts: Option<OfflineTts>,
    sid: i32,
    speed: f32,
    model_path: String,
    model_type: String,
    language: String,
    voice: String,
    use_gpu: bool,
    reference_audio: Option<Vec<f32>>,
    reference_sample_rate: Option<i32>,
    reference_text: Option<String>,
}

impl SherpaOnnxAdapter {
    pub fn new(
        _config: TtsConfig,
        model_path: String,
        model_type: String,
        language: String,
        voice: String,
        use_gpu: bool,
    ) -> Result<Self> {
        let model_dir = Path::new(&model_path);
        if !model_dir.exists() {
            return Err(anyhow!("Sherpa-ONNX 模型目录不存在: {model_path}"));
        }

        // 以模型目录本身为准解析真实类型：角色设置里的 `sherpa_onnx_model_type`
        // 默认为 `vits`，若照单全收会让 Kokoro/Matcha 等模型按错误类型加载而报
        // 「未找到模型文件」。
        let model_type = crate::ai_service::tts::local::sherpa_onnx_manager::resolve_model_type(
            model_dir,
            &model_type,
        );

        let provider = Self::provider_for(use_gpu);

        // 按平台选择合适的执行提供方（GPU/专用加速）；不可用或失败时回退 CPU。
        let create = |provider: Option<&str>| -> Result<OfflineTts> {
            match model_type.as_str() {
                "vits" | "fastspeech2" => Self::create_vits_tts(model_dir, provider),
                "matcha" => Self::create_matcha_tts(model_dir, provider),
                "kokoro" => Self::create_kokoro_tts(model_dir, &language, provider),
                "kitten" => Self::create_kitten_tts(model_dir, provider),
                "zipvoice" => Self::create_zipvoice_tts(model_dir, provider),
                "pocket" => Self::create_pocket_tts(model_dir, provider),
                "supertonic" => Self::create_supertonic_tts(model_dir, provider),
                other => Err(anyhow!("Sherpa-ONNX 不支持的模型类型: {other}")),
            }
        };

        let tts = match create(provider) {
            Ok(tts) => {
                if let Some(p) = provider {
                    tracing::info!("Sherpa-ONNX 使用推理后端: {p}");
                }
                tts
            },
            Err(e) => match provider {
                // GPU/专用后端初始化失败时静默回退到 CPU，保证可用性。
                Some(_) => {
                    tracing::warn!("Sherpa-ONNX 后端初始化失败（{e}），回退到 CPU 推理");
                    create(None)?
                },
                None => return Err(e),
            },
        };

        let sr = tts.sample_rate();
        tracing::info!(
            "Sherpa-ONNX 初始化完成: model_type={model_type}, language={language}, provider={:?}, sample_rate={sr}",
            provider.unwrap_or("cpu")
        );

        Ok(Self {
            tts: Some(tts),
            sid: 0,
            speed: 1.0,
            model_path,
            model_type,
            language,
            voice,
            use_gpu,
            reference_audio: None,
            reference_sample_rate: None,
            reference_text: None,
        })
    }

    /// 根据是否开启 GPU 加速与目标平台，选择 ONNX Runtime 执行提供方。
    ///
    /// 返回 `None` 表示使用 CPU。各平台：
    /// - Android: `nnapi`（走设备 GPU/NPU）
    /// - Windows / Linux x86_64: 链接官方 CUDA 包时用 `cuda`，否则请求 GPU 也会被
    ///   sherpa-onnx 自己降级（CPU 包没编 `SHERPA_ONNX_ENABLE_GPU`）
    /// - macOS: `coreml`
    /// - 其它: CPU
    fn provider_for(use_gpu: bool) -> Option<&'static str> {
        if !use_gpu {
            return None;
        }
        #[cfg(target_os = "android")]
        {
            return Some("nnapi");
        }
        #[cfg(any(
            target_os = "windows",
            all(target_os = "linux", target_arch = "x86_64")
        ))]
        {
            return Self::desktop_gpu_provider();
        }
        #[cfg(target_os = "macos")]
        {
            return Some("coreml");
        }
        #[allow(unreachable_code)]
        None
    }

    /// 桌面端（Windows / Linux x86_64）的 GPU provider。
    ///
    /// 本仓库的 sherpa-onnx 预编译包默认是 CPU 版，此时请求 GPU 会被 sherpa 自己降级到
    /// CPU（日志里会打 `Please compile with -DSHERPA_ONNX_ENABLE_GPU=ON`）；只有用
    /// `SHERPA_ONNX_GPU=cuda12|cuda13` 编出来的 CUDA 版才会真的把 provider 交给 ONNX
    /// Runtime。而 CUDA 版有个坑：provider 动态库加载失败时 ORT 抛的是 C++ 异常，
    /// 穿不过 FFI，**直接 abort 整个进程**（实测缺 CUDA 运行时就是如此），所以交给
    /// sherpa 之前先由 `cuda_ep_ready` 判断 CUDA EP 是否真的可用。
    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    fn desktop_gpu_provider() -> Option<&'static str> {
        // Windows 的 CPU 包走 DirectML 这条既有路径（同样会被 sherpa 降级）；
        // 一旦换成 CUDA 包，则两个平台都用 cuda。
        if !Self::cuda_package_linked() {
            return Some(if cfg!(target_os = "windows") {
                "dml"
            } else {
                "cuda"
            });
        }

        if Self::cuda_ep_ready() {
            Some("cuda")
        } else {
            None
        }
    }

    /// 链接进来的 sherpa-onnx 预编译包是不是官方 CUDA 包（见
    /// `crates/ling-chat-main/build.rs` 的 `emit_sherpa_gpu_variant`）。
    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    fn cuda_package_linked() -> bool {
        matches!(GPU_VARIANT, "cuda12" | "cuda13")
    }

    /// CUDA 执行提供方（CUDA EP）能不能用。
    ///
    /// 不能用时**绝不能**把 provider 交给 sherpa-onnx：ORT 1.27 加载 provider 动态库
    /// 失败会抛 C++ 异常，异常穿不过 FFI，进程直接 abort（实测：缺 `libcublasLt.so.12`
    /// 时就是这样）。而又不能真的 dlopen provider 库来试探 —— provider 的静态初始化会调用
    /// `Provider_GetHost()`，该 host 指针由 ORT 在加载 provider **之前**设置，自己先加载
    /// 会段错误（实测）。所以退一步做两件静态检查：
    ///
    /// 1. provider 库文件在不在（ORT 按 `libonnxruntime` 所在目录找它）；
    /// 2. CUDA 运行时的动态库能不能加载（provider 的 `DT_NEEDED`，缺一个必然加载失败）。
    ///
    /// 覆盖不到的情况：库都齐全、但 CUDA 初始化失败（例如显卡算力不被支持）。那只能靠 ORT
    /// 自己的日志排查，代价是「宁可退回 CPU，也不让开关变成崩溃按钮」。
    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    fn cuda_ep_ready() -> bool {
        let dirs = Self::cuda_provider_search_dirs();
        let provider = Self::cuda_provider_file_name();
        let Some(found_in) = dirs.iter().find(|dir| dir.join(provider).is_file()) else {
            tracing::warn!(
                "Sherpa-ONNX 没找到 CUDA 执行提供方动态库 {provider}（查找目录: {}），改用 CPU 推理",
                dirs.iter()
                    .map(|dir| dir.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return false;
        };

        for soname in Self::cuda_runtime_sonames() {
            if let Err(err) = unsafe { libloading::Library::new(soname) } {
                tracing::warn!(
                    "Sherpa-ONNX CUDA 运行时缺少 {soname}（{err}），改用 CPU 推理；provider 库在 {}",
                    found_in.display()
                );
                return false;
            }
        }

        tracing::info!("Sherpa-ONNX CUDA 执行提供方就绪（{}）", found_in.display());
        true
    }

    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    fn cuda_provider_file_name() -> &'static str {
        if cfg!(target_os = "windows") {
            "onnxruntime_providers_cuda.dll"
        } else {
            "libonnxruntime_providers_cuda.so"
        }
    }

    /// provider 库可能所在的目录：可执行文件目录（开发时的 `target/<profile>`、安装后的
    /// 安装目录），以及 Linux 下 `libonnxruntime` 实际被加载的目录（测试二进制在
    /// `target/<profile>/deps/`，只看可执行文件目录会漏）。
    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    fn cuda_provider_search_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            dirs.push(dir);
        }

        #[cfg(target_os = "linux")]
        if let Some(dir) = Self::libonnxruntime_dir_from_maps() {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }

        dirs
    }

    /// 从 `/proc/self/maps` 里读 `libonnxruntime.so` 的加载路径并取所在目录。
    #[cfg(target_os = "linux")]
    fn libonnxruntime_dir_from_maps() -> Option<PathBuf> {
        let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
        for line in maps.lines() {
            // 形如：7f12…-7f13… r-xp 00000000 08:02 1234567 /path/to/libonnxruntime.so
            let Some(path) = line.split_whitespace().nth(5) else {
                continue;
            };
            if !path.ends_with("libonnxruntime.so") {
                continue;
            }
            return Path::new(path).parent().map(Path::to_path_buf);
        }
        None
    }

    /// CUDA 运行时动态库名（provider 的 `DT_NEEDED`）。
    ///
    /// cuda12 这份由实测 `readelf -d libonnxruntime_providers_cuda.so` 得到；cuda13 按同样
    /// 的 soname 规则（主版本号换 CUDA 13）列出，**未实测**。漏判只会让 GPU 退回 CPU
    /// （有日志），不会崩。
    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    fn cuda_runtime_sonames() -> &'static [&'static str] {
        let cuda13 = GPU_VARIANT == "cuda13";

        if cfg!(target_os = "windows") {
            return if cuda13 {
                &[
                    "nvcuda.dll",
                    "cudart64_13.dll",
                    "cublas64_13.dll",
                    "cublasLt64_13.dll",
                    "cudnn64_9.dll",
                    "cufft64_12.dll",
                    "curand64_10.dll",
                    "nvrtc64_130_0.dll",
                ]
            } else {
                &[
                    "nvcuda.dll",
                    "cudart64_12.dll",
                    "cublas64_12.dll",
                    "cublasLt64_12.dll",
                    "cudnn64_9.dll",
                    "cufft64_11.dll",
                    "curand64_10.dll",
                    "nvrtc64_120_0.dll",
                ]
            };
        }

        if cuda13 {
            &[
                "libcuda.so.1",
                "libcudart.so.13",
                "libcublas.so.13",
                "libcublasLt.so.13",
                "libcudnn.so.9",
                "libcufft.so.12",
                "libcurand.so.10",
                "libnvrtc.so.13",
            ]
        } else {
            &[
                "libcuda.so.1",
                "libcudart.so.12",
                "libcublas.so.12",
                "libcublasLt.so.12",
                "libcudnn.so.9",
                "libcufft.so.11",
                "libcurand.so.10",
                "libnvrtc.so.12",
            ]
        }
    }

    pub fn set_reference_audio(&mut self, samples: Vec<f32>, sample_rate: i32, text: String) {
        self.reference_audio = Some(samples);
        self.reference_sample_rate = Some(sample_rate);
        self.reference_text = Some(text);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    fn create_vits_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let model_file = find_file(
            model_dir,
            &["model.onnx", "tts-model.onnx", "sherpa-onnx-tts.onnx"],
        )?;
        let tokens_file = find_file(model_dir, &["tokens.txt"])?;

        let vits_config = OfflineTtsVitsModelConfig {
            model: Some(model_file.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            tokens: Some(tokens_file.to_string_lossy().to_string()),
            data_dir: find_dir_optional(model_dir, &["data-dir", "data"])
                .map(|p| p.to_string_lossy().to_string()),
            noise_scale: 0.667,
            noise_scale_w: 0.8,
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict-dir", "dict"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: vits_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX VITS 模型加载失败，请检查模型文件是否完整"))
    }

    /// 返回 `espeak-ng-data` 目录路径（即 data_dir 的取值）。
    fn espeak_data_dir(model_dir: &Path) -> Option<PathBuf> {
        find_dir_optional(model_dir, &["espeak-ng-data"])
    }

    fn create_matcha_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let acoustic_model = find_file(
            model_dir,
            &["model.onnx", "model-steps-3.onnx", "model-steps-6.onnx"],
        )?;
        let tokens_file = find_file(model_dir, &["tokens.txt"])?;
        let vocoder = find_file(
            model_dir,
            &[
                "vocos-22khz-univ.onnx",
                "vocos-16khz-univ.onnx",
                "hifigan_v3.onnx",
                "mb_melgan.onnx",
                "vocoder.onnx",
            ],
        )?;

        let matcha_config = OfflineTtsMatchaModelConfig {
            acoustic_model: Some(acoustic_model.to_string_lossy().to_string()),
            vocoder: Some(vocoder.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            tokens: Some(tokens_file.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
            noise_scale: 0.667,
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                matcha: matcha_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Matcha 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_kokoro_tts(
        model_dir: &Path,
        language: &str,
        provider: Option<&str>,
    ) -> Result<OfflineTts> {
        let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
        let voices = find_file(model_dir, &["voices.bin"])?;
        // Android/arm64 上 kokoro 的 int8 量化会输出纯静音或钉死的垃圾样本（上游
        // k2-fsa/sherpa-onnx#3754；同一设备换 fp32 立即恢复，已在实机验证）。这里
        // 直接报错，避免用户只听到一段静音却看不出原因。桌面端不受影响。
        if cfg!(target_os = "android")
            && model
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("int8"))
        {
            return Err(anyhow!(
                "Android 上 int8 量化的 Kokoro 模型会输出静音/杂音（上游缺陷 \
                 sherpa-onnx#3754），请改用未量化版本（Kokoro 多语言 103 音色 (未量化)）\
                 或其他模型"
            ));
        }
        // 多语言模型：只有日文需要显式指定——不指定时会被逐字当成中文念（27 字要
        // 16 秒）。中文/英文/中英混合以及数字都交给模型自行判断即可；显式传 "zh"
        // 反而会把句子里的英文片段和数字整段丢掉（音素化失败后静默跳过）。
        // 注意不能传 "auto"：它会让纯英文/日文片段音素化失败并让整次合成直接报错。
        let lang = match language.trim() {
            "ja" => Some("ja".to_string()),
            _ => None,
        };

        let kokoro_config = OfflineTtsKokoroModelConfig {
            model: Some(model.to_string_lossy().to_string()),
            voices: Some(voices.to_string_lossy().to_string()),
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict"])
                .map(|p| p.to_string_lossy().to_string()),
            lexicon: kokoro_lexicons(model_dir),
            lang,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: kokoro_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Kokoro 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_zipvoice_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let encoder = find_file(model_dir, &["text_encoder.onnx"])?;
        let decoder = find_file(model_dir, &["fm_decoder.onnx"])?;
        let vocoder = find_file(model_dir, &["vocos_24khz.onnx", "vocoder.onnx"])?;

        // data-dir 与 lexicon 都是 sherpa-onnx 的致命依赖：任一为空，原生侧就
        // `SHERPA_ONNX_EXIT(-1)`（=`_Exit(-1)`，进程以 255 静默退出，Rust 侧捕不到），
        // 所以先在这里拦成普通错误。
        let data_dir = zipvoice_data_dir(model_dir).ok_or_else(|| {
            anyhow!(
                "ZipVoice 模型缺少 espeak-ng-data（需含 phontab/phondata/phonindex），模型包不完整: {}",
                model_dir.display()
            )
        })?;
        let lexicon = zipvoice_lexicon(model_dir)?;

        let zipvoice_config = OfflineTtsZipvoiceModelConfig {
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            encoder: Some(encoder.to_string_lossy().to_string()),
            decoder: Some(decoder.to_string_lossy().to_string()),
            vocoder: Some(vocoder.to_string_lossy().to_string()),
            // ZipVoice 的 data-dir 必须是直接包含 phontab/phondata/phonindex 的目录
            //（与 Matcha/espeak 的 espeak-ng-data 父目录层级不同）。部分模型把
            // 这些语音库文件放到 espeak-ng-data/ 下，因此需按 phontab 实际所在目录解析。
            data_dir: Some(data_dir.to_string_lossy().to_string()),
            lexicon: Some(lexicon.to_string_lossy().to_string()),
            // 默认值必须显式写：sherpa-onnx 的 Rust 绑定这里 `Default` 全是 0.0，
            // 而原生侧要求 feat_scale/target_rms/guidance_scale > 0（0 会被
            // `Generate` 直接判为非法参数）。数值取上游原生默认值，见
            // sherpa-onnx/csrc/offline-tts-zipvoice-model-config.h：
            // `feat_scale = 0.1, t_shift = 0.5, target_rms = 0.1, guidance_scale = 1.0`。
            // 这三个值直接决定提示梅尔缩放、解码时间步与响度归一化，偏离一个数量级
            // 就会合成出极轻的低频噪声（实测 feat_scale=10、t_shift=0.005 时音频
            // 均响度只有 -36 dB，频谱集中在极低频）。
            feat_scale: 0.1,
            t_shift: 0.5,
            target_rms: 0.1,
            guidance_scale: 1.0,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                zipvoice: zipvoice_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX ZipVoice 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_kitten_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
        let voices = find_file(model_dir, &["voices.bin"])?;

        let kitten_config = OfflineTtsKittenModelConfig {
            model: Some(model.to_string_lossy().to_string()),
            voices: Some(voices.to_string_lossy().to_string()),
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir).map(|p| p.to_string_lossy().to_string()),
            ..Default::default()
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kitten: kitten_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Kitten 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_pocket_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let pocket_config = OfflineTtsPocketModelConfig {
            lm_flow: Some(
                find_file(model_dir, &["lm_flow.int8.onnx", "lm_flow.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            lm_main: Some(
                find_file(model_dir, &["lm_main.int8.onnx", "lm_main.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            encoder: Some(
                find_file(model_dir, &["encoder.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            decoder: Some(
                find_file(model_dir, &["decoder.int8.onnx", "decoder.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            text_conditioner: Some(
                find_file(model_dir, &["text_conditioner.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            vocab_json: Some(
                find_file(model_dir, &["vocab.json"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            token_scores_json: Some(
                find_file(model_dir, &["token_scores.json"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            voice_embedding_cache_capacity: 8,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                pocket: pocket_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Pocket TTS 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_supertonic_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let supertonic_config = OfflineTtsSupertonicModelConfig {
            duration_predictor: Some(
                find_file(model_dir, &["duration_predictor.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            text_encoder: Some(
                find_file(model_dir, &["text_encoder.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            vector_estimator: Some(
                find_file(model_dir, &["vector_estimator.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            vocoder: Some(
                find_file(model_dir, &["vocoder.onnx", "vocoder_f16.onnx"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            tts_json: Some(
                find_file(model_dir, &["tts.json"])?
                    .to_string_lossy()
                    .to_string(),
            ),
            unicode_indexer: find_file_optional(model_dir, &["unicode_indexer.json"])
                .map(|p| p.to_string_lossy().to_string()),
            voice_style: find_file_optional(model_dir, &["voice_style.onnx"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                supertonic: supertonic_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        OfflineTts::create(&with_rule_files(config, model_dir))
            .ok_or_else(|| anyhow!("Sherpa-ONNX Supertonic 模型加载失败，请检查模型文件是否完整"))
    }
}

#[async_trait]
impl TtsAdapter for SherpaOnnxAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let tts = self
            .tts
            .as_ref()
            .ok_or_else(|| anyhow!("Sherpa-ONNX 引擎未初始化"))?;

        let gen_config = GenerationConfig {
            sid: self.sid,
            speed: self.speed,
            reference_audio: self.reference_audio.clone(),
            reference_sample_rate: self.reference_sample_rate.unwrap_or(0),
            reference_text: self.reference_text.clone(),
            ..Default::default()
        };

        let audio = tts
            .generate_with_config(text, &gen_config, None::<fn(&[f32], f32) -> bool>)
            .ok_or_else(|| anyhow!("Sherpa-ONNX 生成失败"))?;

        let samples = audio.samples().to_vec();
        let sample_rate = audio.sample_rate();

        if samples.is_empty() {
            return Err(anyhow!("Sherpa-ONNX 生成的音频为空"));
        }

        Ok(f32_to_wav(&samples, sample_rate))
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut params = HashMap::new();
        params.insert("model_path".into(), json!(self.model_path));
        params.insert("model_type".into(), json!(self.model_type));
        params.insert("language".into(), json!(self.language));
        params.insert("voice".into(), json!(self.voice));
        params.insert("use_gpu".into(), json!(self.use_gpu));
        params.insert("speed".into(), json!(self.speed));
        params.insert("sid".into(), json!(self.sid));
        params.insert(
            "has_reference_audio".into(),
            json!(self.reference_audio.is_some()),
        );
        params
    }
}

impl std::fmt::Debug for SherpaOnnxAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SherpaOnnxAdapter")
            .field("model_path", &self.model_path)
            .field("model_type", &self.model_type)
            .field("language", &self.language)
            .field("voice", &self.voice)
            .field("use_gpu", &self.use_gpu)
            .field("sid", &self.sid)
            .field("speed", &self.speed)
            .field("has_reference_audio", &self.reference_audio.is_some())
            .finish()
    }
}

fn find_file(dir: &Path, candidates: &[&str]) -> Result<PathBuf> {
    for name in candidates {
        let path = dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "在 {} 中未找到模型文件，候选: {:?}",
        dir.display(),
        candidates
    ))
}

/// 若模型目录内附带 Thrax 规则文件（rule_fst.fst / rule.far 等），自动挂载到
/// `OfflineTtsConfig`，使长文本/数字的书名号、日期等规范化更准确。
fn with_rule_files(mut config: OfflineTtsConfig, model_dir: &Path) -> OfflineTtsConfig {
    if let Some(fst) = find_file_optional(model_dir, &["rule_fst.fst", "rule_fst.far", "rule.fst"])
    {
        config.rule_fsts = Some(fst.to_string_lossy().to_string());
    }
    if let Some(far) = find_file_optional(model_dir, &["rulefst.far", "rule.far"]) {
        config.rule_fars = Some(far.to_string_lossy().to_string());
    }
    config
}

fn find_file_optional(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
}

fn find_dir_optional(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.is_dir())
}

/// 收集 Kokoro 需要的 lexicon 文件（逗号分隔）。
///
/// 顺序与官方示例一致：英文 → 中文，其余 `lexicon*.txt` 兜底追加。
/// 英文只挂一份：`lexicon-us-en.txt` 与 `lexicon-gb-en.txt` 内容高度重叠，
/// 同时挂载会刷出上千行 "Duplicated word ... Ignore it" 告警。
///
/// Kokoro 不挂 lexicon 时中文只能退到 espeak-ng 的音素化，实测合成中文几乎出不来
/// 有效音频（只有 0.2~0.5 秒的噪声或纯静音），因此这里必须显式挂载。
fn kokoro_lexicons(model_dir: &Path) -> Option<String> {
    const ENGLISH: &[&str] = &["lexicon-us-en.txt", "lexicon-gb-en.txt"];
    const CHINESE: &[&str] = &["lexicon-zh.txt"];

    let mut files: Vec<PathBuf> = Vec::new();
    for group in [ENGLISH, CHINESE] {
        if let Some(path) = find_file_optional(model_dir, group) {
            files.push(path);
        }
    }

    // 自定义模型可能只带 lexicon.txt 之类，兜底扫描目录内其余 lexicon*.txt。
    if let Ok(entries) = std::fs::read_dir(model_dir) {
        let mut extra: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("lexicon") && name.ends_with(".txt"))
                    && !files.contains(path)
                    && !ENGLISH.iter().any(|name| path.ends_with(name))
            })
            .collect();
        extra.sort();
        files.extend(extra);
    }

    if files.is_empty() {
        return None;
    }
    Some(
        files
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// 定位 ZipVoice 数据目录：该目录需直接包含 phontab/phondata/phonindex。
/// 优先匹配目录本身或其 `espeak-ng-data` 子目录，其次匹配模型目录。
fn zipvoice_data_dir(model_dir: &Path) -> Option<PathBuf> {
    if model_dir.join("phontab").exists() {
        return Some(model_dir.to_path_buf());
    }
    if find_dir_optional(model_dir, &["espeak-ng-data"]).is_some_and(|p| p.join("phontab").exists())
    {
        return find_dir_optional(model_dir, &["espeak-ng-data"]);
    }
    None
}

/// 内置的 ZipVoice 中英音素表（`字/词 → 声母0 韵母…`），随二进制一起发布。
///
/// 为什么必须内置：sherpa-onnx 的 `MatchaTtsLexicon` 在 lexicon 为空时直接
/// `SHERPA_ONNX_EXIT(-1)`（= `_Exit(-1)`，进程以 255 静默退出，Rust 侧捕不到），
/// 而 ZipVoice 的 `InitFrontend()` 必然走这个类。上游官方包里的 `lexicon.txt` 并不是
/// 模型自带文件，而是 CI 打包时用 `scripts/zipvoice/zh-en/generate_lexicon.py`
/// （pypinyin + 中英音素转换）现场生成的；我们用的 ModelScope 镜像包
/// （`gomodels/sherpa`）里只有 `pinyin.raw`、没有该文件，而 `pinyin.raw` 的整音节写法
/// （`好 hao3`）与模型 `tokens.txt` 的「声母+0 / 韵母」方案（`h0 ao3`）不兼容，
/// 不能直接拿来当 lexicon。
///
/// 本文件即上述脚本的输出（68037 条），音素已逐条校验都在模型 `tokens.txt` 里。
const ZIPVOICE_LEXICON_ZH_EN: &str = include_str!("zipvoice-lexicon-zh-en.txt");

/// 定位 ZipVoice 的 lexicon：优先模型目录自带的，缺失时写入内置的中英音素表。
///
/// 写回模型目录而不是临时目录，是为了让模型目录自洽（其它工具、以及下次启动
/// 都直接命中），而且只需要写一次。
fn zipvoice_lexicon(model_dir: &Path) -> Result<PathBuf> {
    if let Some(path) = find_file_optional(model_dir, &["lexicon.txt"]) {
        // 0 字节的 lexicon 等于没有：sherpa 只在「路径为空」时报错退出，空文件则
        // 让每个字都变成 OOV（念不出声），所以这里也按缺失处理。
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > 0 {
            return Ok(path);
        }
        tracing::warn!(
            "ZipVoice 模型自带的 {} 为空，改用内置中英音素表",
            path.display()
        );
    }

    let text = bundled_zipvoice_lexicon();
    if !zipvoice_lexicon_matches_tokens(model_dir, &text) {
        return Err(anyhow!(
            "ZipVoice 模型缺少 lexicon.txt，且内置中英音素表与 {} 里的 tokens.txt 不匹配（不是同一个中英模型）",
            model_dir.display()
        ));
    }

    let path = model_dir.join("lexicon.txt");
    std::fs::write(&path, text.as_bytes())
        .map_err(|e| anyhow!("写入 ZipVoice lexicon.txt 失败（{}）: {e}", path.display()))?;
    tracing::info!(
        "ZipVoice 模型缺少 lexicon.txt（上游由 CI 生成），已写入内置中英音素表: {}",
        path.display()
    );
    Ok(path)
}

/// 内置音素表文本，行尾统一成 LF。
///
/// `include_str!` 是逐字节嵌入：Windows 上若把换行检出成 CRLF，行尾的 `\r` 会被
/// 当成音素的一部分而全部查不到（`.gitattributes` 已固定 LF，这里再兜一层）。
fn bundled_zipvoice_lexicon() -> Cow<'static, str> {
    if ZIPVOICE_LEXICON_ZH_EN.contains('\r') {
        Cow::Owned(ZIPVOICE_LEXICON_ZH_EN.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(ZIPVOICE_LEXICON_ZH_EN)
    }
}

/// 内置音素表的音素是否都被模型 `tokens.txt` 收录。
///
/// 防止把中英音素表塞给别的 ZipVoice 模型（例如纯英文模型）——那种情况下宁可报错，
/// 也不要念出一堆 OOV。
fn zipvoice_lexicon_matches_tokens(model_dir: &Path, lexicon: &str) -> bool {
    let Ok(tokens) = std::fs::read_to_string(model_dir.join("tokens.txt")) else {
        return false;
    };
    let tokens: HashSet<&str> = tokens
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    lexicon
        .lines()
        .all(|line| line.split_whitespace().skip(1).all(|p| tokens.contains(p)))
}

fn f32_to_wav(samples: &[f32], sample_rate: i32) -> Vec<u8> {
    let num_samples = samples.len();
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;
    let sr = sample_rate as u32;
    let byte_rate = sr * 2;

    let mut wav = Vec::with_capacity(44 + data_size);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(file_size as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sr.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());

    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        wav.extend_from_slice(&i16_sample.to_le_bytes());
    }

    wav
}

pub fn load_reference_audio(path: &Path) -> Result<(Vec<f32>, i32)> {
    // Android 的 content:// URI（早期版本直接存进设置的路径）读不了，早点说清楚
    // 该怎么办，别让用户看到下面那句「仅支持 WAV」而摸不着头脑。
    if path.to_string_lossy().starts_with("content://") {
        return Err(anyhow!(
            "参考音频是 Android 的 content:// URI，请先在语音设置里重新导入一次（会复制到应用数据目录）: {}",
            path.display()
        ));
    }

    // 仅支持 WAV 格式（sherpa-onnx 零样本克隆要求 WAV 输入）
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "wav" {
        return Err(anyhow!(
            "参考音频仅支持 WAV 格式，当前文件格式: {} ({}). 请转换为 WAV 后重试",
            ext,
            path.display()
        ));
    }

    let data = std::fs::read(path)
        .map_err(|e| anyhow!("无法读取参考音频文件 {}: {}", path.display(), e))?;

    // 最小 WAV 头：44 字节（RIFF/WAVE/fmt/data 头）。低于此长度的输入直接报错，
    // 避免下方切片越界 panic。
    if data.len() < 44 {
        return Err(anyhow!(
            "WAV 文件过短（{} 字节，至少需要 44 字节）: {}",
            data.len(),
            path.display()
        ));
    }

    let channels = u16::from_le_bytes(data[22..24].try_into().unwrap()) as u32;
    let sample_rate = i32::from_le_bytes(data[24..28].try_into().unwrap());
    let bits_per_sample = u16::from_le_bytes(data[34..36].try_into().unwrap());

    let data_offset = find_wav_data_chunk(&data)
        .ok_or_else(|| anyhow!("WAV 文件中未找到 data 块: {}", path.display()))?;

    let samples = match bits_per_sample {
        16 => {
            let raw = &data[data_offset..];
            raw.as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect::<Vec<_>>()
        },
        32 => {
            let raw = &data[data_offset..];
            raw.as_chunks::<4>()
                .0
                .iter()
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2147483648.0)
                .collect::<Vec<_>>()
        },
        _ => {
            return Err(anyhow!(
                "不支持的 WAV 位深度: {} (仅支持 16/32)",
                bits_per_sample
            ));
        },
    };

    let mono_samples = if channels > 1 {
        samples
            .chunks(channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    if mono_samples.is_empty() {
        return Err(anyhow!("参考音频为空"));
    }

    Ok((mono_samples, sample_rate))
}

fn find_wav_data_chunk(data: &[u8]) -> Option<usize> {
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        if chunk_id == b"data" {
            return Some(pos + 8);
        }
        pos += 8 + chunk_size;
        if pos % 2 != 0 {
            pos += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model_dir() -> String {
        std::env::var("SHERPA_TEST_MODEL_DIR").unwrap_or_default()
    }

    /// 解析 WAV 字节并返回 (采样率, 单声道样本数)。
    fn parse_wav(wav: &[u8]) -> (u32, usize) {
        assert!(wav.len() >= 44, "WAV 太短");
        assert_eq!(&wav[0..4], b"RIFF", "缺 RIFF 头");
        assert_eq!(&wav[8..12], b"WAVE", "缺 WAVE");
        let sample_rate = u32::from_le_bytes(wav[24..28].try_into().unwrap());
        let bits = u16::from_le_bytes(wav[34..36].try_into().unwrap());
        let data_start = find_wav_data_chunk(wav).expect("缺 data chunk");
        let data_bytes = wav.len() - data_start;
        (sample_rate, data_bytes / (bits as usize / 8))
    }

    /// 造一个「假 ZipVoice 模型目录」：只会写 tokens.txt。
    ///
    /// `tokens_from_bundled = true` 时用内置音素表的音素生成 tokens.txt（模拟中英
    /// 模型，音素全部命中）；否则只放无关音素（模拟别的 ZipVoice 模型）。
    fn fake_zipvoice_dir(tag: &str, tokens_from_bundled: bool) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lingchat-zipvoice-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("建临时模型目录失败");

        let tokens = if tokens_from_bundled {
            let bundled = bundled_zipvoice_lexicon();
            let mut phonemes: Vec<&str> = bundled
                .lines()
                .flat_map(|line| line.split_whitespace().skip(1))
                .collect();
            phonemes.sort_unstable();
            phonemes.dedup();
            phonemes.iter().map(|p| format!("{p} 0\n")).collect()
        } else {
            "x0 0\ny0 1\n".to_string()
        };
        std::fs::write(dir.join("tokens.txt"), tokens).expect("写 tokens.txt 失败");
        dir
    }

    /// 模型目录缺 lexicon.txt 时，应写入内置中英音素表并返回该路径。
    #[test]
    fn zipvoice_missing_lexicon_falls_back_to_bundled() {
        let dir = fake_zipvoice_dir("bundled", true);

        let path = zipvoice_lexicon(&dir).expect("应回退到内置音素表");
        assert_eq!(path, dir.join("lexicon.txt"));

        let written = std::fs::read_to_string(&path).expect("读取写入的 lexicon 失败");
        assert_eq!(written, bundled_zipvoice_lexicon().as_ref());
        assert!(
            written.lines().count() > 60_000,
            "内置音素表疑似被截断: {} 行",
            written.lines().count()
        );

        // 已存在（非空）的 lexicon 直接复用，不再覆盖
        std::fs::write(&path, "好 h0 ao3\n").expect("写测试 lexicon 失败");
        assert_eq!(zipvoice_lexicon(&dir).expect("应复用已有 lexicon"), path);
        assert_eq!(
            std::fs::read_to_string(&path).expect("读取 lexicon 失败"),
            "好 h0 ao3\n"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 0 字节的 lexicon.txt 视为缺失（否则每个字都会变成 OOV）。
    #[test]
    fn zipvoice_empty_lexicon_is_replaced() {
        let dir = fake_zipvoice_dir("empty", true);
        std::fs::write(dir.join("lexicon.txt"), "").expect("写空 lexicon 失败");

        let path = zipvoice_lexicon(&dir).expect("空 lexicon 应被内置音素表替换");
        let written = std::fs::read_to_string(&path).expect("读取 lexicon 失败");
        assert!(written.lines().count() > 60_000, "应写入完整内置音素表");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 音素与 tokens.txt 不匹配时不允许写入（宁可报错也不要念出一堆 OOV）。
    #[test]
    fn zipvoice_lexicon_rejected_when_tokens_mismatch() {
        let dir = fake_zipvoice_dir("mismatch", false);

        let err = zipvoice_lexicon(&dir).expect_err("音素不匹配时应报错");
        assert!(
            err.to_string().contains("tokens.txt"),
            "错误信息应说明 tokens.txt 不匹配: {err}"
        );
        assert!(
            !dir.join("lexicon.txt").exists(),
            "不匹配时不应写入 lexicon"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ZipVoice 零样本语音克隆：加载参考音频 → 设置 → 合成 → 验证非静音。
    ///
    /// 依赖真实模型和参考音频，默认忽略；通过设置 `SHERPA_TEST_MODEL_DIR`（模型目录）
    /// 并用 `-- --ignored` 显式运行。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "需要 zipvoice 模型目录（SHERPA_TEST_MODEL_DIR）"]
    async fn zipvoice_clone_with_reference() {
        let model_dir = test_model_dir();
        assert!(
            !model_dir.is_empty() && Path::new(&model_dir).exists(),
            "模型目录不存在，请设置 SHERPA_TEST_MODEL_DIR: {model_dir}"
        );

        let ref_path = std::env::var("SHERPA_TEST_REF_AUDIO")
            .unwrap_or_else(|_| "/tmp/baizi_ref.wav".to_string());
        assert!(Path::new(&ref_path).exists(), "参考音频不存在: {ref_path}");

        // 参考文本必须与参考音频内容一致（ZipVoice 会用它对提示音频做对齐），
        // 换参考音频时用 SHERPA_TEST_REF_TEXT 一起换。
        let ref_text = std::env::var("SHERPA_TEST_REF_TEXT")
            .unwrap_or_else(|_| "你好，我是白子。".to_string());

        // 加载参考音频
        let (samples, sample_rate) =
            load_reference_audio(Path::new(&ref_path)).expect("加载参考音频失败");
        eprintln!(
            "参考音频: sr={}, samples={} ({:.2}s)",
            sample_rate,
            samples.len(),
            samples.len() as f64 / sample_rate as f64
        );

        // 初始化 adapter
        let adapter = SherpaOnnxAdapter::new(
            TtsConfig::default(),
            model_dir,
            "zipvoice".to_string(),
            "zh".to_string(),
            "female".to_string(),
            false,
        )
        .expect("初始化 Sherpa-ONNX 失败");

        // 设置参考音频
        let mut adapter = adapter;
        adapter.set_reference_audio(samples, sample_rate, ref_text);
        adapter.set_speed(1.0);

        // 合成
        let text = "老师好，今天也请多指教了。";
        eprintln!("合成文本: {text}");
        let wav = adapter.generate_voice(text, "").await.expect("合成失败");

        let (sr, num_samples) = parse_wav(&wav);
        eprintln!("输出: sr={sr}, 样本数={num_samples}, WAV-Ok");

        // 验证音频非静音
        let data_start = find_wav_data_chunk(&wav).unwrap();
        let pcm = &wav[data_start..];
        let has_signal = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .any(|c| i16::from_le_bytes(*c).unsigned_abs() > 200);
        assert!(has_signal, "生成音频疑似全静音");

        // 保存输出
        let out = std::env::temp_dir().join("sherpa_clone_output.wav");
        std::fs::write(&out, &wav).expect("写入输出失败");
        eprintln!("输出文件: {}", out.display());
    }

    /// 完整跑一遍 Matcha 语音合成（覆盖初始化 + generate_voice + WAV 编码），
    /// 分别验证 CPU 与 GPU(此机无 NVIDIA -> CUDA 失败回退 CPU) 两条路径。
    ///
    /// 依赖真实模型，默认忽略；通过设置 `SHERPA_TEST_MODEL_DIR`（模型目录）
    /// 并用 `-- --ignored` 显式运行。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "需要真实 sherpa 模型目录（SHERPA_TEST_MODEL_DIR）"]
    async fn full_matcha_synthesis() {
        let model_dir = test_model_dir();
        assert!(
            !model_dir.is_empty() && Path::new(&model_dir).exists(),
            "模型目录不存在，请设置 SHERPA_TEST_MODEL_DIR: {model_dir}"
        );

        for use_gpu in [false, true] {
            let adapter = SherpaOnnxAdapter::new(
                TtsConfig::default(),
                model_dir.clone(),
                "matcha".to_string(),
                "zh".to_string(),
                "female".to_string(),
                use_gpu,
            )
            .unwrap_or_else(|e| panic!("use_gpu={use_gpu} 初始化失败: {e}"));

            let wav = adapter
                .generate_voice("你好，世界。这是一段语音合成测试。", "")
                .await
                .expect("生成失败");

            let (sr, samples) = parse_wav(&wav);
            assert_eq!(sr, 22050, "Matcha-zh 采样率应为 22050");
            assert!(samples >= 5, "生成音频过短");

            let data_start = find_wav_data_chunk(&wav).unwrap();
            let pcm = &wav[data_start..];
            let has_signal = pcm.as_chunks::<2>().0.iter().any(|c| {
                let v = i16::from_le_bytes(*c);
                v.unsigned_abs() > 200
            });
            assert!(has_signal, "use_gpu={use_gpu} 生成音频疑似全静音");
            eprintln!("use_gpu={use_gpu}: sr={sr}, 样本数={samples}, WAV-Ok");
        }
    }
}
