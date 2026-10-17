//! Sherpa-ONNX TTS 输出的「能出声 + 时长随文本增长」回归检查。
//!
//! 背景：这类故障不会报错。最典型的是 kokoro 系列——不挂 lexicon 时中文会退到
//! espeak-ng 音素化，实测只产出 0.2~0.5 秒的噪声甚至纯静音；用户侧表现就是
//! 「日志显示能推理，但合成出来的不到 1 秒、效果极差」。因此这里在输出层面兜底：
//! 断言峰值非静音，并且长文本的输出时长要按比例增长（防止被截断成一小段）。
//!
//! 需要真实模型目录，默认忽略。
//!
//! 本机（Linux/macOS/Windows）运行：
//! ```text
//! SHERPA_MODEL_DIR=~/models/matcha-icefall-zh-baker SHERPA_MODEL_TYPE=matcha \
//!   cargo test -p ling-chat-main --test sherpa_onnx_synthesis -- --ignored --nocapture
//! ```
//!
//! Android 实机运行（不必重装 APK：交叉编译本测试再 push 到设备即可，模型的路径
//! 直接指到应用外部数据目录；`libc++_shared.so` 与 sherpa 的 .so 也要一起推）：
//! ```text
//! NDK=$ANDROID_NDK_HOME TC=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
//! export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$TC/aarch64-linux-android24-clang
//! export CC_aarch64_linux_android=$TC/aarch64-linux-android24-clang
//! export AR_aarch64_linux_android=$TC/llvm-ar
//! export SHERPA_ONNX_LIB_DIR=src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a
//! cargo build -p ling-chat-main --test sherpa_onnx_synthesis --target aarch64-linux-android
//! adb push target/aarch64-linux-android/debug/deps/sherpa_onnx_synthesis-* /data/local/tmp/probe
//! adb push $SHERPA_ONNX_LIB_DIR/*.so /data/local/tmp/
//! adb shell "cd /data/local/tmp && chmod 755 probe && LD_LIBRARY_PATH=/data/local/tmp \
//!   SHERPA_MODEL_DIR='/storage/emulated/0/Android/data/com.noiq.lingchat/files/sherpa_onnx_models/matcha-icefall-zh-baker' \
//!   SHERPA_MODEL_TYPE=matcha ./probe --ignored --nocapture --test-threads=1"
//! ```

use ling_chat_main::ai_service::tts::adapters::sherpa_onnx::SherpaOnnxAdapter;
use ling_chat_main::ai_service::tts::provider::TtsAdapter;
use ling_chat_main::config::tts::TtsConfig;

/// 短测试句（12 字）。
const SHORT_TEXT: &str = "你好，这是一段测试语音。";

/// 约 4 倍长、含标点与数字的测试句（59 字）。
const LONG_TEXT: &str = "老师早。今天天气很好，我们一起去散步吧？不过在那之前，\
我想先把昨天的作业交给你，顺便问问你有没有什么需要我帮忙的事情，\
比如第 3 章的那些题目。";

/// 解析 44 字节 WAV 头并取出单声道 16 位样本。
fn parse_wav(wav: &[u8]) -> (u32, Vec<i16>) {
    assert!(wav.len() > 44, "WAV 太短: {} 字节", wav.len());
    assert_eq!(&wav[0..4], b"RIFF", "缺 RIFF 头");
    assert_eq!(&wav[8..12], b"WAVE", "缺 WAVE");
    let sample_rate = u32::from_le_bytes(wav[24..28].try_into().unwrap());
    let bits = u16::from_le_bytes(wav[34..36].try_into().unwrap());
    assert_eq!(bits, 16, "期望 16 位输出，实际 {bits} 位");
    let samples = wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect();
    (sample_rate, samples)
}

