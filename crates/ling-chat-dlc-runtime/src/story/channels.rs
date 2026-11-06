//! Per-run native interaction state, stored through the host's typed extension slot.
use ling_chat_main::ai_service::game_system::script_engine::events::ScriptChannels;
/// 当前 `force_choice` 的一次性能力票据：只允许对应主窗口在短时间内移动鼠标，
/// 提交时后端还会强制核对 forced 文本，不能只依赖前端 disabled。
#[derive(Clone, Debug)]
pub struct ForceChoiceGuard {
    pub request_id: String,
    pub forced: String,
    pub warp_enabled: bool,
    pub warp_expires_at: std::time::Instant,
}

/// 绑定一次写诗互动的独立提交通道。
pub struct PoemSubmissionChannel {
    pub request_id: String,
    pub tx: tokio::sync::oneshot::Sender<String>,
}

#[derive(Default)]
pub struct NativeChannels {
    pub force_choice_guard: Option<ForceChoiceGuard>,
    pub poem_tx: Option<PoemSubmissionChannel>,
}
pub fn validate_choice(
    channels: &mut ScriptChannels,
    choice: &str,
    request_id: Option<&str>,
) -> Result<(), String> {
    if let Some(guard) = channels
        .extension_mut::<NativeChannels>()
        .force_choice_guard
        .as_ref()
    {
        if request_id != Some(guard.request_id.as_str()) {
            return Err("force_choice 提交票据无效".to_string());
        }
        if choice != guard.forced {
            return Err("force_choice 只能提交 forced 选项".to_string());
        }
    } else if request_id.is_some() {
        return Err("force_choice 已结束或票据已过期".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_choice_requires_the_current_ticket_and_forced_value() {
        let mut channels = ScriptChannels::new();
        channels
            .extension_mut::<NativeChannels>()
            .force_choice_guard = Some(ForceChoiceGuard {
            request_id: "current".into(),
            forced: "continue".into(),
            warp_enabled: false,
            warp_expires_at: std::time::Instant::now(),
        });
        assert!(validate_choice(&mut channels, "continue", Some("stale")).is_err());
        assert!(validate_choice(&mut channels, "escape", Some("current")).is_err());
        assert!(validate_choice(&mut channels, "continue", Some("current")).is_ok());
        channels.extension_state.clear();
        assert!(validate_choice(&mut channels, "continue", Some("current")).is_err());
        assert!(validate_choice(&mut channels, "ordinary choice", None).is_ok());
    }
    #[test]
    fn clearing_extensions_cancels_poem_without_consuming_host_input() {
        let mut channels = ScriptChannels::new();
        let (tx, mut poem_rx) = tokio::sync::oneshot::channel();
        let (input_tx, mut input_rx) = tokio::sync::oneshot::channel();
        channels.input_tx = Some(input_tx);
        channels.extension_mut::<NativeChannels>().poem_tx = Some(PoemSubmissionChannel {
            request_id: "poem".into(),
            tx,
        });
        channels.extension_state.clear();
        assert!(matches!(
            poem_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Closed)
        ));
        channels
            .input_tx
            .take()
            .unwrap()
            .send("ordinary input".into())
            .unwrap();
        assert_eq!(input_rx.try_recv().unwrap(), "ordinary input");
    }
}
