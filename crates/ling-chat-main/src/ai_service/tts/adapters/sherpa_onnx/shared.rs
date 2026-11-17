//! Sherpa-ONNX 共享引擎缓存。
//!
//! 同一个模型目录、模型类型、执行后端和采样参数只加载一份 `OfflineTts`；
//! 不同角色的 sid、语速、参考音频仍由各自的 `SherpaOnnxAdapter` 持有。
//!
//! 缓存使用 `Weak`：最后一个适配器释放后，引擎会随 `Arc` 自动销毁，
//! 不会因为“曾经加载过”而永久占住模型内存。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use anyhow::Result;
use sherpa_onnx::OfflineTts;

use super::SherpaTuning;

/// 一个可跨角色复用的推理引擎。
pub(super) struct SharedSherpaEngine {
    pub(super) tts: OfflineTts,
    pub(super) sample_rate: i32,
    /// 串行化同一引擎上的合成，避免多个角色同时调用原生 TTS。
    pub(super) generate_lock: tokio::sync::Mutex<()>,
}

impl SharedSherpaEngine {
    pub(super) fn new(tts: OfflineTts) -> Self {
        let sample_rate = tts.sample_rate();
        Self {
            tts,
            sample_rate,
            generate_lock: tokio::sync::Mutex::new(()),
        }
    }
}

/// 共享引擎的缓存键。只有这些参数会影响 `OfflineTts` 的构造；
/// 角色级的 sid / speed / reference audio 不参与，因此可以安全复用。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct EngineKey {
    model_dir: PathBuf,
    model_type: String,
    /// ZipVoice 的 flow-matching 参数在创建引擎时固定。
    /// 其它模型不读取这两个值，统一归一化以便尽可能共享。
    num_steps: i32,
    guidance_scale_bits: u32,
}

impl EngineKey {
    pub(super) fn new(model_dir: &Path, model_type: &str, tuning: SherpaTuning) -> Self {
        let model_dir =
            std::fs::canonicalize(model_dir).unwrap_or_else(|_| model_dir.to_path_buf());
        let tuning = if model_type == "zipvoice" {
            tuning
        } else {
            SherpaTuning::default()
        };

        Self {
            model_dir,
            model_type: model_type.to_string(),
            num_steps: tuning.num_steps,
            guidance_scale_bits: tuning.guidance_scale.to_bits(),
        }
    }
}

type Registry = Mutex<HashMap<EngineKey, Weak<SharedSherpaEngine>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_registry() -> std::sync::MutexGuard<'static, HashMap<EngineKey, Weak<SharedSherpaEngine>>> {
    registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 取回已加载的共享引擎；不存在时调用 `create` 加载。
///
/// 返回 `(engine, reused)`，`reused` 表示命中缓存。为了不让模型加载阻塞
/// 其它模型的读取，锁只在查表和插入时短暂持有；极少数并发首次加载可能
/// 多加载一次，但后插入的实例会被丢弃，不会形成长期重复占用。
pub(super) fn get_or_create(
    key: EngineKey,
    create: impl FnOnce() -> Result<SharedSherpaEngine>,
) -> Result<(Arc<SharedSherpaEngine>, bool)> {
    {
        let mut cache = lock_registry();
        cache.retain(|_, engine| engine.strong_count() > 0);
        if let Some(engine) = cache.get(&key).and_then(Weak::upgrade) {
            return Ok((engine, true));
        }
    }

    let engine = Arc::new(create()?);

    let mut cache = lock_registry();
    cache.retain(|_, engine| engine.strong_count() > 0);
    if let Some(existing) = cache.get(&key).and_then(Weak::upgrade) {
        return Ok((existing, true));
    }
    cache.insert(key, Arc::downgrade(&engine));
    Ok((engine, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_tracks_engine_config_only() {
        let dir = std::env::temp_dir();
        let default = EngineKey::new(&dir, "matcha", SherpaTuning::default());

        // 非 ZipVoice 模型不读取 tuning，归一化后应命中同一份引擎。
        let other_tuning = EngineKey::new(
            &dir,
            "matcha",
            SherpaTuning {
                num_steps: 9,
                guidance_scale: 4.0,
            },
        );
        assert_eq!(default, other_tuning);

        // 模型类型不同，不能共享。
        assert_ne!(
            default,
            EngineKey::new(&dir, "vits", SherpaTuning::default())
        );

        // ZipVoice 的采样参数在创建时固定，必须参与缓存键。
        let zipvoice = EngineKey::new(&dir, "zipvoice", SherpaTuning::default());
        assert_ne!(
            zipvoice,
            EngineKey::new(
                &dir,
                "zipvoice",
                SherpaTuning {
                    num_steps: 5,
                    guidance_scale: 1.0,
                },
            )
        );
        assert_ne!(
            zipvoice,
            EngineKey::new(
                &dir,
                "zipvoice",
                SherpaTuning {
                    num_steps: 4,
                    guidance_scale: 2.0,
                },
            )
        );
    }
}
