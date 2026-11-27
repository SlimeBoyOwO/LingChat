//! 插件自带的本地脚本运行时：前台执行、常驻服务与停止。
//!
//! 这是通用能力：宿主只认“插件目录里的一个 Node 脚本”，不解释脚本内容。
//! 前台执行用 Node 权限模型限制在插件目录与本次工作目录内；常驻服务只跑插件
//! 自带的脚本（可信代码），用于本地页面。

use std::collections::{HashMap, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::guard::{current_plugin, current_script_run_allow};
use super::{err_json, kwargs_map, no_host, py_to_value, runtime, value_to_pyobject};
use crate::PluginManager;

const MAX_INPUT_FILES: usize = 64;
const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_LOG_LINES: usize = 500;
const MAX_LOG_CHARS: usize = 1024 * 1024;
const DEFAULT_TIMEOUT_MS: u64 = 60_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const SERVE_START_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ARGS: usize = 64;
const MAX_ARG_CHARS: usize = 8_000;

/// 一条常驻脚本服务。
struct ScriptRun {
    plugin_id: String,
    workdir: PathBuf,
    child: Child,
}

static RUNS: OnceLock<Mutex<HashMap<String, ScriptRun>>> = OnceLock::new();

fn runs() -> &'static Mutex<HashMap<String, ScriptRun>> {
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 插件目录（脚本必须在这里面）。
fn plugin_root(app: &AppHandle, plugin_id: &str) -> Result<PathBuf, String> {
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    let dir = manager
        .plugin_dir(plugin_id)
        .ok_or_else(|| format!("插件目录不存在：{plugin_id}"))?;
    dir.canonicalize()
        .map_err(|error| format!("插件目录无效：{error}"))
}

fn safe_relative(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw.trim());
    if raw.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("只允许插件目录内的相对路径：{raw}"));
    }
    Ok(path.to_path_buf())
}

fn resolve_script(root: &Path, script: &str) -> Result<PathBuf, String> {
    let relative = safe_relative(script)?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("找不到脚本 {script}：{error}"))?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(format!("脚本不在插件目录内：{script}"));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "mjs" | "js" | "cjs") {
        return Err("脚本运行时当前只支持 .mjs / .js / .cjs".to_string());
    }
    Ok(path)
}

fn new_workdir(plugin_id: &str) -> Result<PathBuf, String> {
    let dir = ling_chat_main::api::data_dir()
        .join("plugin_runs")
        .join(plugin_id)
        .join(uuid::Uuid::new_v4().simple().to_string());
    std::fs::create_dir_all(&dir).map_err(|error| format!("创建工作目录失败：{error}"))?;
    Ok(dir)
}

fn stage_files(workdir: &Path, files: &Value) -> Result<(), String> {
    let Some(files) = files.as_object() else {
        if files.is_null() {
            return Ok(());
        }
        return Err("files 必须是对象".to_string());
    };
    if files.len() > MAX_INPUT_FILES {
        return Err(format!("输入文件过多（上限 {MAX_INPUT_FILES} 个）"));
    }

    let mut total = 0usize;
    for (name, value) in files {
        let relative = safe_relative(name)?;
        let bytes = if let Some(text) = value.as_str() {
            text.as_bytes().to_vec()
        } else if let Some(encoded) = value.get("base64").and_then(Value::as_str) {
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|error| format!("{name} 的 base64 无效：{error}"))?
        } else {
            return Err(format!("{name} 必须是字符串或 {{\"base64\": \"...\"}}"));
        };
        total += bytes.len();
        if total > MAX_INPUT_BYTES {
            return Err(format!("输入文件总大小超过 {MAX_INPUT_BYTES} 字节"));
        }
        let path = workdir.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建输入目录失败：{error}"))?;
        }
        std::fs::write(&path, bytes).map_err(|error| format!("写入输入文件失败：{error}"))?;
    }
    Ok(())
}

