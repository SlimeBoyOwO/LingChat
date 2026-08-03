//! Shell 命令执行与用户审批。

use crate::ai_service::skill_agent::events::SkillAgentEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{oneshot, Mutex};

/// 一个等待用户决策的审批请求。
pub struct ApprovalRequest {
    pub tx: oneshot::Sender<bool>,
}

pub type ApprovalMap = Arc<Mutex<HashMap<String, ApprovalRequest>>>;

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// 子进程输出解码：中文 Windows 上命令输出通常是 GBK/CP936，非 UTF-8 时回退 GBK。
fn decode_console_output(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    encoding_rs::GBK.decode(bytes).0.into_owned()
}

impl CommandOutput {
    pub fn to_prompt_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("退出码: {}\n", self.exit_code));
        if !self.stdout.trim().is_empty() {
            out.push_str(&format!("stdout:\n{}\n", self.stdout));
        }
        if !self.stderr.trim().is_empty() {
            out.push_str(&format!("stderr:\n{}\n", self.stderr));
        }
        if self.stdout.trim().is_empty() && self.stderr.trim().is_empty() {
            out.push_str("（无输出）\n");
        }
        out
    }
}

pub fn new_request_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let n = REQUEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("req-{}-{}", ts, n)
}

/// 运行 shell 命令。Windows 用 `cmd /C`，POSIX 用 `sh -c`。
/// 需要审批时（auto_approve=false）发 PendingApproval 事件并等待用户决定（120s 超时自动拒绝）。
#[allow(clippy::too_many_arguments)]
pub async fn execute_command(
    channel: &tauri::ipc::Channel<SkillAgentEvent>,
    approvals: &ApprovalMap,
    auto_approve: bool,
    sandbox_dir: &Path,
    command: &str,
    cwd: &str,
) -> anyhow::Result<CommandOutput> {
    tracing::debug!(
        "[skill_agent] execute_command auto_approve={} cmd={}",
        auto_approve,
        command
    );

    if !auto_approve {
        let request_id = new_request_id();
        let args = serde_json::json!({ "command": command, "cwd": cwd });
        let (tx, rx) = oneshot::channel::<bool>();
        approvals
            .lock()
            .await
            .insert(request_id.clone(), ApprovalRequest { tx });

        let _ = channel.send(SkillAgentEvent::PendingApproval {
            request_id: request_id.clone(),
            tool: "execute_command".into(),
            args,
        });

        let decision = tokio::time::timeout(Duration::from_secs(120), rx).await;
        approvals.lock().await.remove(&request_id);

        match decision {
            Ok(Ok(true)) => tracing::debug!("[skill_agent] approval granted: {}", request_id),
            Ok(Ok(false)) => anyhow::bail!("命令已被用户拒绝"),
            Ok(Err(_)) => anyhow::bail!("审批通道已关闭，命令未执行"),
            Err(_) => anyhow::bail!("命令审批超时（120 秒），已自动拒绝"),
        }
    }

    let cwd_path = if cwd.trim().is_empty() {
        sandbox_dir.to_path_buf()
    } else {
        std::path::PathBuf::from(cwd.trim())
    };

    let output = if cfg!(windows) {
        // raw_arg 原样传命令，避免 std 自动加引号被 cmd.exe 自己的一套引号规则弄坏内层引号
        tokio::process::Command::new("cmd")
            .arg("/C")
            .raw_arg(std::ffi::OsStr::new(command))
            .current_dir(cwd_path)
            .output()
            .await
    } else {
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(cwd_path)
            .output()
            .await
    };

    let output = output.map_err(|e| anyhow::anyhow!("无法执行命令: {}", e))?;
    Ok(CommandOutput {
        stdout: decode_console_output(&output.stdout),
        stderr: decode_console_output(&output.stderr),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

#[cfg(test)]
mod tests {
    /// `cmd /C python script.py "pink dark neon"` 必须原样保留带引号参数。
    /// 没有 raw_arg 时 std 会对整串自动加引号，cmd.exe 的引号规则会剥掉内层引号。
    #[test]
    fn cmd_preserves_quoted_args_with_raw_arg() {
        use std::os::windows::process::CommandExt;

        let script = std::env::temp_dir().join("lingchat_quote_test.py");
        std::fs::write(
            &script,
            "import sys\nprint(repr(sys.argv[1:]))\n",
        )
        .unwrap();

        let cmd = format!(
            "python {} \"pink dark neon\" --domain color -n 3",
            script.to_string_lossy()
        );
        let out = std::process::Command::new("cmd")
            .arg("/C")
            .raw_arg(std::ffi::OsStr::new(&cmd))
            .output()
            .expect("run cmd");

        let _ = std::fs::remove_file(&script);

        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(
            stdout.contains("['pink dark neon', '--domain', 'color', '-n', '3']"),
            "quoted arg was mangled.\nstdout: {}\nstderr: {}",
            stdout,
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
