//! 日志系统装配。
//!
//! 负责 tracing 订阅器的初始化（stdout fmt + 日志桥 + 文件 fmt + 可热重载过滤器），
//! 以及 `log.genai_debug` 开关的热重载。本模块原先位于 crate 根（`lib.rs`）。

use chrono::Local;
use tauri::Listener;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::{config, data_dir, utils};

/// 本地时间格式化器，用于日志输出的时间戳。
struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%H:%M:%S"))
    }
}

/// 构建日志过滤器。
///
/// `genai_debug` 为 true 时把 `genai` crate 的日志级别从 error 提到 debug，
/// 用于查看 LLM 请求/响应细节（默认关闭，由 `log.genai_debug` 设置控制）。
fn build_log_filter(genai_debug: bool) -> tracing_subscriber::EnvFilter {
    let base = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,ling_chat_lib=info"))
        .add_directive("sqlx=warn".parse().unwrap());
    if genai_debug {
        base.add_directive("genai=debug".parse().unwrap())
    } else {
        base.add_directive("genai=error".parse().unwrap())
    }
}

/// genai 调试日志开关的热重载句柄。
///
/// `reload::Handle<EnvFilter, S>` 的 `S` 是订阅器**整条 layer 栈**的嵌套类型
/// （`Layered<fmt, Layered<LogBridgeLayer, Layered<fmt, Registry>>>`），
/// 无法在不写死全部 layer 与 `LocalTimer` / `LogFileWriter` 的前提下命名；
/// 且任何 layer 的增删都会让它静默失配。这里把句柄擦除成一个闭包，
/// 既避免类型泄漏，也让 `Send + Sync + 'static` 与具体订阅器类型解耦。
pub struct LogFilterHandle(
    Box<dyn Fn(bool) -> Result<(), tracing_subscriber::reload::Error> + Send + Sync>,
);

impl LogFilterHandle {
    /// 应用 genai 调试日志开关（`log.genai_debug`）。
    ///
    /// 语义等价于直接调用 `reload::Handle::reload(build_log_filter(enabled))`。
    pub fn set_genai_debug(&self, enabled: bool) -> Result<(), tracing_subscriber::reload::Error> {
        (self.0)(enabled)
    }
}

/// 安装全局日志系统并返回 genai 调试开关的热重载句柄。
///
/// 只应在 `run()` 中调用一次。注意 `reload::Handle` 内部是 `Weak` 引用，
/// 句柄不持有 layer——它依赖本次 `.init()` 安装的全局订阅器保活，
/// 订阅器一旦被 drop，`set_genai_debug` 会返回 `SubscriberGone`。
pub fn init_tracing() -> LogFilterHandle {
    // 配置日志过滤器（genai 调试日志由 log.genai_debug 设置在 setup 阶段动态控制）。
    // reload::Layer 包装的 EnvFilter 作为全局过滤层，避免在多个 fmt layer 上 clone 的限制。
    let (filter, reload_handle) = tracing_subscriber::reload::Layer::new(build_log_filter(false));

    // 初始化日志系统
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_timer(LocalTimer))
        .with(crate::utils::log_bridge::LogBridgeLayer)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(crate::utils::file_logger::LogFileWriter)
                .with_timer(LocalTimer)
                .with_ansi(false),
        )
        .with(filter)
        .init();

    LogFilterHandle(Box::new(move |genai_debug| {
        reload_handle.reload(build_log_filter(genai_debug))
    }))
}

/// 读取设置并应用文件日志 / LLM 请求体日志 / genai 调试开关。
///
/// 原先内联在 `lib.rs` 的 setup 闭包中（「初始化文件日志」块）。
pub fn apply_log_settings(app: &tauri::App<tauri::Wry>, log_filter: &LogFilterHandle) {
    // 初始化文件日志（从设置读取开关和保留天数）
    let store = config::settings_store(app.handle()).ok();
    let log_enable = store
        .as_ref()
        .and_then(|s| s.get(config::keys::LOG_ENABLE))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let retention_days = store
        .as_ref()
        .and_then(|s| s.get(config::keys::LOG_RETENTION_DAYS))
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .unwrap_or(10);

    let data_dir = data_dir::get_data_dir();
    utils::file_logger::init_logging(data_dir, log_enable);
    utils::file_logger::cleanup_old_logs(retention_days);

    // 初始化 LLM 请求体日志（默认关闭）
    let llm_request_log_enable = store
        .as_ref()
        .and_then(|s| s.get(config::keys::LOG_LLM_REQUEST_BODY))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    utils::llm_request_logger::init(data_dir, llm_request_log_enable);

    // 应用 genai 调试日志开关（log.genai_debug，默认关闭）
    let genai_debug = store
        .as_ref()
        .and_then(|s| s.get(config::keys::LOG_GENAI_DEBUG))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if let Err(e) = log_filter.set_genai_debug(genai_debug) {
        tracing::warn!("应用日志过滤器失败: {e}");
    }
}

/// 监听 settings store 变更，热重载 genai 调试日志（无需重启）。
///
/// 按值接管 `log_filter`——句柄随监听闭包存活至进程结束。原先内联在 `lib.rs`
/// 的 setup 闭包中（「热重载 genai 调试日志」块）。
pub fn watch_genai_debug(app: &tauri::App<tauri::Wry>, log_filter: LogFilterHandle) {
    let app_handle = app.handle().clone();
    app_handle.listen("store://change", move |event| {
        #[derive(serde::Deserialize)]
        struct StoreChangePayload {
            key: String,
            value: Option<serde_json::Value>,
        }
        let Ok(payload) = serde_json::from_str::<StoreChangePayload>(event.payload()) else {
            return;
        };
        if payload.key != config::keys::LOG_GENAI_DEBUG {
            return;
        }
        let genai_debug = matches!(payload.value, Some(serde_json::Value::Bool(true)));
        if let Err(e) = log_filter.set_genai_debug(genai_debug) {
            tracing::warn!("热重载 genai 调试日志失败: {e}");
        } else {
            tracing::info!(
                "genai 调试日志已{}",
                if genai_debug { "开启" } else { "关闭" }
            );
        }
    });
}
