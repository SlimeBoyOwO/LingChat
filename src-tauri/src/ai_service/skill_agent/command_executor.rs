//! Shell 命令执行、用户审批、超时与输出上限。

use crate::ai_service::skill_agent::events::SkillAgentEvent;
#[cfg(windows)]
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
#[cfg(windows)]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex as StdMutex};
#[cfg(windows)]
use std::time::Instant;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{oneshot, Mutex, Notify};

pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
pub const MAX_COMMAND_TIMEOUT: Duration = Duration::from_secs(300);
pub const DEFAULT_BACKGROUND_COMMAND_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub const MAX_BACKGROUND_COMMAND_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const MAX_COMMAND_OUTPUT_BYTES: usize = 1024 * 1024;
const OUTPUT_DRAIN_GRACE: Duration = Duration::from_millis(400);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
static PROCESS_ELEVATED: OnceLock<bool> = OnceLock::new();

/// 当前 LingChat 进程是否已经持有管理员令牌。进程生命周期内权限不会变化，故缓存结果。
#[cfg(windows)]
pub fn is_current_process_elevated() -> bool {
    *PROCESS_ELEVATED.get_or_init(|| match query_current_process_elevation() {
        Ok(elevated) => elevated,
        Err(error) => {
            tracing::warn!("无法读取当前进程的 Windows 令牌权限: {error}");
            false
        }
    })
}

#[cfg(windows)]
fn query_current_process_elevation() -> windows::core::Result<bool> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)?;
        let result = (|| {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut returned_size = 0;
            GetTokenInformation(
                token,
                TokenElevation,
                Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned_size,
            )?;
            Ok(elevation.TokenIsElevated != 0)
        })();
        let _ = CloseHandle(token);
        result
    }
}

#[cfg(not(windows))]
pub fn is_current_process_elevated() -> bool {
    false
}

/// `uac=true` 只有在当前进程尚未提权时才需要启动 RunAs 辅助进程。
pub fn needs_elevated_launcher(uac_requested: bool, process_elevated: bool) -> bool {
    uac_requested && !process_elevated
}

/// 通过 Windows 正常 RunAs 流程启动管理员重启辅助进程。
///
/// 辅助进程先写入就绪标记，再等待当前 LingChat 完全退出，最后启动继承管理员
/// 令牌的新实例。这样可以避免两个 WebView/Tauri 实例短暂重叠时，新实例因共享
/// 资源仍被旧实例占用而立即退出。用户仍需在系统 UAC 对话框中明确同意，本函数
/// 不绕过系统安全边界。
#[cfg(windows)]
pub fn launch_current_process_as_admin() -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;

    if is_current_process_elevated() {
        return Ok(());
    }
    let executable = std::env::current_exe()?;
    let working_directory = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("当前程序路径没有父目录"))?;
    let ready_file =
        std::env::temp_dir().join(format!("lingchat_admin_restart_{}.ready", new_request_id()));
    let helper_script = build_admin_restart_helper_script(
        std::process::id(),
        &executable,
        working_directory,
        &ready_file,
    );
    let encoded_helper = encode_powershell_command(&helper_script);
    let script = format!(
        "$ErrorActionPreference = 'Stop'; \
         $p = Start-Process -FilePath powershell.exe \
           -ArgumentList @('-NoProfile','-NonInteractive','-WindowStyle','Hidden','-ExecutionPolicy','Bypass','-EncodedCommand','{encoded_helper}') \
           -WindowStyle Hidden -Verb RunAs -PassThru; \
         Write-Output $p.Id",
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| anyhow::anyhow!("无法启动管理员实例: {error}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&ready_file);
        let message = decode_console_output(&output.stderr);
        anyhow::bail!(
            "管理员重启未完成（可能在 UAC 窗口选择了“否”）{}",
            if message.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", message.trim())
            }
        );
    }

    // Start-Process 返回只代表 ShellExecute 接受了请求。等到提权后的辅助进程
    // 真正运行并写入标记后，调用方才可以安全关闭当前实例。
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready_file.is_file() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    if !ready_file.is_file() {
        let _ = std::fs::remove_file(&ready_file);
        anyhow::bail!("管理员启动器未能就绪，已保留当前 LingChat 窗口")
    }
    Ok(())
}