fn read_args(
    kwargs: &HashMap<String, PyObjectRef>,
    vm: &VirtualMachine,
) -> Result<Vec<String>, String> {
    let Some(value) = kwargs.get("args") else {
        return Ok(Vec::new());
    };
    let value = py_to_value(vm, value);
    if value.is_null() {
        return Ok(Vec::new());
    }
    let items = value
        .as_array()
        .ok_or_else(|| "args 必须是字符串数组".to_string())?;
    if items.len() > MAX_ARGS {
        return Err(format!("参数过多（上限 {MAX_ARGS} 个）"));
    }
    items
        .iter()
        .map(|item| {
            let text = item
                .as_str()
                .ok_or_else(|| "args 里只能放字符串".to_string())?;
            if text.chars().count() > MAX_ARG_CHARS {
                return Err("单个参数过长".to_string());
            }
            Ok(text.to_string())
        })
        .collect()
}

fn read_timeout(kwargs: &HashMap<String, PyObjectRef>, vm: &VirtualMachine) -> u64 {
    kwargs
        .get("timeout_ms")
        .and_then(|value| py_to_value(vm, value).as_u64())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1_000, MAX_TIMEOUT_MS)
}

fn truncate_text(mut text: String, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text;
    }
    text = text.chars().take(limit).collect();
    text.push_str("\n……（输出过长，已截断）");
    text
}

fn collect_files(root: &Path) -> (serde_json::Map<String, Value>, bool) {
    let mut out = serde_json::Map::new();
    let mut total = 0usize;
    let mut truncated = false;

    fn walk(
        dir: &Path,
        root: &Path,
        out: &mut serde_json::Map<String, Value>,
        total: &mut usize,
        truncated: &mut bool,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out, total, truncated);
                continue;
            }
            let Ok(metadata) = path.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let size = metadata.len() as usize;
            if *total + size > MAX_OUTPUT_BYTES {
                *truncated = true;
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            *total += bytes.len();
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(
                relative,
                json!({
                    "base64": base64::engine::general_purpose::STANDARD.encode(&bytes),
                    "size": bytes.len(),
                }),
            );
        }
    }

    walk(root, root, &mut out, &mut total, &mut truncated);
    (out, truncated)
}

