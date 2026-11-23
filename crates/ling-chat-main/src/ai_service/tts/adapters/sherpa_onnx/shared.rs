//! Sherpa-ONNX 共享引擎缓存。
//!
//! 同一个模型目录、模型类型、执行后端和采样参数只加载一份 `OfflineTts`；
//! 不同角色的 sid、语速、参考音频仍由各自的 `SherpaOnnxAdapter` 持有。
//!
//! 正常使用跟着适配器走：最后一个适配器释放后引擎自动销毁。另外保留最近一份
//! 引擎的强引用一段时间，读档、切角色、试听这些会先清空角色再重建的路径因此
//! 不必重新加载模型——模型加载是首次合成的大头，推理本身只占几十毫秒。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use anyhow::Result;
use sherpa_onnx::OfflineTts;

use super::SherpaTuning;

/// 最近使用的引擎保持强引用的时长；只保活一份，避免多模型同时驻留内存。
const RECENT_TTL: Duration = Duration::from_secs(10 * 60);

/// 一个可跨角色复用的推理引擎。
pub(super) struct SharedSherpaEngine {
    pub(super) tts: OfflineTts,
    pub(super) sample_rate: i32,
    /// 串行化同一引擎上的合成，避免多个角色同时调用原生 TTS。
    pub(super) generate_lock: tokio::sync::Mutex<()>,
    /// 已经跑通过一次合成（预热或真实合成）。同一引擎只预热一次。
    warmed: AtomicBool,
}

impl SharedSherpaEngine {
    pub(super) fn new(tts: OfflineTts) -> Self {
        let sample_rate = tts.sample_rate();
        Self {
            tts,
            sample_rate,
            generate_lock: tokio::sync::Mutex::new(()),
            warmed: AtomicBool::new(false),
        }
    }

    /// 抢占预热权；已预热或已有预热在跑时返回 false。
    pub(super) fn claim_warmup(&self) -> bool {
        !self.warmed.swap(true, Ordering::AcqRel)
    }

    /// 合成成功后引擎已经热了，后续预热可以直接跳过。
    pub(super) fn mark_warmed(&self) {
        self.warmed.store(true, Ordering::Release);
    }

    pub(super) fn reset_warmup(&self) {
        self.warmed.store(false, Ordering::Release);
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
    /// 推理线程数在创建引擎时固定；改线程数需要新引擎。
    num_threads: i32,
}

impl EngineKey {
    pub(super) fn new(model_dir: &Path, model_type: &str, tuning: SherpaTuning) -> Self {
        let model_dir =
            std::fs::canonicalize(model_dir).unwrap_or_else(|_| model_dir.to_path_buf());
        let num_threads = tuning.num_threads;
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
            num_threads,
        }
    }
}

/// 引擎缓存：`live` 跟踪仍被适配器持有的引擎，`recent` 额外保活最近一份。
struct Registry {
    live: HashMap<EngineKey, Weak<SharedSherpaEngine>>,
    recent: Option<RecentEngine>,
}

struct RecentEngine {
    key: EngineKey,
    engine: Arc<SharedSherpaEngine>,
    last_used: Instant,
}

impl Registry {
    fn new() -> Self {
        Self {
            live: HashMap::new(),
            recent: None,
        }
    }

    /// 清掉已经没人持有的弱引用；最近引擎由强引用保活，不会被误删。
    fn prune(&mut self) {
        self.live.retain(|_, engine| engine.strong_count() > 0);
    }

    fn touch(&mut self, key: &EngineKey, engine: &Arc<SharedSherpaEngine>) {
        self.recent = Some(RecentEngine {
            key: key.clone(),
            engine: Arc::clone(engine),
            last_used: Instant::now(),
        });
    }

    fn lookup(&mut self, key: &EngineKey) -> Option<Arc<SharedSherpaEngine>> {
        self.prune();
        if let Some(engine) = self.live.get(key).and_then(Weak::upgrade) {
            self.touch(key, &engine);
            return Some(engine);
        }
        let recent = self.recent.as_ref()?;
        if &recent.key != key {
            return None;
        }
        if recent.last_used.elapsed() >= RECENT_TTL {
            // 超过保活时长就放掉，别让旧模型一直占着内存。
            self.recent = None;
            return None;
        }
        let engine = Arc::clone(&recent.engine);
        self.touch(key, &engine);
        Some(engine)
    }
}

fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::new()))
}

/// 进程级推理闸门：同一时刻只让一份 Sherpa 合成真正跑在 CPU 上。
///
/// 每个引擎已经有自己的 `generate_lock`，但低端机/多角色场景下可能同时存在多个
/// 模型（不同角色用不同模型，或试听与聊天并发）。多份推理一起抢核会把音频播放
/// 线程挤出时间片，表现为字内卡顿、爆音。合成本身是串行消费的，全局排队不会
/// 增加端到端延迟，却能保证播放始终有 CPU 可用。
pub(super) fn inference_gate() -> &'static tokio::sync::Semaphore {
    static GATE: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Semaphore::new(1))
}

fn lock_registry() -> std::sync::MutexGuard<'static, Registry> {
    registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 每次真实合成后调用：让保活时长跟着最后一次使用走，长时间聊天后读档也不重载。
pub(super) fn note_use(engine: &Arc<SharedSherpaEngine>) {
    let mut cache = lock_registry();
    if let Some(recent) = cache.recent.as_mut()
        && Arc::ptr_eq(&recent.engine, engine)
    {
        recent.last_used = Instant::now();
    }
}

/// 到点放掉保活引用；期间引擎又被使用过就按最新时间续期。
///
/// 还有适配器持有时，保活引用不额外占内存，先留着保证读档/切角色能命中；
/// 适配器全部释放后才开始计算保活时长。
fn schedule_release(engine: &Arc<SharedSherpaEngine>) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let engine = Arc::downgrade(engine);
    handle.spawn(async move {
        loop {
            let Some(engine) = engine.upgrade() else {
                return;
            };
            let wait = {
                let mut cache = lock_registry();
                let Some(recent) = cache.recent.as_ref() else {
                    return;
                };
                if !Arc::ptr_eq(&recent.engine, &engine) {
                    return;
                }
                let held = cache
                    .live
                    .get(&recent.key)
                    .is_some_and(|weak| weak.strong_count() > 1);
                if held {
                    Duration::from_secs(60)
                } else {
                    let remaining = RECENT_TTL.saturating_sub(recent.last_used.elapsed());
                    if remaining.is_zero() {
                        cache.recent = None;
                        return;
                    }
                    remaining
                }
            };
            tokio::time::sleep(wait).await;
        }
    });
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
    if let Some(engine) = lock_registry().lookup(&key) {
        schedule_release(&engine);
        return Ok((engine, true));
    }

    let engine = Arc::new(create()?);

    let mut cache = lock_registry();
    // 加载期间可能有同 key 的引擎先进缓存，复用它，别重复占内存。
    if let Some(existing) = cache.lookup(&key) {
        drop(cache);
        schedule_release(&existing);
        return Ok((existing, true));
    }
    cache.live.insert(key.clone(), Arc::downgrade(&engine));
    cache.touch(&key, &engine);
    drop(cache);
    schedule_release(&engine);
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
                ..Default::default()
            },
        );
        assert_eq!(default, other_tuning);

        // 线程数在创建引擎时固定，必须参与缓存键。
        assert_ne!(
            default,
            EngineKey::new(&dir, "matcha", SherpaTuning::default().with_num_threads(2))
        );

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
                    ..Default::default()
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
                    ..Default::default()
                },
            )
        );
    }
}
