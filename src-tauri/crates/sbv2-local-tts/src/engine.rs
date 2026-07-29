// In-process SBV2 engine. All synthesis happens in spawn_blocking because
// ONNX is CPU-bound and `ort::Session` is `!Send + !Sync`; the holder is
// taken out of the async Mutex, used on the blocking pool, then put back.

use std::sync::Arc;

use sbv2_core::tts::{SynthesizeOptions, TTSModelHolder};
use tokio::sync::Mutex;

use super::paths::LocalTtsPaths;

pub struct LocalTtsEngine {
    holder: Arc<Mutex<Option<TTSModelHolder>>>,
    // Serialises the take-out-and-run-on-blocking-pool pattern used by
    // `load_voice` / `synthesize`. Without it, two fragments running
    // in parallel can both pass the `holder.lock().await` guard, each
    // `take()` the holder (leaving the inner cell as None), and the
    // loser reports `engine not initialized` even though init succeeded.
    serialize: Arc<Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct SynthesizeRequest {
    pub voice_id: String,
    pub text: String,
    pub style_id: i32,
    pub speaker_id: i64,
    pub sdp_ratio: f32,
    pub length_scale: f32,
}

impl LocalTtsEngine {
    pub fn new() -> Self {
        Self {
            holder: Arc::new(Mutex::new(None)),
            serialize: Arc::new(Mutex::new(())),
        }
    }

    pub async fn is_ready(&self) -> bool {
        self.holder.lock().await.is_some()
    }

    /// Initialize the holder from on-disk DeBerta + tokenizer bytes.
    pub async fn init(&self, paths: &LocalTtsPaths) -> std::result::Result<(), String> {
        let bert = tokio::fs::read(paths.deberta_dir().join("deberta.onnx"))
            .await
            .map_err(|e| format!("read deberta: {e}"))?;
        let tok = tokio::fs::read(paths.deberta_dir().join("tokenizer.json"))
            .await
            .map_err(|e| format!("read tokenizer: {e}"))?;

        let bert_clone = bert.clone();
        let tok_clone = tok.clone();
        let holder = tokio::task::spawn_blocking(move || {
            TTSModelHolder::new(bert_clone, tok_clone, Some(4))
        })
        .await
        .map_err(|e| format!("join: {e}"))?
        .map_err(|e| format!("TTSModelHolder::new: {e}"))?;

        let mut guard = self.holder.lock().await;
        *guard = Some(holder);
        Ok(())
    }

    /// Lazy-load a voice from disk into the holder.
    pub async fn load_voice(
        &self,
        paths: &LocalTtsPaths,
        voice_id: &str,
    ) -> std::result::Result<(), String> {
        let sbv2 = paths.voice_dir(voice_id).join("model.sbv2");
        let onnx = paths.voice_dir(voice_id).join("model.onnx");
        let (model_bytes, style_vectors) = if sbv2.exists() {
            (
                tokio::fs::read(&sbv2)
                    .await
                    .map_err(|e| format!("read sbv2: {e}"))?,
                None,
            )
        } else if onnx.exists() {
            let style_path = paths.voice_dir(voice_id).join("style_vectors.json");
            if !style_path.exists() {
                return Err(format!(
                    "voice {voice_id} is missing style_vectors.json required by model.onnx"
                ));
            }
            (
                tokio::fs::read(&onnx)
                    .await
                    .map_err(|e| format!("read onnx: {e}"))?,
                Some(
                    tokio::fs::read(&style_path)
                        .await
                        .map_err(|e| format!("read style_vectors.json: {e}"))?,
                ),
            )
        } else {
            return Err(format!("voice {voice_id} not installed"));
        };

        let vid = voice_id.to_string();
        // Hold the serialize guard for the entire take-out + run +
        // put-back window so concurrent fragments can't observe the
        // empty inner cell mid-flight.
        let _serialize_guard = self.serialize.lock().await;

        // Take the holder out of the mutex, run the load on a blocking
        // thread, then put it back. ort::Session is !Send + !Sync, so we
        // can't hold the guard across an await on spawn_blocking.
        let mut guard = self.holder.lock().await;
        let mut holder = guard.take().ok_or("engine not initialized")?;
        drop(guard);

        let vid_for_closure = vid.clone();
        // The closure owns holder, so we ship it back out alongside the
        // Result so we can reinsert it into the mutex below.
        let (holder, load_result) = tokio::task::spawn_blocking(move || {
            let r = match style_vectors {
                Some(style_bytes) => holder.load(vid_for_closure, style_bytes, model_bytes),
                None => holder.load_sbv2file(vid_for_closure, model_bytes),
            };
            (holder, r)
        })
        .await
        .map_err(|e| format!("join: {e}"))?;
        let load_result = load_result.map_err(|e| format!("load_sbv2file: {e}"));

        let mut guard = self.holder.lock().await;
        *guard = Some(holder);
        load_result
    }

