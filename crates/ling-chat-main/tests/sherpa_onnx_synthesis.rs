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
//! Windows DirectML 变体（自编 sherpa + 复用应用随包的 onnxruntime.dll，见
//! scripts/build-desktop-sherpa-dml.mjs；只在 Windows 上有效，其它平台 provider
//! 不会交给 sherpa）：
//! ```text
//! node scripts/build-desktop-sherpa-dml.mjs   # 先把 DML 版库编出来
//! SHERPA_ONNX_LIB_DIR=target/sherpa-onnx-directml/libs-v1.13.5 \
//! SHERPA_MODEL_DIR=~/models/matcha-icefall-zh-baker SHERPA_MODEL_TYPE=matcha SHERPA_USE_GPU=1 \
//!   cargo test -p ling-chat-main --features sherpa-shared --test sherpa_onnx_synthesis \
//!   -- --ignored --nocapture
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

use ling_chat_main::ai_service::tts::adapters::sherpa_onnx::{
    SherpaOnnxAdapter, SherpaTuning, load_reference_audio,
};
use ling_chat_main::ai_service::tts::provider::TtsAdapter;
use ling_chat_main::config::tts::TtsConfig;

/// 短测试句（12 字）。
const SHORT_TEXT: &str = "你好，这是一段测试语音。";

/// 用户可调的采样参数（对应设置页的两个滑动条）：
/// `SHERPA_NUM_STEPS` 推理步数、`SHERPA_GUIDANCE_SCALE` 引导强度（仅 ZipVoice 生效）。
fn tuning_from_env() -> SherpaTuning {
    SherpaTuning::from_options(
        std::env::var("SHERPA_NUM_STEPS")
            .ok()
            .and_then(|v| v.parse().ok()),
        std::env::var("SHERPA_GUIDANCE_SCALE")
            .ok()
            .and_then(|v| v.parse().ok()),
    )
}

/// 约 4 倍长、含标点与数字的测试句（59 字）。
const LONG_TEXT: &str = "老师早。今天天气很好，我们一起去散步吧？不过在那之前，\
我想先把昨天的作业交给你，顺便问问你有没有什么需要我帮忙的事情，\
比如第 3 章的那些题目。";

/// 数字读法回归：同一句话的「阿拉伯数字」与「汉字数字」两版。
///
/// 中文模型走 CharacterLexicon 时，非汉字在缺 rule_fsts 的情况下会被直接丢词
/// （日志 `Ignore OOV '3'`），用户听到的是「第 3 章」读成「第 章」。挂上模型自带的
/// `number.fst` 后阿拉伯数字会在分词前被改写成汉字，两版时长应该接近。
const DIGIT_TEXT: &str = "第 3 章很重要。";
const DIGIT_TEXT_HAN: &str = "第三章很重要。";

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

/// 静音判定阈值：与 sherpa 的 `ScaleSilence` 一致（0.01 满量程 ≈ -40 dBFS）。
const SILENCE_I16: i16 = 327;

/// 列出所有静音区间 → (起始秒, 时长秒)。
fn silence_runs(samples: &[i16], sample_rate: u32) -> Vec<(f64, f64)> {
    let sr = f64::from(sample_rate.max(1));
    let mut runs = Vec::new();
    let mut i = 0;
    while i < samples.len() {
        if samples[i].abs() > SILENCE_I16 {
            i += 1;
            continue;
        }
        let start = i;
        while i < samples.len() && samples[i].abs() <= SILENCE_I16 {
            i += 1;
        }
        runs.push((start as f64 / sr, (i - start) as f64 / sr));
    }
    runs
}

