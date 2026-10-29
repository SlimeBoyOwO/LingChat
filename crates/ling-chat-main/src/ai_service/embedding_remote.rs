//! 云端向量化后端与「本地 / 云端」路由。
//!
//! [`Embedder`] 契约留在 notes crate，云端实现必须待在这里：notes 位于依赖链最底层，
//! 不能把 HTTP 栈拖进去。
//!
//! [`RemoteEmbedder`] 用 genai 的 embeddings 端点，对外保持同步阻塞——内部驱动一次
//! 异步请求即可，这样整条检索链路的签名、并发模型与 SQLite 访问方式都不用区分后端。
//! [`EmbeddingSelector`] 实现同一个 [`Embedder`]，负责按用户配置选路、重试与熔断，
//! 下游只看得到一个嵌入器，看不出下面是本地还是云端。

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use genai::Client as GenaiClient;
use genai::ServiceTarget;
use genai::adapter::AdapterKind;
use genai::resolver::{AuthData, Endpoint};
use serde::Serialize;
use tauri::AppHandle;
use tauri::Emitter;

use ling_chat_notes::embedding::{Embedder, l2_normalize};

use super::llm::normalize_base_url;
use super::tools::settings::{EmbeddingSettings, SharedToolSettings};

/// 一次逻辑调用的最大尝试次数；累计仍失败即进入熔断态。
pub const MAX_EMBEDDING_ATTEMPTS: usize = 5;

/// 熔断告警事件（同一熔断周期内只发一次）。
pub const ALERT_EVENT: &str = "embedding:alert";

/// 预热进度事件。
pub const WARMING_EVENT: &str = "embedding:warming";

/// 熔断告警载荷。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingAlert {
    /// 生效后端标识，如 `cloud openai/text-embedding-3-small` 或 `local`。
    pub backend: String,
    /// 失败原因。
    pub reason: String,
}

/// 预热进度载荷。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarmingProgress {
    pub backend: String,
    pub done: usize,
    pub total: usize,
    pub failed: usize,
    /// 结束（完成 / 失败 / 取消）时为 true，前端据此隐藏进度条。
    pub finished: bool,
}

/// 在阻塞线程里驱动一次异步嵌入请求。
///
/// 嵌入接口的契约是「同步阻塞、由调用方放进 spawn_blocking」；blocking 线程上有
/// runtime 上下文，`Handle::block_on` 合法。极端情况下（没有 runtime，例如单元测试）
/// 临时建一个 current-thread runtime，避免 panic。
fn drive<F: std::future::Future>(future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(future),
        Err(_) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("临时 runtime 创建失败");
            runtime.block_on(future)
        },
    }
}

/// 云端嵌入后端。
pub struct RemoteEmbedder {
    client: GenaiClient,
    model: String,
    namespace: String,
    timeout: Duration,
    /// 首次成功响应后固定下来的向量维度。
    dimension: OnceLock<usize>,
}

impl RemoteEmbedder {
    /// 按配置构造客户端。配置未就绪时不应当走到这里。
    pub fn new(cfg: &EmbeddingSettings) -> Result<Self> {
        if cfg.model.trim().is_empty() {
            return Err(anyhow!("云端向量化未配置模型名"));
        }
        let timeout = Duration::from_secs(cfg.timeout_secs.max(1));
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow!("构造嵌入 HTTP 客户端失败: {e}"))?;

        let kind = match cfg.provider.as_str() {
            "gemini" => AdapterKind::Gemini,
            _ => AdapterKind::OpenAI,
        };
        let key = cfg.api_key.clone();
        let mut builder = GenaiClient::builder()
            .with_reqwest(http)
            .with_adapter_kind(kind)
            .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(key.clone()))));
        if !cfg.base_url.is_empty() {
            let base = normalize_base_url(&cfg.base_url);
            builder = builder.with_service_target_resolver_fn(move |mut target: ServiceTarget| {
                target.endpoint = Endpoint::from_owned(base.clone());
                Ok(target)
            });
        }
        let client = builder
            .build()
            .map_err(|e| anyhow!("构造嵌入客户端失败: {e}"))?;

        Ok(Self {
            client,
            model: cfg.model.clone(),
            // 分库目录名：保留可读性，同时避免 `/` 这类路径分隔符（模型名常带斜杠）。
            namespace: format!("{}_{}", cfg.provider, cfg.model.replace(['/', '\\'], "-")),
            timeout,
            dimension: OnceLock::new(),
        })
    }

    /// 面向用户与日志的后端展示名。
    pub fn backend_label(&self) -> String {
        format!("cloud {}", self.namespace)
    }

    fn record_dimension(&self, vector: &[f32]) {
        self.dimension.set(vector.len()).ok();
    }

    async fn call(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let response = tokio::time::timeout(
            self.timeout,
            self.client.embed_batch(&self.model, texts, None),
        )
        .await
        .map_err(|_| anyhow!("云端向量化请求超时（{} 秒）", self.timeout.as_secs()))?
        .map_err(|e| anyhow!("云端向量化请求失败: {e}"))?;
        let vectors = response.into_vectors();
        if vectors.is_empty() {
            return Err(anyhow!("云端向量化返回空结果"));
        }
        Ok(vectors)
    }
}