    /// Synthesize speech to WAV bytes (CPU-bound, runs on blocking pool).
    pub async fn synthesize(&self, req: SynthesizeRequest) -> std::result::Result<Vec<u8>, String> {
        let options = SynthesizeOptions {
            sdp_ratio: req.sdp_ratio,
            length_scale: req.length_scale,
            style_weight: 1.0,
            split_sentences: true,
        };
        let voice_id = req.voice_id.clone();
        let text = req.text.clone();

        // Same serialisation rationale as `load_voice` above.
        let _serialize_guard = self.serialize.lock().await;

        let mut guard = self.holder.lock().await;
        let mut holder = guard.take().ok_or("engine not initialized")?;
        drop(guard);

        let voice_id2 = voice_id.clone();
        let style_id = req.style_id;
        let speaker_id = req.speaker_id;
        let (holder, result) = tokio::task::spawn_blocking(move || {
            let r = holder.easy_synthesize(&voice_id2, &text, style_id, speaker_id, options);
            (holder, r)
        })
        .await
        .map_err(|e| format!("join: {e}"))?;
        let result = result.map_err(|e| format!("synthesize: {e}"));

        let mut guard = self.holder.lock().await;
        *guard = Some(holder);
        result
    }
}

// Manual placeholder Debug: `TTSModelHolder` from sbv2_core owns ONNX
// `Session`s (no `Debug`), so a derived impl is not possible. We emit
// just the type name so the `Debug` derive on `VoiceMaker` keeps
// compiling. Fields stay hidden — anything sensitive is already opaque
// via the ONNX session.
impl std::fmt::Debug for LocalTtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalTtsEngine").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{LocalTtsEngine, SynthesizeRequest};
    use crate::paths::LocalTtsPaths;
    use std::path::{Path, PathBuf};

    fn fixture_voice_id(fixture_root: &Path) -> String {
        if let Ok(voice_id) = std::env::var("SBV2_FIXTURE_VOICE_ID") {
            if !voice_id.trim().is_empty() {
                return voice_id;
            }
        }

        let voices_dir = fixture_root.join("voices");
        let mut candidates: Vec<String> = std::fs::read_dir(&voices_dir)
            .unwrap_or_else(|e| panic!("read fixture voices {}: {e}", voices_dir.display()))
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| {
                let voice_dir = entry.path();
                let complete = voice_dir.join("model.sbv2").is_file()
                    || (voice_dir.join("model.onnx").is_file()
                        && voice_dir.join("style_vectors.json").is_file());
                complete.then(|| entry.file_name().to_string_lossy().into_owned())
            })
            .collect();
        candidates.sort();
        candidates.into_iter().next().unwrap_or_else(|| {
            panic!(
                "no complete voice fixture found under {}",
                voices_dir.display()
            )
        })
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fixture_happy_path_init_load_synthesize() {
        let Ok(fixture_dir) = std::env::var("SBV2_FIXTURE_DIR") else {
            eprintln!("SBV2_FIXTURE_DIR is not set; skipping model-backed happy-path test");
            return;
        };

        let fixture_root = PathBuf::from(fixture_dir);
        assert!(
            fixture_root.join("assets/deberta/deberta.onnx").is_file(),
            "fixture is missing assets/deberta/deberta.onnx"
        );
        assert!(
            fixture_root.join("assets/deberta/tokenizer.json").is_file(),
            "fixture is missing assets/deberta/tokenizer.json"
        );

        let voice_id = fixture_voice_id(&fixture_root);
        let cache = tempfile::tempdir().expect("create fixture cache");
        let paths = LocalTtsPaths {
            root: fixture_root.clone(),
            assets: fixture_root.join("assets"),
            voices: fixture_root.join("voices"),
            cache: cache.path().to_path_buf(),
        };
        let engine = LocalTtsEngine::new();

        engine
            .init(&paths)
            .await
            .expect("initialize fixture engine");
        assert!(engine.is_ready().await);
        engine
            .load_voice(&paths, &voice_id)
            .await
            .expect("load fixture voice");
        let wav = engine
            .synthesize(SynthesizeRequest {
                voice_id,
                text: "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}\u{3002}".to_owned(),
                style_id: 0,
                speaker_id: 0,
                sdp_ratio: 0.0,
                length_scale: 1.0,
            })
            .await
            .expect("synthesize fixture voice");

        assert!(wav.len() > 44, "WAV output must contain audio data");
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }
}