fn node_command() -> Command {
    let mut command = Command::new("node");
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Node 不认识 Windows 的 verbatim 前缀（`\\?\`），传参前还原成普通路径。
fn node_path(path: &Path) -> String {
    let text = path.display().to_string();
    #[cfg(windows)]
    {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    text
}

async fn run_impl(
    root: PathBuf,
    plugin_id: String,
    script: String,
    args: Vec<String>,
    files: Value,
    timeout_ms: u64,
) -> Value {
    let script_path = match resolve_script(&root, &script) {
        Ok(path) => path,
        Err(error) => return json!({ "ok": false, "error": error }),
    };
    let workdir = match new_workdir(&plugin_id) {
        Ok(dir) => dir,
        Err(error) => return json!({ "ok": false, "error": error }),
    };
    if let Err(error) = stage_files(&workdir, &files) {
        return json!({ "ok": false, "error": error, "workdir": workdir.display().to_string() });
    }

    // Node 只把脚本路径之前的参数当选项，权限参数必须排在脚本前面。
    let mut command = node_command();
    command
        .arg("--permission")
        .arg(format!("--allow-fs-read={}", node_path(&root)))
        .arg(format!("--allow-fs-read={}", node_path(&workdir)))
        .arg(format!("--allow-fs-write={}", node_path(&workdir)))
        .arg(node_path(&script_path))
        .args(&args)
        .current_dir(&workdir)
        .env_clear();
    #[cfg(windows)]
    {
        for key in ["SystemRoot", "TEMP", "TMP"] {
            if let Ok(value) = std::env::var(key) {
                command.env(key, value);
            }
        }
    }

    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return json!({
                "ok": false,
                "error": format!("启动 Node 失败（需要 Node 20+ 且在 PATH 中）：{error}"),
                "workdir": workdir.display().to_string(),
            });
        },
    };

    let output =
        match tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
            .await
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                return json!({
                    "ok": false,
                    "error": format!("脚本执行失败：{error}"),
                    "workdir": workdir.display().to_string(),
                });
            },
            Err(_) => {
                return json!({
                    "ok": false,
                    "timed_out": true,
                    "error": format!("脚本超过 {timeout_ms} 毫秒没有结束"),
                    "workdir": workdir.display().to_string(),
                });
            },
        };

    let stdout = truncate_text(
        String::from_utf8_lossy(&output.stdout).into_owned(),
        MAX_LOG_CHARS,
    );
    let stderr = truncate_text(
        String::from_utf8_lossy(&output.stderr).into_owned(),
        MAX_LOG_CHARS,
    );
    if !output.status.success() {
        let code = output.status.code();
        let excerpt = stderr.chars().take(400).collect::<String>();
        tracing::warn!(
            "插件脚本执行失败 plugin={plugin_id} script={script} code={code:?} stderr={excerpt}"
        );
    }
    let (files, files_truncated) = collect_files(&workdir);
    json!({
        "ok": output.status.success(),
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
        "files": files,
        "files_truncated": files_truncated,
        "timed_out": false,
        "workdir": workdir.display().to_string(),
    })
}

pub(super) fn script_run(
    script: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_script_run_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明脚本执行能力（需在 manifest.toml 顶层写 script_run = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };
    let kwargs = kwargs_map(kwargs);
    let args = match read_args(&kwargs, vm) {
        Ok(args) => args,
        Err(error) => return Ok(err_json(vm, &error)),
    };
    let files = kwargs
        .get("files")
        .map(|value| py_to_value(vm, value))
        .unwrap_or(Value::Null);
    let timeout_ms = read_timeout(&kwargs, vm);
    // 插件目录在进入 tokio runtime 之前解析：manager 用的是 blocking_lock，
    // 放进 block_on 里会撞上“不能在 runtime 内阻塞”的限制。
    let root = match plugin_root(&app, &plugin_id) {
        Ok(root) => root,
        Err(error) => return Ok(err_json(vm, &error)),
    };

    let result = runtime().block_on(run_impl(root, plugin_id, script, args, files, timeout_ms));
    Ok(value_to_pyobject(vm, &result))
}

fn extract_url(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|word| {
        let word = word.trim_end_matches([')', ',', '.', ';', '，', '。']);
        (word.starts_with("http://") || word.starts_with("https://")).then(|| word.to_string())
    })
}

async fn drain<R>(
    reader: R,
    logs: Arc<StdMutex<VecDeque<String>>>,
    urls: tokio::sync::mpsc::UnboundedSender<String>,
) where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(url) = extract_url(&line) {
            let _ = urls.send(url);
        }
        if let Ok(mut logs) = logs.lock() {
            logs.push_back(line);
            while logs.len() > MAX_LOG_LINES {
                logs.pop_front();
            }
        }
    }
}

async fn serve_impl(
    root: PathBuf,
    plugin_id: String,
    script: String,
    args: Vec<String>,
    files: Value,
) -> Value {
    let script_path = match resolve_script(&root, &script) {
        Ok(path) => path,
        Err(error) => return json!({ "ok": false, "error": error }),
    };
    let workdir = match new_workdir(&plugin_id) {
        Ok(dir) => dir,
        Err(error) => return json!({ "ok": false, "error": error }),
    };
    if let Err(error) = stage_files(&workdir, &files) {
        return json!({ "ok": false, "error": error, "workdir": workdir.display().to_string() });
    }

    // 常驻服务跑插件自带的页面/查看器，属于可信代码；模型生成的数据/代码只走
    // script_run 的权限沙箱，两者不要混用。
    let mut command = node_command();
    command
        .arg(node_path(&script_path))
        .args(&args)
        .current_dir(&workdir);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return json!({
                "ok": false,
                "error": format!("启动 Node 失败（需要 Node 20+ 且在 PATH 中）：{error}"),
                "workdir": workdir.display().to_string(),
            });
        },
    };

    let logs = Arc::new(StdMutex::new(VecDeque::new()));
    let (url_tx, mut url_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(drain(stdout, logs.clone(), url_tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(drain(stderr, logs.clone(), url_tx));
    }

    let url = match tokio::time::timeout(SERVE_START_TIMEOUT, url_rx.recv()).await {
        Ok(Some(url)) => url,
        Ok(None) => {
            let _ = child.kill().await;
            return json!({
                "ok": false,
                "error": "脚本退出前没有输出可访问的 URL",
                "logs": logs_snapshot(&logs),
                "workdir": workdir.display().to_string(),
            });
        },
        Err(_) => {
            let _ = child.kill().await;
            return json!({
                "ok": false,
                "error": "脚本启动超时，没有输出可访问的 URL",
                "logs": logs_snapshot(&logs),
                "workdir": workdir.display().to_string(),
            });
        },
    };

    let run_id = format!("run-{}", uuid::Uuid::new_v4().simple());
    runs().lock().await.insert(
        run_id.clone(),
        ScriptRun {
            plugin_id,
            workdir: workdir.clone(),
            child,
        },
    );
    json!({
        "ok": true,
        "run_id": run_id,
        "url": url,
        "logs": logs_snapshot(&logs),
        "workdir": workdir.display().to_string(),
    })
}

fn logs_snapshot(logs: &StdMutex<VecDeque<String>>) -> Vec<String> {
    logs.lock()
        .map(|logs| logs.iter().cloned().collect())
        .unwrap_or_default()
}

pub(super) fn script_serve(
    script: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_script_run_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明脚本执行能力（需在 manifest.toml 顶层写 script_run = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };
    let kwargs = kwargs_map(kwargs);
    let args = match read_args(&kwargs, vm) {
        Ok(args) => args,
        Err(error) => return Ok(err_json(vm, &error)),
    };
    let files = kwargs
        .get("files")
        .map(|value| py_to_value(vm, value))
        .unwrap_or(Value::Null);
    let root = match plugin_root(&app, &plugin_id) {
        Ok(root) => root,
        Err(error) => return Ok(err_json(vm, &error)),
    };

    let result = runtime().block_on(serve_impl(root, plugin_id, script, args, files));
    Ok(value_to_pyobject(vm, &result))
}

pub(super) fn script_stop(run_id: String, vm: &VirtualMachine) -> PyResult<PyObjectRef> {
    if !current_script_run_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明脚本执行能力（需在 manifest.toml 顶层写 script_run = true）",
        ));
    }
    let plugin_id = current_plugin();
    let result = runtime().block_on(async {
        match plugin_id.as_deref() {
            Some(plugin_id) => stop_run(&run_id, Some(plugin_id)).await,
            None => stop_run(&run_id, None).await,
        }
    });
    Ok(value_to_pyobject(
        vm,
        &match result {
            Ok(()) => json!({ "ok": true }),
            Err(error) => json!({ "ok": false, "error": error }),
        },
    ))
}

/// 停掉一条常驻服务。`plugin_id` 给出时校验归属，防止别的插件停别人的服务。
pub(crate) async fn stop_run(run_id: &str, plugin_id: Option<&str>) -> Result<(), String> {
    let mut map = runs().lock().await;
    let Some(run) = map.get(run_id) else {
        return Err(format!("没有找到常驻脚本 {run_id}"));
    };
    if let Some(plugin_id) = plugin_id
        && run.plugin_id != plugin_id
    {
        return Err("这条常驻脚本不属于当前插件".to_string());
    }
    let mut run = map.remove(run_id).expect("刚检查过");
    drop(map);

    let _ = run.child.kill().await;
    let _ = run.child.wait().await;
    let _ = tokio::fs::remove_dir_all(&run.workdir).await;
    Ok(())
}

/// 查一条常驻服务的归属插件。
pub(crate) async fn run_owner(run_id: &str) -> Option<String> {
    runs()
        .lock()
        .await
        .get(run_id)
        .map(|run| run.plugin_id.clone())
}

/// 插件禁用或删除时回收它的全部常驻服务。
pub(crate) async fn stop_plugin(plugin_id: &str) {
    let ids: Vec<String> = {
        let map = runs().lock().await;
        map.iter()
            .filter(|(_, run)| run.plugin_id == plugin_id)
            .map(|(id, _)| id.clone())
            .collect()
    };
    for id in ids {
        let _ = stop_run(&id, Some(plugin_id)).await;
    }
}