impl Embedder for RemoteEmbedder {
    fn is_enabled(&self) -> bool {
        true
    }

    /// 维度由首次响应确定；在此之前不能建库，检索走降级。
    fn dimension(&self) -> Option<usize> {
        self.dimension.get().copied()
    }

    fn namespace(&self) -> Option<String> {
        Some(self.namespace.clone())
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut vectors = drive(self.call(vec![text.to_string()]))?;
        let vector = vectors
            .pop()
            .ok_or_else(|| anyhow!("云端向量化返回空结果"))?;
        self.record_dimension(&vector);
        Ok(l2_normalize(vector))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let vectors = drive(self.call(texts.to_vec()))?;
        if vectors.len() != texts.len() {
            return Err(anyhow!(
                "云端向量化结果数量与输入不一致: 期望 {}，实际 {}",
                texts.len(),
                vectors.len()
            ));
        }
        self.record_dimension(&vectors[0]);
        Ok(vectors.into_iter().map(l2_normalize).collect())
    }
}

/// Selector 的一次选路结果。
enum Route {
    /// 用本地 ONNX（未配置云端，或云端构造失败）。
    Local,
    /// 用云端。
    Cloud(Arc<RemoteEmbedder>),
    /// 云端配置在位但已熔断：不再向量化，但命名空间仍属云端，避免半路切回本地空间。
    Tripped(Arc<RemoteEmbedder>),
}

/// 可变的选路状态。
struct Routing {
    /// 上次生效的云端配置标识；变化即重建客户端并清除熔断（即「保存配置即恢复」）。
    identity: Option<(bool, String, String, String, String)>,
    remote: Option<Arc<RemoteEmbedder>>,
    tripped: bool,
    alert_sent: bool,
}

/// 本地 / 云端路由器，同时是一个 [`Embedder`]。
pub struct EmbeddingSelector {
    local: Arc<dyn Embedder>,
    settings: SharedToolSettings,
    app: AppHandle,
    routing: Mutex<Routing>,
}

impl EmbeddingSelector {
    pub fn new(
        local: Arc<dyn Embedder>,
        settings: SharedToolSettings,
        app: AppHandle,
    ) -> Arc<Self> {
        let selector = Arc::new(Self {
            local,
            settings,
            app,
            routing: Mutex::new(Routing {
                identity: None,
                remote: None,
                tripped: false,
                alert_sent: false,
            }),
        });
        // 启动时按当前配置装配一次，日志里能直接看出用的是哪个后端。
        let _ = selector.route();
        selector
    }

    fn route(&self) -> Route {
        let config = self.settings.embedding();
        let identity = config.routing_identity();
        let ready = config.is_ready();
        let mut routing = match self.routing.lock() {
            Ok(guard) => guard,
            Err(_) => return Route::Local,
        };
        if routing.identity.as_ref() != Some(&identity) {
            routing.remote = if ready {
                match RemoteEmbedder::new(&config) {
                    Ok(remote) => {
                        tracing::info!(
                            "[embedding] 云端向量化后端就绪: {}（维度待首次调用确定）",
                            remote.backend_label()
                        );
                        Some(Arc::new(remote))
                    },
                    Err(e) => {
                        tracing::warn!("[embedding] 云端向量化后端构造失败，回退本地: {e}");
                        None
                    },
                }
            } else {
                None
            };
            routing.identity = Some(identity);
            routing.tripped = false;
            routing.alert_sent = false;
        }
        match (routing.remote.clone(), routing.tripped) {
            (Some(remote), true) => Route::Tripped(remote),
            (Some(remote), false) => Route::Cloud(remote),
            (None, _) => Route::Local,
        }
    }

    /// 当前生效后端的展示名（自检回报与告警文案用）。
    pub fn active_backend_label(&self) -> String {
        match self.route() {
            Route::Local => "local".to_string(),
            Route::Cloud(remote) | Route::Tripped(remote) => remote.backend_label(),
        }
    }

    /// 清除熔断态并按当前配置重新选路（保存云端配置时调用）。
    pub fn reset(&self) {
        if let Ok(mut routing) = self.routing.lock() {
            routing.identity = None;
            routing.remote = None;
            routing.tripped = false;
            routing.alert_sent = false;
        }
        let _ = self.route();
    }

    /// 进入熔断态，并在本周期内首次触发时向前端发一次告警。
    fn trip(&self, remote: &RemoteEmbedder, reason: &str) {
        let mut should_alert = false;
        if let Ok(mut routing) = self.routing.lock() {
            routing.tripped = true;
            if !routing.alert_sent {
                routing.alert_sent = true;
                should_alert = true;
            }
        }
        tracing::warn!(
            "[embedding] {} 连续 {MAX_EMBEDDING_ATTEMPTS} 次失败，本轮跳过向量化: {reason}",
            remote.backend_label()
        );
        if should_alert {
            let payload = EmbeddingAlert {
                backend: remote.backend_label(),
                reason: reason.to_string(),
            };
            if let Err(e) = self.app.emit(ALERT_EVENT, &payload) {
                tracing::warn!("[embedding] 熔断告警事件发送失败: {e}");
            }
        }
    }