#[cfg(windows)]
fn build_admin_restart_helper_script(
    parent_pid: u32,
    executable: &Path,
    working_directory: &Path,
    ready_file: &Path,
) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'\r\n\
         $parentPid = {parent_pid}\r\n\
         $readyFile = '{}'\r\n\
         Set-Content -LiteralPath $readyFile -Value $PID -NoNewline\r\n\
         try {{\r\n\
           $deadline = [DateTime]::UtcNow.AddSeconds(30)\r\n\
           while ((Get-Process -Id $parentPid -ErrorAction SilentlyContinue) -and [DateTime]::UtcNow -lt $deadline) {{\r\n\
             Start-Sleep -Milliseconds 100\r\n\
           }}\r\n\
           if (Get-Process -Id $parentPid -ErrorAction SilentlyContinue) {{ exit 124 }}\r\n\
           Start-Process -FilePath '{}' -WorkingDirectory '{}' | Out-Null\r\n\
         }} finally {{\r\n\
           Remove-Item -LiteralPath $readyFile -Force -ErrorAction SilentlyContinue\r\n\
         }}",
        powershell_single_quoted_path(ready_file),
        powershell_single_quoted_path(executable),
        powershell_single_quoted_path(working_directory),
    )
}

#[cfg(windows)]
fn encode_powershell_command(script: &str) -> String {
    let utf16_le = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    base64::engine::general_purpose::STANDARD.encode(utf16_le)
}

#[cfg(not(windows))]
pub fn launch_current_process_as_admin() -> anyhow::Result<()> {
    anyhow::bail!("管理员重启仅支持 Windows")
}

/// 等待用户审批决定的命令。
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

/// Windows 命令输出通常是 GBK/CP936 而非 UTF-8。
fn decode_console_output(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    encoding_rs::GBK.decode(bytes).0.into_owned()
}

