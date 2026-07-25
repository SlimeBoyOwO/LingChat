//! Standalone end-to-end smoke binary for the local SBV2 TTS pipeline on
//! Windows. Drives sbv2_core directly (no Tauri runtime needed) so it
//! can run in a regular `cargo run` without feature gating.
//!
//! Pipeline:
//!   1. Read FP32 DeBERTa + tokenizer bytes from disk.
//!   2. Build a `TTSModelHolder` (this is where the original
//!      `com.microsoft.Gelu(1) tensor(float16)` error used to surface
//!      on the CPU EP).
//!   3. Load a voice (model.onnx + style_vectors.json).
//!   4. Synthesize a short phrase and write the WAV bytes.
//!
//! Usage:
//!     sbv2_local_smoke <deberta.onnx> <tokenizer.json> \
//!                      <voice_model.onnx> <style_vectors.json> \
//!                      <output.wav> [text] [speaker_id]
//!
//! Exit code 0 on success, non-zero otherwise.
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use sbv2_core::tts::{SynthesizeOptions, TTSModelHolder};

fn wav_summary(path: &PathBuf) -> Result<(u32, u16, f32), String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(format!("not a valid RIFF/WAVE file: {}", path.display()));
    }
    let mut off = 12usize;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut byte_rate = 0u32;
    let mut data_len = 0u32;
    while off + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap()) as usize;
        let body = off + 8;
        if &bytes[off..off + 4] == b"fmt " && len >= 10 {
            channels = u16::from_le_bytes(bytes[body + 2..body + 4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().unwrap());
            byte_rate = u32::from_le_bytes(bytes[body + 8..body + 12].try_into().unwrap());
        } else if &bytes[off..off + 4] == b"data" {
            data_len = len as u32;
        }
        off = body + len + (len & 1);
        if off > bytes.len() {
            break;
        }
    }
    let duration = if byte_rate > 0 { data_len as f32 / byte_rate as f32 } else { 0.0 };
    Ok((sample_rate, channels, duration))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 {
        eprintln!(
            "usage: {} <deberta.onnx> <tokenizer.json> <voice_model.onnx> \
             <style_vectors.json> <output.wav> [text] [speaker_id]",
            args.first().map(|s| s.as_str()).unwrap_or("sbv2_local_smoke")
        );
        std::process::exit(2);
    }
    let deberta_path = PathBuf::from(&args[1]);
    let tokenizer_path = PathBuf::from(&args[2]);
    let voice_onnx_path = PathBuf::from(&args[3]);
    let style_path = PathBuf::from(&args[4]);
    let out_wav_path = PathBuf::from(&args[5]);
    let text = args.get(6).cloned().unwrap_or_else(|| "\u{4f60}\u{597d}\u{ff0c}\u{4e16}\u{754c}\u{3002}".to_string());
    let speaker_id: i64 = args
        .get(7)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let voice_id = "smoke-voice";

    println!("[step] reading fixtures");
    let t0 = Instant::now();
    let bert = fs::read(&deberta_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", deberta_path.display()));
    let tok = fs::read(&tokenizer_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", tokenizer_path.display()));
    let voice_onnx = fs::read(&voice_onnx_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", voice_onnx_path.display()));
    let style = fs::read(&style_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", style_path.display()));
    println!(
        "  bert={} tok={} voice={} style={} ({} ms)",
        bert.len(),
        tok.len(),
        voice_onnx.len(),
        style.len(),
        t0.elapsed().as_millis()
    );

    println!("[step] TTSModelHolder::new (this is where the old Gelu/F16 error fired)");
    let t1 = Instant::now();
    let mut holder = TTSModelHolder::new(bert, tok, Some(4))
        .unwrap_or_else(|e| panic!("TTSModelHolder::new failed: {e:?}"));
    println!("  holder ready ({} ms)", t1.elapsed().as_millis());

    println!("[step] loading voice `{voice_id}`");
    let t2 = Instant::now();
    holder
        .load(voice_id, style, voice_onnx)
        .unwrap_or_else(|e| panic!("voice load failed: {e:?}"));
    println!("  voice ready ({} ms)", t2.elapsed().as_millis());

    println!("[step] synthesizing text=\"{text}\" speaker_id={speaker_id}");
    let t3 = Instant::now();
    let wav = holder
        .easy_synthesize(
            voice_id,
            &text,
            0,
            speaker_id,
            SynthesizeOptions {
                sdp_ratio: 0.2,
                length_scale: 1.0,
                style_weight: 1.0,
                split_sentences: true,
            },
        )
        .unwrap_or_else(|e| panic!("synthesize failed: {e:?}"));
    println!(
        "  synthesize OK wav_bytes={} ({} ms)",
        wav.len(),
        t3.elapsed().as_millis()
    );

    let mut f = fs::File::create(&out_wav_path)
        .unwrap_or_else(|e| panic!("create {}: {e}", out_wav_path.display()));
    f.write_all(&wav)
        .unwrap_or_else(|e| panic!("write {}: {e}", out_wav_path.display()));
    println!("[step] wrote WAV to {}", out_wav_path.display());

    match wav_summary(&out_wav_path) {
        Ok((sr, ch, dur)) => {
            println!(
                "[summary] sample_rate={sr} channels={ch} duration={dur:.3}s"
            );
        }
        Err(e) => {
            eprintln!("[summary] WAV summary failed: {e}");
        }
    }

    println!("[ok] smoke complete");
}