    /// 走云端：内部重试至多 [`MAX_EMBEDDING_ATTEMPTS`] 次，全部失败即熔断。
    fn cloud_embed_batch(
        &self,
        remote: &Arc<RemoteEmbedder>,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>> {
        let mut last_error = String::new();
        for attempt in 1..=MAX_EMBEDDING_ATTEMPTS {
            match remote.embed_batch(texts) {
                Ok(vectors) => return Ok(vectors),
                Err(e) => {
                    last_error = e.to_string();
                    tracing::warn!(
                        "[embedding] 云端向量化第 {attempt}/{MAX_EMBEDDING_ATTEMPTS} 次失败: {last_error}"
                    );
                    if attempt < MAX_EMBEDDING_ATTEMPTS {
                        std::thread::sleep(Duration::from_millis(400 * attempt as u64));
                    }
                },
            }
        }
        self.trip(remote, &last_error);
        Err(anyhow!("云端向量化失败: {last_error}"))
    }
}

impl Embedder for EmbeddingSelector {
    fn is_enabled(&self) -> bool {
        match self.route() {
            Route::Local => self.local.is_enabled(),
            Route::Cloud(_) => true,
            Route::Tripped(_) => false,
        }
    }

    fn dimension(&self) -> Option<usize> {
        match self.route() {
            Route::Local => self.local.dimension(),
            Route::Cloud(remote) | Route::Tripped(remote) => remote.dimension(),
        }
    }

    fn namespace(&self) -> Option<String> {
        match self.route() {
            Route::Local => self.local.namespace(),
            Route::Cloud(remote) | Route::Tripped(remote) => remote.namespace(),
        }
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match self.route() {
            Route::Local => self.local.embed(text),
            Route::Cloud(remote) => {
                let texts = [text.to_string()];
                self.cloud_embed_batch(&remote, &texts)?
                    .pop()
                    .ok_or_else(|| anyhow!("云端向量化返回空结果"))
            },
            Route::Tripped(_) => Err(anyhow!("云端向量化已熔断，本轮跳过")),
        }
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self.route() {
            Route::Local => self.local.embed_batch(texts),
            Route::Cloud(remote) => self.cloud_embed_batch(&remote, texts),
            Route::Tripped(_) => Err(anyhow!("云端向量化已熔断，本轮跳过")),
        }
    }
}

/// 向前端回报预热进度。
pub fn emit_warming(app: &AppHandle, progress: &WarmingProgress) {
    if let Err(e) = app.emit(WARMING_EVENT, progress) {
        tracing::warn!("[embedding] 预热进度事件发送失败: {e}");
    }
}

/// 一轮预热任务的状态。
struct WarmRun {
    generation: u64,
    cancel: Arc<AtomicBool>,
    handle: Option<tauri::async_runtime::JoinHandle<()>>,
}

/// 预热任务句柄：同一时刻只允许一个在跑。
///
/// 每轮任务有独立的取消标记，避免「取消旧任务」与「新任务已开跑」之间的竞态；
/// 新的一次保存会取消上一个（配置已被覆盖），取消信号在角色之间检查。
#[derive(Default)]
pub struct WarmingTask {
    generation: AtomicU64,
    run: Mutex<Option<WarmRun>>,
}

impl WarmingTask {
    /// 开始一轮任务：中止并标记取消上一轮，返回（本轮代号, 取消标记）。
    pub fn begin(&self) -> (u64, Arc<AtomicBool>) {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        let previous = {
            let mut guard = match self.run.lock() {
                Ok(guard) => guard,
                Err(_) => return (generation, cancel),
            };
            guard.replace(WarmRun {
                generation,
                cancel: cancel.clone(),
                handle: None,
            })
        };
        if let Some(previous) = previous {
            previous.cancel.store(true, Ordering::Relaxed);
            if let Some(handle) = previous.handle {
                handle.abort();
            }
        }
        (generation, cancel)
    }

    /// 登记任务句柄（代号不匹配说明已被新一轮取代）。
    pub fn attach(&self, generation: u64, handle: tauri::async_runtime::JoinHandle<()>) {
        if let Ok(mut guard) = self.run.lock() {
            if let Some(run) = guard.as_mut().filter(|run| run.generation == generation) {
                run.handle = Some(handle);
            }
        }
    }

    /// 任务体收尾：只清理自己这一轮，不动后继任务。
    pub fn finish(&self, generation: u64) {
        if let Ok(mut guard) = self.run.lock() {
            if guard
                .as_ref()
                .is_some_and(|run| run.generation == generation)
            {
                *guard = None;
            }
        }
    }
}