#[tokio::test]
#[ignore = "需要真实模型目录（SHERPA_MODEL_DIR）"]
async fn synthesis_is_audible_and_scales_with_text_length() {
    let model_dir = std::env::var("SHERPA_MODEL_DIR").expect("请设置 SHERPA_MODEL_DIR");
    let model_type = std::env::var("SHERPA_MODEL_TYPE").unwrap_or_else(|_| "matcha".into());
    let language = std::env::var("SHERPA_LANG").unwrap_or_else(|_| "zh".into());
    // 设 SHERPA_USE_GPU=1 可实跑一次 GPU 后端（只有 Windows DirectML 变体才有 GPU，
    // 见 scripts/build-desktop-sherpa-dml.mjs；其它平台 provider_for 直接按 CPU 走，
    // 不会把开关变成「看起来开了、其实降级」）。
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

    let mut adapter = SherpaOnnxAdapter::new(
        TtsConfig::default(),
        model_dir.clone(),
        model_type.clone(),
        language.clone(),
        "female".into(),
        use_gpu,
        tuning_from_env(),
    )
    .expect("初始化 Sherpa-ONNX 适配器失败");

    // 可选：挂参考音频（ZipVoice/Pocket 必需；其它模型忽略）。带这个开关就能在没有
    // 图形界面的机器上复现「换参考音频音色是否跟着变」「停顿是否合理」这类问题。
    if let Ok(ref_wav) = std::env::var("SHERPA_REF_WAV") {
        let ref_text = std::env::var("SHERPA_REF_TEXT").unwrap_or_default();
        let (samples, sample_rate) = load_reference_audio(std::path::Path::new(&ref_wav))
            .unwrap_or_else(|e| panic!("加载参考音频失败（{ref_wav}）: {e}"));
        adapter.set_reference_audio(samples, sample_rate, ref_text.clone());
        println!(
            "已挂载参考音频: {ref_wav}（{sample_rate} Hz，参考文本 {} 字）",
            ref_text.chars().count()
        );
    }

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
        measured.push((text.chars().count(), seconds, sample_rate, samples));
    }

    let (short_chars, short_seconds, _, _) = &measured[0];
    let (long_chars, long_seconds, long_sr, long_samples) = &measured[1];
    let (short_chars, short_seconds, long_chars, long_seconds) =
        (*short_chars, *short_seconds, *long_chars, *long_seconds);
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

    // 停顿回归（见 adapter 里的 SILENCE_SCALE）：一旦退回绑定默认的 0.2，超过 0.2 秒的
    // 静音会被压到两成，句号/问号后的停顿就和逗号后的一样短——用户侧表现就是
    // 「断句不清、读得赶」。长句里有「？」和「。」各一处，必须找得到明显的句读停顿。
    let runs: Vec<(f64, f64)> = silence_runs(long_samples, *long_sr)
        .into_iter()
        .filter(|(_, d)| *d >= 0.15)
        .collect();
    let longest_internal = runs
        .iter()
        .filter(|(start, _)| *start > 0.001)
        .map(|(_, d)| *d)
        .fold(0.0_f64, f64::max);
    println!(
        "长句停顿（≥150ms）{} 处，最长句内停顿 {:.0}ms：{:?}",
        runs.len(),
        longest_internal * 1000.0,
        runs.iter()
            .map(|(t, d)| format!("{:.2}s/{:.0}ms", t, d * 1000.0))
            .collect::<Vec<_>>()
    );
    assert!(
        longest_internal >= 0.25,
        "句读停顿被压得过短（最长 {:.0}ms）：检查 SILENCE_SCALE 是不是又被退回默认的 0.2",
        longest_internal * 1000.0
    );

    // 首尾静音要裁到上限内：多段台词顺序播放时，段首那段空白就是「等它开口」。
    let lead = runs
        .iter()
        .find(|(start, _)| *start <= 0.001)
        .map(|(_, d)| *d)
        .unwrap_or(0.0);
    let trail = runs
        .iter()
        .filter(|(start, d)| start + d >= long_seconds - 0.001)
        .map(|(_, d)| *d)
        .fold(0.0_f64, f64::max);
    assert!(lead <= 0.15, "段首静音未裁干净：{:.0}ms", lead * 1000.0);
    assert!(trail <= 0.2, "段尾静音未裁干净：{:.0}ms", trail * 1000.0);

    // 数字读法回归：只对「模型目录里带规则 FST」的模型断言——那正是 with_rule_files 真正
    // 挂上去的东西（matcha-zh 的 phone/date/number.fst、kokoro 的 *-zh.fst）。
    // zipvoice 的 impl 不消费 rule_fsts（上游实现如此），数字走 espeak 英文路径，且它的
    // 时长抖动有 ±0.3 秒，用时长断言会假失败，这里直接跳过。
    let rule_fst = std::fs::read_dir(&model_dir).ok().and_then(|it| {
        it.flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .find(|name| name.ends_with(".fst"))
    });
    match rule_fst {
        Some(name) => {
            println!("数字读法回归：模型目录带规则 FST（{name}），检查阿拉伯数字是否被改写");
            let mut digit_seconds = 0.0;
            for (label, text) in [("阿拉伯数字", DIGIT_TEXT), ("汉字数字", DIGIT_TEXT_HAN)]
            {
                let wav = adapter
                    .generate_voice(text, "")
                    .await
                    .unwrap_or_else(|e| panic!("{label}数字回归合成失败: {e}"));
                let (sr, samples) = parse_wav(&wav);
                let seconds = samples.len() as f64 / f64::from(sr);
                println!("数字读法: {label}「{text}」→ {seconds:.3}s");
                if label == "阿拉伯数字" {
                    digit_seconds = seconds;
                } else {
                    assert!(
                        digit_seconds >= seconds * 0.8,
                        "阿拉伯数字疑似被丢词：「{DIGIT_TEXT}」{digit_seconds:.3}s vs \
                         「{DIGIT_TEXT_HAN}」{seconds:.3}s（检查 rule_fsts 是否被挂上）"
                    );
                }
            }
        },
        None => println!(
            "数字读法回归：{model_dir} 里没有规则 FST，跳过（这类模型由 chinese_digits 在 Rust 侧改写）"
        ),
    }

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