#[tokio::test]
#[ignore = "需要真实模型目录（SHERPA_MODEL_DIR）"]
async fn synthesis_is_audible_and_scales_with_text_length() {
    let model_dir = std::env::var("SHERPA_MODEL_DIR").expect("请设置 SHERPA_MODEL_DIR");
    let model_type = std::env::var("SHERPA_MODEL_TYPE").unwrap_or_else(|_| "matcha".into());
    let language = std::env::var("SHERPA_LANG").unwrap_or_else(|_| "zh".into());
    // 设 SHERPA_USE_GPU=1 可实跑一次 GPU 后端（需带 GPU 的预编译包，见
    // crates/sherpa-onnx-sys/build.rs）；后端不可用时会回退 CPU。
    let use_gpu = std::env::var("SHERPA_USE_GPU").is_ok_and(|v| v != "0" && !v.is_empty());
    // 排查 provider 选择/回退原因时加 `RUST_LOG=info`（GPU 相关的告警都在 tracing 里）。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
    assert!(
        std::path::Path::new(&model_dir).is_dir(),
        "模型目录不存在: {model_dir}"
    );

    let adapter = SherpaOnnxAdapter::new(
        TtsConfig::default(),
        model_dir.clone(),
        model_type.clone(),
        language.clone(),
        "female".into(),
        use_gpu,
    )
    .expect("初始化 Sherpa-ONNX 适配器失败");

    let mut measured = Vec::new();
    let mut total_elapsed = std::time::Duration::ZERO;
    for (label, text) in [("短句", SHORT_TEXT), ("长句", LONG_TEXT)] {
        let started = std::time::Instant::now();
        let wav = adapter
            .generate_voice(text, "")
            .await
            .unwrap_or_else(|e| panic!("{label}合成失败（{model_type}）: {e}"));
        let elapsed = started.elapsed();
        total_elapsed += elapsed;
        let (sample_rate, samples) = parse_wav(&wav);
        let seconds = samples.len() as f64 / sample_rate as f64;
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        let rms = (samples.iter().map(|s| (f64::from(*s)).powi(2)).sum::<f64>()
            / samples.len().max(1) as f64)
            .sqrt();

        println!(
            "{label}: {} 字 @ {model_type}/{language} → 音频 {seconds:.3}s sr={sample_rate} 峰值={peak} RMS={rms:.1} \
             用时={elapsed:.3?}（{:.1}x 实时）",
            text.chars().count(),
            seconds / elapsed.as_secs_f64()
        );

        // 静音/噪声：kokoro 缺 lexicon、int8 模型在部分 arm64 设备上都会命中这里。
        assert!(peak > 1000, "{label}输出疑似静音（峰值 {peak}）");
        assert!(rms > 100.0, "{label}输出能量过低（RMS {rms:.1}）");
        measured.push((text.chars().count(), seconds));
    }

    let (short_chars, short_seconds) = measured[0];
    let (long_chars, long_seconds) = measured[1];
    let char_ratio = long_chars as f64 / short_chars as f64;
    println!(
        "时长比 {:.2}（字数比 {char_ratio:.2}）：短句 {short_seconds:.3}s → 长句 {long_seconds:.3}s",
        long_seconds / short_seconds
    );
    // 长句字数约为短句的 4~5 倍；语音时长应同量级增长。若被截断（例如只返回
    // 一句话的音频），这里会明显偏低。
    assert!(
        long_seconds >= short_seconds * char_ratio * 0.6,
        "长句时长未随文本增长：短句 {short_seconds:.3}s / 长句 {long_seconds:.3}s（字数比 {char_ratio:.2}）"
    );

    // 对比 CPU / GPU 后端性能时看这一行（音频时长本身当然和 provider 无关）。
    println!(
        "合计推理用时 {total_elapsed:.3?}，音频合计 {:.3}s",
        short_seconds + long_seconds
    );

    if let Ok(out) = std::env::var("SHERPA_OUT_WAV") {
        let wav = adapter
            .generate_voice(LONG_TEXT, "")
            .await
            .expect("导出合成失败");
        std::fs::write(&out, &wav).expect("写出 WAV 失败");
        println!("已写出试听文件: {out}");
    }
}
