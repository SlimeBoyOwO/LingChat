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
    BaseDecoder, BaseDecoderConfig, CorpusConfig, TextFrontend, build_corpus, export_tuned_model,
    load_checkpoint, load_decoder_weights, read_initializers, train_base, BaseEncoderConfig,
    BaseTrainingConfig,
};
use ling_chat_main::ai_service::tts::provider::TtsAdapter;
use ling_chat_main::config::tts::TtsConfig;
use tokio_util::sync::CancellationToken;

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

/// 完整链路验收：合成 wav → 导入 → 训练 → 导出独立音色 → sherpa 加载出声。
///
/// 与真实用户操作路径一致（不直接读基座权重，而是从导入数据集训练出检查点再导出）。
/// 需要真实基座目录（SHERPA_MODEL_DIR），默认忽略。
#[tokio::test]
#[ignore = "需要真实基座目录（SHERPA_MODEL_DIR），跑训练又较慢"]
async fn full_pipeline_import_train_export_loads_sherpa() {
    let base = base_dir();
    assert!(base.is_dir(), "基座目录不存在: {}", base.display());
    let tokens_path = base.join("tokens.txt");
    assert!(tokens_path.is_file(), "基座缺少 tokens.txt");

    // 1) 造一个数据集目录：合成 wav + dataset.json 清单（等价于导入命令产物，
    //    避开对 App 初始化的 data_dir 依赖）。导入命令层已由 import 模块单测覆盖。
    let dataset_id = format!("e2e_pipe_{}", std::process::id());
    let root = std::env::temp_dir().join(&dataset_id);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("wavs")).unwrap();
    let texts = [
        "你好世界",
        "今天我们测试语音合成",
        "这是一段用于微调的音频",
    ];
    let items: Vec<serde_json::Value> = texts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let id = format!("s{i}");
            write_synth_wav(&root.join(format!("wavs/{id}.wav")), 17_152, 22_050);
            serde_json::json!({
                "id": id,
                "wav": format!("wavs/{id}.wav"),
                "text": text,
                "duration_secs": 17_152.0 / 22_050.0,
            })
        })
        .collect();
    let manifest = serde_json::json!({
        "version": 1,
        "dataset_id": dataset_id,
        "sample_rate": 22_050,
        "created_at_unix": 0,
        "items": items,
        "skipped": [],
        "stats": {
            "total_items": texts.len(),
            "items_with_text": texts.len(),
            "total_secs": 0.8 * texts.len() as f64,
            "min_required_secs": 300.0,
            "meets_minimum": false,
        },
    });
    std::fs::write(root.join("dataset.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();

    // 2) 组装语料并训练出检查点（epochs=2，快速跑通链路）。
    let tokens_path = base.join("tokens.txt");
    let lexicon_path = base.join("lexicon.txt").is_file().then_some(base.join("lexicon.txt"));
    let frontend = TextFrontend::load(&tokens_path, lexicon_path.as_deref()).unwrap();
    let corpus = build_corpus(&root, &frontend, &CorpusConfig::default()).unwrap();
    assert!(!corpus.samples.is_empty(), "训练语料为空");
    let mel = ling_chat_main::ai_service::tts::finetune::MelConfig::default();
    let encoder_config = BaseEncoderConfig {
        n_vocab: frontend.tokens().len(),
        n_mels: mel.n_mels,
        ..Default::default()
    };
    let decoder_config = BaseDecoderConfig {
        n_mels: mel.n_mels,
        ..Default::default()
    };
    let train_config = BaseTrainingConfig {
        epochs: 2,
        ..Default::default()
    };
    let onnx = base.join("model-steps-3.onnx");
    let checkpoint = root.join("checkpoint.safetensors");
    let device = Device::Cpu;
    train_base(
        &corpus,
        &encoder_config,
        &decoder_config,
        &train_config,
        &onnx,
        Some(&checkpoint),
        Some(&CancellationToken::new()),
        &device,
        |_| {},
    )
    .unwrap();
    assert!(checkpoint.is_file(), "训练没产出检查点");

    // 4) 从检查点装载解码器并导出声学模型，组装独立音色目录。
    let weight_map = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/tts-training/decoder_weight_map.json");
    let mut var_map = VarMap::new();
    let vb = VarBuilder::from_varmap(&var_map, DType::F32, &device);
    BaseDecoder::new(&BaseDecoderConfig::default(), vb).unwrap();
    load_checkpoint(&mut var_map, &checkpoint).unwrap();
    let out_dir = std::env::temp_dir().join(format!("{dataset_id}-model"));
    let tmp = std::env::temp_dir().join(format!("{dataset_id}-export.onnx"));
    export_tuned_model(&var_map, &onnx, &weight_map, &tmp).unwrap();
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::rename(&tmp, out_dir.join("model-steps-3.onnx")).unwrap();
    for f in ["tokens.txt", "lexicon.txt", "vocos-22khz-univ.onnx"] {
        let src = base.join(f);
        if src.is_file() {
            std::fs::copy(&src, out_dir.join(f)).unwrap();
        }
    }

    // 5) sherpa 加载训练出的音色并出声。
    let adapter = SherpaOnnxAdapter::new(
        TtsConfig::default(),
        out_dir.to_string_lossy().to_string(),
        "matcha".into(),
        "zh".into(),
        "female".into(),
        false,
        ling_chat_main::ai_service::tts::adapters::sherpa_onnx::SherpaTuning::default(),
    )
    .expect("sherpa 加载微调音色失败");

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
    // 2 个 epoch 的验证性训练产物，幅度不一定逼近基座，判据是「确实有声」。
    assert!(peak > 100, "输出疑似静音（峰值 {peak}）");

    println!(
        "完整链路（导入→训练→导出→sherpa 出声）验收通过：{}（WAV {} 字节，{} 样本 {} 步）",
        out_dir.display(),
        wav.len(),
        corpus.samples.len(),
        corpus.samples.len() * train_config.epochs,
    );
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&root);
}

/// 写一段 16-bit PCM 单声道正弦 wav 到文件。
fn write_synth_wav(path: &Path, samples: usize, sample_rate: u32) {
    let samples: Vec<i16> = (0..samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            ((2.0 * std::f32::consts::PI * 220.0 * t).sin() * 8_000.0
                + (4.0 * std::f32::consts::PI * 220.0 * t).sin() * 2_000.0)
                as i16
        })
        .collect();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for s in samples {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
}