/// 用 `SHERPA_TEXT` 指定的文本合成一次（配合 `SHERPA_OUT_WAV` 导出），用来排查单个句子
/// 的读音/分词问题，不必改测试里的固定文本。
#[tokio::test]
#[ignore = "需要真实模型目录（SHERPA_MODEL_DIR）+ SHERPA_TEXT"]
async fn synth_env_text() {
    let model_dir = std::env::var("SHERPA_MODEL_DIR").expect("请设置 SHERPA_MODEL_DIR");
    let model_type = std::env::var("SHERPA_MODEL_TYPE").expect("请设置 SHERPA_MODEL_TYPE");
    let lang = std::env::var("SHERPA_LANG").unwrap_or_else(|_| "zh".to_string());
    let text = std::env::var("SHERPA_TEXT").expect("请设置 SHERPA_TEXT");

    let mut adapter = SherpaOnnxAdapter::new(
        TtsConfig::default(),
        model_dir.clone(),
        model_type.clone(),
        lang.clone(),
        String::new(),
        std::env::var("SHERPA_USE_GPU").is_ok(),
        tuning_from_env(),
    )
    .expect("初始化适配器失败");

    if let Ok(ref_wav) = std::env::var("SHERPA_REF_WAV") {
        let ref_text = std::env::var("SHERPA_REF_TEXT").unwrap_or_default();
        let (samples, sample_rate) = load_reference_audio(std::path::Path::new(&ref_wav))
            .unwrap_or_else(|e| panic!("加载参考音频失败（{ref_wav}）: {e}"));
        adapter.set_reference_audio(samples, sample_rate, ref_text);
    }

    let emo = std::env::var("SHERPA_EMO").unwrap_or_default();
    let wav = adapter.generate_voice(&text, &emo).await.expect("合成失败");
    let (sr, samples) = parse_wav(&wav);
    println!(
        "自定义文本 @ {model_type}/{lang}「{text}」情绪「{emo}」→ {:.3}s sr={sr}",
        samples.len() as f64 / f64::from(sr)
    );

    if let Ok(out) = std::env::var("SHERPA_OUT_WAV") {
        std::fs::write(&out, &wav).expect("写出 WAV 失败");
        println!("已写出试听文件: {out}");
    }
}
