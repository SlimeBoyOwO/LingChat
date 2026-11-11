//! 端侧音色微调：数据导入、归一化与训练，全程进程内完成，不依赖外部二进制。

pub mod burn_backend;
pub mod burn_decoder;
pub mod burn_encoder;
pub mod burn_training;
pub mod conv_lora;
pub mod corpus;
pub mod decoder;
pub mod feature;
pub mod import;
pub mod model;
pub mod normalize;
pub mod onnx_export;
pub mod onnx_weights;
pub mod resample;
pub mod style;
pub mod text;
pub mod training;

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
pub use onnx_export::{
    export_tuned_model_burn, export_tuned_model_burn_with_adapters,
    export_tuned_model_burn_with_duration,
};
pub use onnx_weights::{OnnxTensor, read_initializers};
pub use style::{
    DEFAULT_STYLE_CLUSTERS, MAX_STYLE_CLUSTERS, StyleClusterReport, StyleClusterSummary,
    cluster_dataset, summarize_clusters,
};
pub use text::{Lexicon, TextFrontend, TokenTable};
pub use training::{
    TrainingConfig, TrainingOutcome, TrainingProgress, load_checkpoint, save_checkpoint, train,
};

/// 数据集默认归一化采样率（22.05 kHz）；模型采样率不同时由调用方覆盖。
pub const DEFAULT_SAMPLE_RATE: u32 = 22_050;
