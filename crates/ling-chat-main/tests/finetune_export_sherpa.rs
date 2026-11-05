//! 端侧音色微调导出 → sherpa 加载端到端验收。
//!
//! 从基座读取解码器权重（等于未微调副本）导出成声学 ONNX，与配套文件组装成
//! 独立模型目录，再用 sherpa matcha 引擎合成一段，确认「sherpa 能加载导出的
//! 模型目录并出声」这条任务6 核心链路成立。
//!
//! 需要真实基座目录（含 model-steps-3.onnx/tokens.txt/vocos），默认忽略。
//! ```text
//! SHERPA_MODEL_DIR=<基座目录> cargo test -p ling-chat-main \
//!   --test finetune_export_sherpa -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};

use candle_core::{DType, Device};
use candle_nn::{VarBuilder, VarMap};
use ling_chat_main::ai_service::tts::adapters::sherpa_onnx::SherpaOnnxAdapter;
use ling_chat_main::ai_service::tts::finetune::{
    BaseDecoder, BaseDecoderConfig, export_tuned_model, load_decoder_weights, read_initializers,
};
use ling_chat_main::ai_service::tts::provider::TtsAdapter;
use ling_chat_main::config::tts::TtsConfig;

fn base_dir() -> PathBuf {
    std::env::var("SHERPA_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../data/sherpa_onnx_models/matcha-icefall-zh-baker")
        })
}

#[tokio::test]
#[ignore = "需要真实基座目录（SHERPA_MODEL_DIR），含模型与声码器"]
async fn exported_model_directory_is_loadable_by_sherpa_and_audible() {
    let base = base_dir();
    assert!(base.is_dir(), "基座目录不存在: {}", base.display());

    let onnx = base.join("model-steps-3.onnx");
    let weight_map = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/tts-training/decoder_weight_map.json");
    assert!(onnx.is_file(), "基座模型不存在: {}", onnx.display());
    assert!(weight_map.is_file(), "权重映射表不存在");

    let device = Device::Cpu;
    // 从基座读解码器权重（等于「未微调」副本），导出成声学 ONNX。
    let tensors = read_initializers(&onnx).unwrap();
    let mut var_map = VarMap::new();
    let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
    BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();
    load_decoder_weights(&mut var_map, &tensors, &weight_map, &device).unwrap();

    let tmp = std::env::temp_dir().join("lingchat-fin-export.onnx");
    export_tuned_model(&var_map, &onnx, &weight_map, &tmp).unwrap();

    // 组装独立模型目录：声学模型 + 配套文件。
    let out_dir = std::env::temp_dir().join("lingchat-fin-export-model");
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::rename(&tmp, out_dir.join("model-steps-3.onnx")).unwrap();
    for f in ["tokens.txt", "lexicon.txt", "vocos-22khz-univ.onnx"] {
        let src = base.join(f);
        if src.is_file() {
            std::fs::copy(&src, out_dir.join(f)).unwrap();
        }
    }

    // sherpa 加载导出的模型目录并合成。
    let adapter = SherpaOnnxAdapter::new(
        TtsConfig::default(),
        out_dir.to_string_lossy().to_string(),
        "matcha".into(),
        "zh".into(),
        "female".into(),
        false,
        ling_chat_main::ai_service::tts::adapters::sherpa_onnx::SherpaTuning::default(),
    )
    .expect("sherpa 加载导出的模型失败");

    let wav = adapter
        .generate_voice("你好，这是一段测试语音。", "")
        .await
        .unwrap();
    assert!(wav.len() > 44, "WAV 输出过短");
    let pcm = &wav[44..];
    let samples: Vec<i16> = pcm
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect();
    let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    let sum_sq: f64 = samples
        .iter()
        .map(|s| i64::from(*s) * i64::from(*s))
        .sum::<i64>() as f64;
    let rms = (sum_sq / samples.len().max(1) as f64).sqrt();
    assert!(peak > 1000, "输出疑似静音（峰值 {peak}）");
    assert!(rms > 100.0, "输出能量过低（RMS {rms:.1}）");

    println!(
        "导出模型目录被 sherpa 加载并出声成功：{}（{} 字节）",
        out_dir.display(),
        wav.len()
    );
    let _ = std::fs::remove_dir_all(&out_dir);
}
