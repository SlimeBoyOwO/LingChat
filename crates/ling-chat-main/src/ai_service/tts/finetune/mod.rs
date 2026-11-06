//! 端侧音色微调：数据集导入、音频归一化与模型训练。
//!
//! 训练要同时跑在桌面和手机上，因此这一层不调用任何外部二进制，解码、重采样、
//! 归一化全部在进程内完成。数据集统一放在应用数据目录下的
//! `sherpa_onnx_finetune/<dataset_id>/`：
//!
//! - `raw/`：导入时解包出来的原始素材
//! - `wavs/`：归一化后的 16-bit PCM 单声道 WAV
//! - `metadata.csv`：ljspeech 风格清单，wavs/<id>.wav|文本|speaker
//! - `dataset.json`：完整清单、统计与跳过原因
//!
//! 目前落地的是数据管线；训练引擎读取 `dataset.json` 与 `wavs/`，产出
//! sherpa-onnx 可直接加载的 ONNX。

pub mod base_decoder;
pub mod base_encoder;
pub mod burn_backend;
pub mod burn_encoder;
pub mod burn_decoder;
pub mod burn_training;
pub use base_decoder::{BaseDecoder, BaseDecoderConfig};
pub use base_encoder::{BaseEncoder, BaseEncoderConfig};
pub mod base_training;
pub mod corpus;
pub mod decoder;
pub mod feature;
pub mod import;
pub mod model;
pub mod normalize;
pub mod onnx_export;
pub mod onnx_weights;
pub mod resample;
pub mod text;
pub mod training;

pub use base_training::{
    BaseTrainingConfig, BaseTrainingOutcome, BaseTrainingProgress, load_decoder_weights, train_base,
};
pub use corpus::{
    Corpus, CorpusConfig, CorpusStats, SkippedSample, TrainingSample, build_corpus, make_batches,
};
pub use decoder::{CfmConfig, CfmDecoder, flow_matching_loss};
pub use feature::{MelConfig, MelFilterbank, log_mel, stft_magnitude};
pub use import::{
    DatasetItem, DatasetManifest, DatasetStats, ImportReport, SkippedItem, finetune_root,
    import_zip, list_datasets, load_manifest, persist_text_updates, remove_dataset,
};
pub use model::{Matcha, MatchaConfig};
pub use normalize::{NormalizeReport, normalize_to_wav};
pub use onnx_export::{export_tuned_model, export_tuned_model_burn};
pub use onnx_weights::{OnnxTensor, read_initializers};
pub use text::{Lexicon, TextFrontend, TokenTable};
pub use training::{
    TrainingConfig, TrainingOutcome, TrainingProgress, load_checkpoint, save_checkpoint, train,
};

/// 数据集默认归一化采样率。
///
/// 与 sherpa-onnx 常用的 VITS / Matcha 模型保持一致（22.05 kHz）。模型采样率不同时
/// 由调用方显式传入覆盖。
pub const DEFAULT_SAMPLE_RATE: u32 = 22_050;