impl CommandOutput {
    pub fn to_prompt_string(&self) -> String {
        let mut out = format!("退出码: {}\n", self.exit_code);
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
    let n = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("req-{ts}-{n}")
}

/// 以默认超时运行命令。
pub async fn run_shell_command(
    sandbox_dir: &Path,
    command: &str,
    cwd: &str,
) -> anyhow::Result<CommandOutput> {
    run_shell_command_with_timeout(sandbox_dir, command, cwd, DEFAULT_COMMAND_TIMEOUT).await
}

/// 以受限的时间和输出运行 shell 命令。
///
/// shell 进程是生命周期边界。成功分离的后代进程可以继续运行，
/// 但仅凭继承 stdout/stderr 句柄无法让这个 future 保持存活。
/// 超时或输出失控时，进程树会被尽力终止。
pub async fn run_shell_command_with_timeout(
    sandbox_dir: &Path,
    command: &str,
    cwd: &str,
    timeout: Duration,
) -> anyhow::Result<CommandOutput> {
    run_shell_command_with_limits(
        sandbox_dir,
        command,
        cwd,
        clamp_command_timeout(timeout),
        MAX_COMMAND_OUTPUT_BYTES,
    )
    .await
}

/// 以更大且明确受限的后台超时，运行分离的对话命令。
///
/// 特意与 [`run_shell_command_with_timeout`] 分开，
/// 让前台调用方保留原有的五分钟上限。
pub async fn run_shell_command_in_background_with_timeout(
    sandbox_dir: &Path,
    command: &str,
    cwd: &str,
    timeout: Duration,
) -> anyhow::Result<CommandOutput> {
    run_shell_command_with_limits(
        sandbox_dir,
        command,
        cwd,
        clamp_timeout(timeout, MAX_BACKGROUND_COMMAND_TIMEOUT),
        MAX_COMMAND_OUTPUT_BYTES,
    )
    .await
}

async fn run_shell_command_with_limits(
    sandbox_dir: &Path,
    command: &str,
    cwd: &str,
    timeout: Duration,
    output_limit: usize,
) -> anyhow::Result<CommandOutput> {
    if command.trim().is_empty() {
        anyhow::bail!("命令不能为空");
    }
    let cwd_path = resolve_working_directory(sandbox_dir, cwd)?;

    #[cfg(windows)]
    let mut process = {
        let mut process = tokio::process::Command::new("cmd");
        // raw_arg 能保持 cmd.exe 的嵌套引号原样不被破坏。
        process
            .arg("/D")
            .arg("/C")
            .raw_arg(std::ffi::OsStr::new(command))
            .creation_flags(CREATE_NO_WINDOW);
        process
    };
    #[cfg(not(windows))]
    let mut process = {
        use std::os::unix::process::CommandExt;

        let mut process = tokio::process::Command::new("sh");
        process.arg("-c").arg(command).process_group(0);
        process
    };

    process
        .current_dir(cwd_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = process
        .spawn()
        .map_err(|e| anyhow::anyhow!("无法执行命令: {e}"))?;
    let mut cancellation_guard = ProcessTreeCancellationGuard::new(child.id());
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("无法捕获命令 stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("无法捕获命令 stderr"))?;

    let stdout_buf = Arc::new(StdMutex::new(Vec::new()));
    let stderr_buf = Arc::new(StdMutex::new(Vec::new()));
    let total = Arc::new(AtomicUsize::new(0));
    let exceeded = Arc::new(AtomicBool::new(false));
    let output_exceeded = Arc::new(Notify::new());
    let mut stdout_task = tokio::spawn(capture_pipe(
        stdout,
        Arc::clone(&stdout_buf),
        Arc::clone(&total),
        Arc::clone(&exceeded),
        Arc::clone(&output_exceeded),
        output_limit,
    ));
    let mut stderr_task = tokio::spawn(capture_pipe(
        stderr,
        Arc::clone(&stderr_buf),
        Arc::clone(&total),
        Arc::clone(&exceeded),
        Arc::clone(&output_exceeded),
        output_limit,
    ));

    enum Completion {
        Exited(std::process::ExitStatus),
        TimedOut,
        OutputLimit,
    }

    let completion = tokio::select! {
        biased;
        _ = output_exceeded.notified() => Completion::OutputLimit,
        _ = tokio::time::sleep(timeout) => Completion::TimedOut,
        status = child.wait() => Completion::Exited(
            status.map_err(|e| anyhow::anyhow!("等待命令结束失败: {e}"))?
        ),
    };

    if !matches!(completion, Completion::Exited(_)) {
        terminate_process_tree(&mut child).await;
    }
    cancellation_guard.disarm();
    finish_capture(&mut stdout_task, &mut stderr_task).await;

    let partial = CommandOutput {
        stdout: decode_console_output(&clone_capture(&stdout_buf)),
        stderr: decode_console_output(&clone_capture(&stderr_buf)),
        exit_code: -1,
    };

    match completion {
        Completion::TimedOut => anyhow::bail!(
            "命令执行超时（{} 秒），已终止进程树。\n{}",
            timeout.as_secs_f32(),
            partial.to_prompt_string()
        ),
        Completion::OutputLimit => anyhow::bail!(
            "命令输出超过 {output_limit} 字节，已终止进程树；请将大量输出重定向到文件。\n{}",
            partial.to_prompt_string()
        ),
        Completion::Exited(status) => Ok(CommandOutput {
            stdout: partial.stdout,
            stderr: partial.stderr,
            exit_code: status.code().unwrap_or(-1),
        }),
    }
}

fn clamp_command_timeout(timeout: Duration) -> Duration {
    clamp_timeout(timeout, MAX_COMMAND_TIMEOUT)
}

fn clamp_timeout(timeout: Duration, maximum: Duration) -> Duration {
    timeout.max(Duration::from_secs(1)).min(maximum)
}

fn resolve_working_directory(sandbox_dir: &Path, cwd: &str) -> anyhow::Result<PathBuf> {
    let requested = cwd.trim();
    let path = if requested.is_empty() {
        sandbox_dir.to_path_buf()
    } else {
        let path = PathBuf::from(requested);
        if path.is_absolute() {
            path
        } else {
            sandbox_dir.join(path)
        }
    };
    if !path.is_dir() {
        anyhow::bail!("工作目录不存在或不是目录: {}", path.display());
    }
    Ok(path)
}

async fn capture_pipe<R: AsyncRead + Unpin>(
    mut pipe: R,
    buffer: Arc<StdMutex<Vec<u8>>>,
    total: Arc<AtomicUsize>,
    exceeded: Arc<AtomicBool>,
    notify: Arc<Notify>,
    limit: usize,
) {
    let mut chunk = [0u8; 8192];
    loop {
        let read = match pipe.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(read) => read,
        };
        let previous = total.fetch_add(read, Ordering::Relaxed);
        let keep = limit.saturating_sub(previous).min(read);
        if keep > 0 {
            if let Ok(mut captured) = buffer.lock() {
                captured.extend_from_slice(&chunk[..keep]);
            }
        }
        if previous.saturating_add(read) > limit && !exceeded.swap(true, Ordering::AcqRel) {
            notify.notify_one();
        }
    }
}

async fn finish_capture(
    stdout_task: &mut tokio::task::JoinHandle<()>,
    stderr_task: &mut tokio::task::JoinHandle<()>,
) {
    if tokio::time::timeout(OUTPUT_DRAIN_GRACE, async {
        let _ = (&mut *stdout_task).await;
        let _ = (&mut *stderr_task).await;
    })
    .await
    .is_err()
    {
        stdout_task.abort();
        stderr_task.abort();
    }
}

fn clone_capture(buffer: &StdMutex<Vec<u8>>) -> Vec<u8> {
    buffer.lock().map(|bytes| bytes.clone()).unwrap_or_default()
}

async fn terminate_process_tree(child: &mut tokio::process::Child) {
    #[cfg(windows)]
    if let Some(pid) = child.id() {
        let mut taskkill = tokio::process::Command::new("taskkill");
        taskkill
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .kill_on_drop(true);
        let _ = tokio::time::timeout(Duration::from_secs(5), taskkill.status()).await;
    }

    #[cfg(not(windows))]
    if let Some(pid) = child.id() {
        let process_group = format!("-{pid}");
        let mut kill = tokio::process::Command::new("kill");
        kill.args(["-KILL", "--", &process_group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let _ = tokio::time::timeout(Duration::from_secs(5), kill.status()).await;
    }

    let _ = child.kill().await;
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
}

/// 异步取消会直接 drop 命令 future，没有机会再等待清理。
/// 要在 `Child::kill_on_drop` 移除 shell 进程之前运行操作系统的进程树终止命令。
/// 这里特意只在取消时阻塞：分离的 taskkill 会与子进程 drop 竞争，
/// 可能在 taskkill 检查之前就丢失父子进程关系。
struct ProcessTreeCancellationGuard {
    pid: Option<u32>,
}

impl ProcessTreeCancellationGuard {
    fn new(pid: Option<u32>) -> Self {
        Self { pid }
    }

    fn disarm(&mut self) {
        self.pid = None;
    }
}

impl Drop for ProcessTreeCancellationGuard {
    fn drop(&mut self) {
        let Some(pid) = self.pid else { return };

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;

            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }

        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", "--", &format!("-{pid}")])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

#[cfg(windows)]
pub async fn run_shell_command_elevated_with_timeout(
    sandbox_dir: &Path,
    command: &str,
    cwd: &str,
    timeout: Duration,
) -> anyhow::Result<CommandOutput> {
    let timeout = clamp_command_timeout(timeout);
    let cwd_path = resolve_working_directory(sandbox_dir, cwd)?;
    let temp = ElevatedTempFiles::new();
    let bat_content = format!(
        "@echo off\r\nchcp 65001 >nul\r\ncd /d \"{}\"\r\n{} > \"{}\" 2> \"{}\"\r\necho %ERRORLEVEL% > \"{}\"\r\n",
        cwd_path.display(),
        command,
        temp.stdout.display(),
        temp.stderr.display(),
        temp.exit_code.display()
    );
    std::fs::write(&temp.script, bat_content)?;
    std::fs::write(&temp.guard, b"running")?;

    // 提权后的 cmd 进程树由提权 watchdog 负责终止。中等完整性的启动器
    // 无法可靠地 taskkill 高完整性子进程，因此超时/取消时只移除哨兵文件，
    // 让这个提权进程在同等完整性级别下执行清理。
    let watchdog = format!(
        "$ErrorActionPreference = 'Stop'\r\n\
         $guard = '{}'\r\n\
         if (-not (Test-Path -LiteralPath $guard)) {{ exit 124 }}\r\n\
         $p = Start-Process -FilePath cmd.exe -ArgumentList '/D','/C','\"{}\"' -PassThru\r\n\
         Set-Content -LiteralPath '{}' -Value $p.Id -NoNewline\r\n\
         while (-not $p.HasExited) {{\r\n\
           if (-not (Test-Path -LiteralPath $guard)) {{\r\n\
             & taskkill.exe /PID $p.Id /T /F | Out-Null\r\n\
             exit 124\r\n\
           }}\r\n\
           Start-Sleep -Milliseconds 200\r\n\
           $p.Refresh()\r\n\
         }}\r\n\
         exit $p.ExitCode\r\n",
        powershell_single_quoted_path(&temp.guard),
        temp.script.display(),
        powershell_single_quoted_path(&temp.pid),
    );
    std::fs::write(&temp.watchdog, watchdog)?;

    // Process::WaitForExit 只等待提权 watchdog 本身。
    // Start-Process -Wait 还会等待后代进程，会复现句柄继承导致的挂起。
    let ps = format!(
        "$p = Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','\"{}\"' -Verb RunAs -PassThru; $p.WaitForExit(); exit $p.ExitCode",
        temp.watchdog.display()
    );
    let mut process = tokio::process::Command::new("powershell");
    process
        .args(["-NoProfile", "-Command", &ps])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .kill_on_drop(true);

    let output = match tokio::time::timeout(timeout, process.output()).await {
        Ok(result) => result.map_err(|e| anyhow::anyhow!("无法启动提权进程: {e}"))?,
        Err(_) => {
            cancel_elevated_process(&temp.guard, &temp.pid).await;
            anyhow::bail!(
                "提权命令在 {} 秒内未结束；已发送取消信号并尝试终止已启动的提权进程。",
                timeout.as_secs()
            )
        }
    };

    let stdout = read_limited_output(&temp.stdout)?;
    let stderr = read_limited_output(&temp.stderr)?;
    let exit_code = std::fs::read_to_string(&temp.exit_code)
        .ok()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .unwrap_or(-1);
    if !output.status.success() && stdout.is_empty() && exit_code == -1 {
        anyhow::bail!(
            "提权执行失败（用户可能在 UAC 窗口选择了“否”）: {}",
            decode_console_output(&output.stderr)
        );
    }
    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code,
    })
}

#[cfg(windows)]
struct ElevatedTempFiles {
    script: PathBuf,
    watchdog: PathBuf,
    guard: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
    exit_code: PathBuf,
    pid: PathBuf,
}

#[cfg(windows)]
impl ElevatedTempFiles {
    fn new() -> Self {
        let prefix = std::env::temp_dir().join(format!("lingchat_uac_{}", new_request_id()));
        Self {
            script: prefix.with_extension("bat"),
            watchdog: prefix.with_extension("ps1"),
            guard: prefix.with_extension("guard"),
            stdout: prefix.with_extension("out"),
            stderr: prefix.with_extension("err"),
            exit_code: prefix.with_extension("code"),
            pid: prefix.with_extension("pid"),
        }
    }
}

#[cfg(windows)]
impl Drop for ElevatedTempFiles {
    fn drop(&mut self) {
        for path in [
            &self.script,
            &self.watchdog,
            &self.guard,
            &self.stdout,
            &self.stderr,
            &self.exit_code,
            &self.pid,
        ] {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(windows)]
fn powershell_single_quoted_path(path: &Path) -> String {
    path.to_string_lossy().replace("'", "''")
}

/// 删除哨兵后，提权 watchdog 会在同等权限下终止命令树；taskkill 是 watchdog
/// 未能正常运行时的最后兜底。若用户仍停留在 UAC 窗口，PID 文件不存在，之后即使
/// 接受 UAC，watchdog 也会先发现哨兵缺失并拒绝启动命令。
#[cfg(windows)]
async fn cancel_elevated_process(guard_path: &Path, pid_path: &Path) {
    let _ = std::fs::remove_file(guard_path);
    let mut pid = None;
    for _ in 0..10 {
        pid = std::fs::read_to_string(pid_path)
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok());
        if pid.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let Some(pid) = pid else { return };

    // 先给提权 watchdog 一个轮询间隔，让它优先执行特权清理。
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut taskkill = tokio::process::Command::new("taskkill");
    taskkill
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .kill_on_drop(true);
    match tokio::time::timeout(Duration::from_secs(5), taskkill.status()).await {
        Ok(Ok(status)) if status.success() => {}
        // watchdog 可能已经终止了进程；因此非零的兜底结果仅作诊断，
        // 不视为第二次面向用户的失败。
        Ok(Ok(status)) => tracing::debug!(pid, ?status, "提权 watchdog 已接管或兜底终止失败"),
        Ok(Err(error)) => tracing::warn!(pid, %error, "无法启动 taskkill 清理提权进程"),
        Err(_) => tracing::warn!(pid, "终止超时的提权进程失败：taskkill 超时"),
    }
}

#[cfg(windows)]
fn read_limited_output(path: &Path) -> anyhow::Result<String> {
    use std::io::Read;

    let Ok(mut file) = std::fs::File::open(path) else {
        return Ok(String::new());
    };
    let mut bytes = Vec::new();
    file.by_ref()
        .take((MAX_COMMAND_OUTPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_COMMAND_OUTPUT_BYTES {
        anyhow::bail!(
            "提权命令输出超过 {} 字节；请将大量输出重定向到文件",
            MAX_COMMAND_OUTPUT_BYTES
        );
    }
    Ok(decode_console_output(&bytes))
}

#[cfg(not(windows))]
pub async fn run_shell_command_elevated_with_timeout(
    _sandbox_dir: &Path,
    _command: &str,
    _cwd: &str,
    _timeout: Duration,
) -> anyhow::Result<CommandOutput> {
    anyhow::bail!("UAC 提权仅支持 Windows 平台")
}

/// 剧本代理的命令执行，沿用其现有的审批通道。
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

        if let Err(error) = channel.send(SkillAgentEvent::PendingApproval {
            request_id: request_id.clone(),
            tool: "execute_command".into(),
            args,
        }) {
            approvals.lock().await.remove(&request_id);
            anyhow::bail!("无法发送命令审批请求: {error}");
        }

        let decision = tokio::time::timeout(Duration::from_secs(120), rx).await;
        approvals.lock().await.remove(&request_id);
        match decision {
            Ok(Ok(true)) => tracing::debug!("[skill_agent] approval granted: {}", request_id),
            Ok(Ok(false)) => anyhow::bail!("命令已被用户拒绝"),
            Ok(Err(_)) => anyhow::bail!("审批通道已关闭，命令未执行"),
            Err(_) => anyhow::bail!("命令审批超时（120 秒），已自动拒绝"),
        }
    }

    run_shell_command(sandbox_dir, command, cwd).await
}
