//! 上下文用量/压缩命令：kimi 式上下文窗口管理的前端入口。

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::ai_service::game_system::context_compaction;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageInfo {
    /// 估算的当前上下文用量（tokens）
    pub used_tokens: u32,
    /// 当前对话模型的上下文窗口（tokens）
    pub window_tokens: u32,
    /// 用量百分比（0-100，可超 100）
    pub percent: u32,
    /// 当前存档的台词总数
    pub line_count: usize,
    /// 是否有实测锚点（provider 上报过 usage）
    pub has_measured_anchor: bool,
    /// 压缩摘要是否生效中
    pub compacted: bool,
    /// 摘要覆盖到的台词条数（未生效为 0）
    pub compacted_upto: usize,
    /// 自动压缩开关
    pub auto_compact: bool,
}

#[tauri::command]
pub async fn get_context_usage(app: AppHandle) -> Result<ContextUsageInfo, String> {
    let state = app.state::<AppState>();
    let svc = state.ai_service.lock().await;
    let gs = svc.game_status.lock().await;

    let used = context_compaction::current_usage_tokens(&gs);
    let (compacted, compacted_upto) = match context_compaction::effective_summary(&gs) {
        Some((_, cutoff)) => (true, cutoff),
        None => (false, 0),
    };
    let info = ContextUsageInfo {
        used_tokens: used,
        window_tokens: 0, // 下方填（需要异步之外再读配置，同步函数足够）
        percent: 0,
        line_count: gs.line_list.len(),
        has_measured_anchor: gs.last_prompt_tokens.is_some(),
        compacted,
        compacted_upto,
        auto_compact: crate::config::app_config::AppConfig::load(&app)
            .map(|c| c.auto_compact)
            .unwrap_or(true),
    };
    drop(gs);
    drop(svc);

    let window = context_compaction::current_context_window(&app).await;
    Ok(ContextUsageInfo {
        window_tokens: window,
        percent: ((used as f64 / window as f64) * 100.0).ceil() as u32,
        ..info
    })
}

#[tauri::command]
pub async fn compact_context(app: AppHandle) -> Result<context_compaction::CompactOutcome, String> {
    context_compaction::compact_now(&app, true)
        .await
        .map_err(|e| format!("压缩失败: {e}"))
}
