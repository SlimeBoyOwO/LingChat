#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use std::os::raw::c_char;

/// 本次编译链接的预编译包变体：`cpu` / `cuda12` / `cuda13`。
///
/// 由 `build.rs` 按 `SHERPA_ONNX_GPU` 决定（见本目录 README）；调用方用它判断
/// 链接进来的 sherpa-onnx 是否带 CUDA —— CPU 包里 sherpa 自己会把 GPU provider
/// 降级到 CPU，而 CUDA 包里 provider 会被真的交给 ONNX Runtime。
pub const GPU_VARIANT: &str = env!("SHERPA_ONNX_GPU_VARIANT");

extern "C" {
    pub fn SherpaOnnxGetVersionStr() -> *const c_char;
    pub fn SherpaOnnxGetGitSha1() -> *const c_char;
    pub fn SherpaOnnxGetGitDate() -> *const c_char;
    pub fn SherpaOnnxGetOnnxruntimeVersionStr() -> *const c_char;
    pub fn SherpaOnnxFileExists(filename: *const c_char) -> i32;
}

pub mod audio_tagging;
pub mod kws;
pub mod offline_asr;
pub mod offline_punctuation;
pub mod offline_speaker_diarization;
pub mod online_asr;
pub mod online_punctuation;
pub mod resampler;
pub mod speaker_embedding;
pub mod speech_denoiser;
pub mod spoken_language_identification;
pub mod tts;
pub mod vad;
pub mod wave;

pub use audio_tagging::*;
pub use kws::*;
pub use offline_asr::*;
pub use offline_punctuation::*;
pub use offline_speaker_diarization::*;
pub use online_asr::*;
pub use online_punctuation::*;
pub use resampler::*;
pub use speaker_embedding::*;
pub use speech_denoiser::*;
pub use spoken_language_identification::*;
pub use tts::*;
pub use vad::*;
pub use wave::*;
