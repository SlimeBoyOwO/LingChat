//! Native desktop windows used by the horror-script `console_window` event.
//!
//! Windows never launches PowerShell/pwsh here. Error and warning beats use a
//! native TaskDialog, notes launch Notepad directly, and console beats launch
//! cmd.exe directly (blue or blood-red). Every object is generation-owned,
//! bounded by the event validator, and closed when its lifetime or script run
//! ends.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

const MAX_PENDING_REQUESTS: usize = 64;

#[derive(Debug, Clone)]
pub struct PopupSequence {
    pub title: String,
    pub lines: Vec<String>,
    pub count: usize,
    pub interval: f64,
    pub lifetime: f64,
    pub style: String,
}

struct PendingPopup {
    request: PopupSequence,
    generation: u64,
}

static RUN_GENERATION: AtomicU64 = AtomicU64::new(1);
static RUN_ACTIVE: AtomicBool = AtomicBool::new(false);
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static PENDING: LazyLock<Mutex<HashMap<u64, PendingPopup>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn generation_is_current(generation: u64) -> bool {
    RUN_ACTIVE.load(Ordering::SeqCst) && RUN_GENERATION.load(Ordering::SeqCst) == generation
}

pub fn begin_run() {
    close_all();
    RUN_ACTIVE.store(true, Ordering::SeqCst);
}

pub fn queue_pending(request: PopupSequence) -> Result<u64, String> {
    if !request.interval.is_finite() || !request.lifetime.is_finite() {
        return Err("系统窗口时序必须是有限数值".to_string());
    }
    let mut pending = PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !RUN_ACTIVE.load(Ordering::SeqCst) {
        return Err("当前没有可接收系统窗口事件的剧本运行".to_string());
    }
    if pending.len() >= MAX_PENDING_REQUESTS {
        return Err("系统窗口待处理票据已达到安全上限".to_string());
    }
    let generation = RUN_GENERATION.load(Ordering::SeqCst);
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    pending.insert(
        request_id,
        PendingPopup {
            request,
            generation,
        },
    );
    Ok(request_id)
}

pub fn discard_pending(request_id: u64) {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&request_id);
}

fn take_pending(request_id: u64) -> Result<PendingPopup, String> {
    let pending = PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&request_id)
        .ok_or_else(|| "系统窗口票据不存在、已消费或已取消".to_string())?;
    if !generation_is_current(pending.generation) {
        return Err("系统窗口票据已随剧本运行失效".to_string());
    }
    Ok(pending)
}

pub fn show_pending(request_id: u64) -> Result<(), String> {
    let pending = take_pending(request_id)?;
    spawn_sequence(pending.request, pending.generation);
    Ok(())
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::{generation_is_current, PopupSequence};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Child, Command};
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{LazyLock, Mutex};
    use std::time::Duration;
    use uuid::Uuid;
    use windows::core::{BOOL, HRESULT, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, TRUE, WPARAM};
    use windows::Win32::UI::Controls::{
        TaskDialogIndirect, TASKDIALOGCONFIG, TDCBF_OK_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
        TDF_CALLBACK_TIMER, TDF_SIZE_TO_CONTENT, TDM_CLICK_BUTTON, TDN_CREATED, TDN_DESTROYED,
        TDN_TIMER, TD_ERROR_ICON, TD_WARNING_ICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
        PostMessageW, SetWindowPos, HWND_TOPMOST, SC_CLOSE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        WM_CLOSE, WM_SYSCOMMAND,
    };

    const ID_OK: usize = 1;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    static ACTIVE_SLOTS: AtomicUsize = AtomicUsize::new(0);
    static DIALOGS: LazyLock<Mutex<HashMap<u64, isize>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    static PROCESSES: LazyLock<Mutex<HashMap<u64, ProcessPopup>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    struct ProcessPopup {
        child: Child,
        temp_files: Vec<PathBuf>,
        window_marker: Option<String>,
        kill_process: bool,
    }

    struct DialogContext {
        id: u64,
        generation: u64,
        lifetime_ms: u64,
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    fn reserve_slot() -> bool {
        ACTIVE_SLOTS
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |active| {
                (active < 4).then_some(active + 1)
            })
            .is_ok()
    }

    fn release_slot() {
        let _ = ACTIVE_SLOTS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |active| {
            (active > 0).then_some(active - 1)
        });
    }

    unsafe extern "system" fn task_dialog_callback(
        hwnd: HWND,
        notification: windows::Win32::UI::Controls::TASKDIALOG_NOTIFICATIONS,
        wparam: WPARAM,
        _lparam: LPARAM,
        callback_data: isize,
    ) -> HRESULT {
        // SAFETY: TaskDialogIndirect invokes this callback synchronously while
        // the boxed context remains alive in `spawn_task_dialog`.
        let context = unsafe { &*(callback_data as *const DialogContext) };
        if notification == TDN_CREATED {
            if !generation_is_current(context.generation) {
                let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
                return HRESULT(0);
            }
            DIALOGS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(context.id, hwnd.0 as isize);
            if !generation_is_current(context.generation) {
                DIALOGS
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&context.id);
                let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
                return HRESULT(0);
            }
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
                )
            };
        } else if notification == TDN_TIMER && wparam.0 as u64 >= context.lifetime_ms {
            let _ = unsafe {
                PostMessageW(
                    Some(hwnd),
                    TDM_CLICK_BUTTON.0 as u32,
                    WPARAM(ID_OK),
                    LPARAM(0),
                )
            };
        } else if notification == TDN_DESTROYED {
            DIALOGS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&context.id);
        }
        HRESULT(0)
    }

    fn spawn_task_dialog(
        title: String,
        text: String,
        lifetime: f64,
        warning: bool,
        generation: u64,
    ) {
        let id = next_id();
        std::thread::spawn(move || {
            if !generation_is_current(generation) {
                release_slot();
                return;
            }
            let title_w = wide(&title);
            let text_w = wide(&text);
            let context = Box::new(DialogContext {
                id,
                generation,
                lifetime_ms: (lifetime * 1000.0).round() as u64,
            });
            let context_ptr = Box::into_raw(context);
            let mut config = TASKDIALOGCONFIG::default();
            config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
            config.dwFlags =
                TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER | TDF_SIZE_TO_CONTENT;
            config.dwCommonButtons = TDCBF_OK_BUTTON;
            config.pszWindowTitle = PCWSTR(title_w.as_ptr());
            config.pszMainInstruction = PCWSTR(title_w.as_ptr());
            config.pszContent = PCWSTR(text_w.as_ptr());
            config.Anonymous1.pszMainIcon = if warning {
                TD_WARNING_ICON
            } else {
                TD_ERROR_ICON
            };
            config.pfCallback = Some(task_dialog_callback);
            config.lpCallbackData = context_ptr as isize;

            let result = unsafe { TaskDialogIndirect(&config, None, None, None) };
            // SAFETY: TaskDialogIndirect has returned and can no longer call the
            // callback, so the context has no remaining borrowers.
            drop(unsafe { Box::from_raw(context_ptr) });
            DIALOGS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&id);
            release_slot();
            if let Err(error) = result {
                tracing::warn!("[ScriptPopup] 原生系统弹窗失败: {error}");
            }
        });
    }

    fn cmd_literal(value: &str) -> String {
        value
            .chars()
            .filter(|character| {
                !character.is_control()
                    && !matches!(
                        character,
                        '&' | '|' | '<' | '>' | '%' | '^' | '"' | '`' | '!' | '(' | ')'
                    )
            })
            .collect::<String>()
    }

    fn cmd_script(title: &str, text_filename: &str, blood_red: bool) -> String {
        let color = if blood_red { "4F" } else { "1F" };
        [
            "@echo off".to_string(),
            "chcp 65001 >nul".to_string(),
            format!("title {}", cmd_literal(title)),
            format!("color {color}"),
            "cls".to_string(),
            format!("type \"%~dp0{text_filename}\""),
            ":lingchat_wait".to_string(),
            "ping -n 2 127.0.0.1 >nul".to_string(),
            "goto lingchat_wait".to_string(),
        ]
        .join("\r\n")
    }

    struct WindowSearch {
        marker: String,
        handles: Vec<isize>,
    }

    unsafe extern "system" fn collect_matching_windows(hwnd: HWND, data: LPARAM) -> BOOL {
        // SAFETY: `close_windows_by_marker` keeps this stack value alive for the
        // synchronous EnumWindows call.
        let search = unsafe { &mut *(data.0 as *mut WindowSearch) };
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if length > 0 {
            let mut buffer = vec![0u16; length as usize + 1];
            let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
            if copied > 0 {
                let title = String::from_utf16_lossy(&buffer[..copied as usize]);
                if title.to_lowercase().contains(&search.marker) {
                    search.handles.push(hwnd.0 as isize);
                }
            }
        }
        TRUE
    }

    struct ProcessWindowSearch {
        process_id: u32,
        handles: Vec<isize>,
    }

    unsafe extern "system" fn collect_process_windows(hwnd: HWND, data: LPARAM) -> BOOL {
        // SAFETY: `process_windows` owns this context throughout EnumWindows.
        let search = unsafe { &mut *(data.0 as *mut ProcessWindowSearch) };
        let mut process_id = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        if process_id == search.process_id {
            search.handles.push(hwnd.0 as isize);
        }
        TRUE
    }

    fn process_windows(process_id: u32) -> Vec<isize> {
        let mut search = ProcessWindowSearch {
            process_id,
            handles: Vec::new(),
        };
        let data = LPARAM((&mut search as *mut ProcessWindowSearch) as isize);
        if let Err(error) = unsafe { EnumWindows(Some(collect_process_windows), data) } {
            tracing::warn!("[ScriptPopup] 枚举记事本进程窗口失败: {error}");
            return Vec::new();
        }
        search.handles
    }

    fn matching_windows(marker: &str) -> Vec<isize> {
        let mut search = WindowSearch {
            marker: marker.to_lowercase(),
            handles: Vec::new(),
        };
        let data = LPARAM((&mut search as *mut WindowSearch) as isize);
        if let Err(error) = unsafe { EnumWindows(Some(collect_matching_windows), data) } {
            tracing::warn!("[ScriptPopup] 枚举记事本窗口失败: {error}");
            return Vec::new();
        }
        search.handles
    }

    fn close_windows_by_marker(marker: &str) -> Vec<isize> {
        let handles = matching_windows(marker);
        for raw in &handles {
            let hwnd = HWND(*raw as *mut core::ffi::c_void);
            let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }
        handles
    }

    fn tracked_window_title(raw: isize) -> Option<String> {
        let hwnd = HWND(raw as *mut core::ffi::c_void);
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return None;
        }
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        let mut buffer = vec![0u16; length.max(0) as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
        Some(String::from_utf16_lossy(&buffer[..copied.max(0) as usize]))
    }

    fn register_process(
        child: Child,
        temp_files: Vec<PathBuf>,
        window_marker: Option<String>,
        kill_process: bool,
        lifetime: f64,
        generation: u64,
    ) {
        let id = next_id();
        PROCESSES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                id,
                ProcessPopup {
                    child,
                    temp_files,
                    window_marker,
                    kill_process,
                },
            );
        // Cleanup may have advanced the generation between process creation and
        // registry insertion. Recheck after insertion so that race is closed.
        if !generation_is_current(generation) {
            close_process(id);
            return;
        }
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f64(lifetime));
            close_process(id);
        });
    }

    fn close_process(id: u64) {
        let popup = PROCESSES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&id);
        if let Some(mut popup) = popup {
            let popup_process_id = popup.child.id();
            let mut tracked_notepad_handles = Vec::new();
            let mut notepad_document_open = false;
            if let Some(marker) = popup.window_marker.as_deref() {
                // Notepad can take a moment to create its UUID-titled HWND. Close
                // only that exact document window; never kill/reuse a Notepad process.
                for _ in 0..10 {
                    tracked_notepad_handles = close_windows_by_marker(marker);
                    if tracked_notepad_handles.is_empty() {
                        // Windows 11 may title the new window simply “记事本”.
                        // The exact child PID is still a safe ownership boundary.
                        tracked_notepad_handles = process_windows(popup_process_id);
                        for raw in &tracked_notepad_handles {
                            let hwnd = HWND(*raw as *mut core::ffi::c_void);
                            let _ =
                                unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
                        }
                    }
                    if !tracked_notepad_handles.is_empty() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if !tracked_notepad_handles.is_empty() {
                    std::thread::sleep(Duration::from_millis(160));
                    let marker_lower = marker.to_lowercase();
                    for raw in &tracked_notepad_handles {
                        if let Some(title) = tracked_window_title(*raw) {
                            if title.to_lowercase().contains(&marker_lower) {
                                // The document or its save prompt still owns this HWND.
                                notepad_document_open = true;
                            } else {
                                // Windows 11 Notepad may replace the closed document with
                                // a blank tab; close that same tracked window once more.
                                let hwnd = HWND(*raw as *mut core::ffi::c_void);
                                let _ = unsafe {
                                    PostMessageW(
                                        Some(hwnd),
                                        WM_SYSCOMMAND,
                                        WPARAM(SC_CLOSE as usize),
                                        LPARAM(0),
                                    )
                                };
                            }
                        }
                    }
                    if !notepad_document_open {
                        std::thread::sleep(Duration::from_millis(120));
                    }
                }
            }

            let mut child_running = match popup.child.try_wait() {
                Ok(Some(_)) => false,
                Ok(None) if popup.kill_process => {
                    let _ = popup.child.kill();
                    let _ = popup.child.wait();
                    false
                }
                Ok(None) => true,
                Err(error) => {
                    tracing::warn!("[ScriptPopup] 查询系统窗口进程失败: {error}");
                    false
                }
            };
            if !popup.kill_process
                && !notepad_document_open
                && !tracked_notepad_handles.is_empty()
                && child_running
            {
                // `/newWindow` gave us a dedicated child and its UUID document
                // already closed cleanly. End only that exact blank popup host;
                // never target any pre-existing Notepad process by image name.
                let _ = popup.child.kill();
                let _ = popup.child.wait();
                child_running = false;
            }
            let preserve_note = !popup.kill_process
                && (notepad_document_open || (tracked_notepad_handles.is_empty() && child_running));
            if preserve_note {
                tracing::warn!("[ScriptPopup] 记事本窗口仍在使用，保留其临时文件且不终止进程");
            } else {
                for path in popup.temp_files {
                    if let Err(error) = fs::remove_file(&path) {
                        if error.kind() != std::io::ErrorKind::NotFound {
                            tracing::warn!(
                                "[ScriptPopup] 删除临时系统窗口文件失败 {}: {error}",
                                path.display()
                            );
                        }
                    }
                }
            }
            release_slot();
        }
    }

    fn spawn_cmd(
        title: &str,
        lines: &[String],
        lifetime: f64,
        blood_red: bool,
        generation: u64,
    ) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        // 剧本文字只进入临时纯文本文件，绝不成为 cmd.exe 命令的一部分；
        // .cmd 仅含固定命令、净化后的标题与随机安全文件名。
        let id = Uuid::new_v4();
        let temp_dir = std::env::temp_dir();
        let text_filename = format!("lingchat-console-{id}.txt");
        let script_filename = format!("lingchat-console-{id}.cmd");
        let text_path = temp_dir.join(&text_filename);
        let script_path = temp_dir.join(&script_filename);
        let body = format!("{}\r\n", lines.join("\r\n"));
        fs::write(&text_path, body.as_bytes())
            .map_err(|error| format!("写入 CMD 演出文本失败: {error}"))?;
        if let Err(error) = fs::write(
            &script_path,
            cmd_script(title, &text_filename, blood_red).as_bytes(),
        ) {
            let _ = fs::remove_file(&text_path);
            return Err(format!("写入 CMD 启动脚本失败: {error}"));
        }

        let mut command = Command::new("cmd.exe");
        command
            .arg("/D")
            .arg("/Q")
            .arg("/K")
            .arg(&script_filename)
            .current_dir(&temp_dir)
            .creation_flags(CREATE_NEW_CONSOLE);
        match command.spawn() {
            Ok(child) => {
                register_process(
                    child,
                    vec![script_path, text_path],
                    None,
                    true,
                    lifetime,
                    generation,
                );
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(&script_path);
                let _ = fs::remove_file(&text_path);
                Err(format!("启动真实 CMD 失败: {error}"))
            }
        }
    }

    fn spawn_notepad(
        title: &str,
        lines: &[String],
        lifetime: f64,
        generation: u64,
    ) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let marker = format!("lingchat-note-{}", Uuid::new_v4());
        let path = std::env::temp_dir().join(format!("{marker}.txt"));
        let body = format!("{title}\r\n\r\n{}\r\n", lines.join("\r\n"));
        let mut encoded = vec![0xEF, 0xBB, 0xBF];
        encoded.extend_from_slice(body.as_bytes());
        fs::write(&path, encoded).map_err(|error| format!("写入临时记事本残页失败: {error}"))?;
        let mut command = Command::new("notepad.exe");
        command
            .arg("/newWindow")
            .arg(&path)
            .creation_flags(CREATE_NO_WINDOW);
        match command.spawn() {
            Ok(child) => {
                register_process(child, vec![path], Some(marker), false, lifetime, generation);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(&path);
                Err(format!("启动真实记事本失败: {error}"))
            }
        }
    }

    fn spawn_one(request: &PopupSequence, generation: u64) -> Result<(), String> {
        if !generation_is_current(generation) {
            return Ok(());
        }
        if !reserve_slot() {
            return Err("真实系统窗口全局上限为 4，已拒绝额外窗口".to_string());
        }
        let text = request.lines.join("\n");
        let result = match request.style.as_str() {
            "error" => {
                spawn_task_dialog(
                    request.title.clone(),
                    text,
                    request.lifetime,
                    false,
                    generation,
                );
                Ok(())
            }
            "warning" => {
                spawn_task_dialog(
                    request.title.clone(),
                    text,
                    request.lifetime,
                    true,
                    generation,
                );
                Ok(())
            }
            "notepad" => {
                spawn_notepad(&request.title, &request.lines, request.lifetime, generation)
            }
            "blood_cmd" => spawn_cmd(
                &request.title,
                &request.lines,
                request.lifetime,
                true,
                generation,
            ),
            _ => spawn_cmd(
                &request.title,
                &request.lines,
                request.lifetime,
                false,
                generation,
            ),
        };
        if result.is_err() {
            release_slot();
        }
        result
    }

    pub fn spawn_sequence(request: PopupSequence, generation: u64) {
        tauri::async_runtime::spawn(async move {
            for index in 0..request.count {
                if !generation_is_current(generation) {
                    break;
                }
                if let Err(error) = spawn_one(&request, generation) {
                    tracing::warn!("[ScriptPopup] 系统弹窗演出失败: {error}");
                    break;
                }
                if request.interval > 0.0 && index + 1 < request.count {
                    tokio::time::sleep(Duration::from_secs_f64(request.interval)).await;
                }
            }
        });
    }

    pub fn close_all_native() {
        let dialogs: Vec<isize> = DIALOGS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .drain()
            .map(|(_, hwnd)| hwnd)
            .collect();
        for raw in dialogs {
            let hwnd = HWND(raw as *mut core::ffi::c_void);
            let _ = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
        }

        let process_ids: Vec<u64> = PROCESSES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .keys()
            .copied()
            .collect();
        for id in process_ids {
            close_process(id);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::cmd_script;

        #[test]
        fn blood_cmd_uses_red_background_without_powershell() {
            let command = cmd_script("RUNTIME", "lingchat-console-test.txt", true);
            assert!(command.contains("color 4F"));
            assert!(command.contains("%~dp0lingchat-console-test.txt"));
            assert!(!command.to_ascii_lowercase().contains("powershell"));
            assert!(!command.to_ascii_lowercase().contains("pwsh"));
            assert!(!command.contains("safe story text"));
        }

        #[test]
        fn normal_cmd_uses_blue_background() {
            let command = cmd_script("RUNTIME", "lingchat-console-test.txt", false);
            assert!(command.contains("color 1F"));
        }
    }
}

#[cfg(target_os = "windows")]
fn spawn_sequence(request: PopupSequence, generation: u64) {
    windows_impl::spawn_sequence(request, generation);
}

#[cfg(not(target_os = "windows"))]
fn spawn_sequence(_request: PopupSequence, _generation: u64) {
    tracing::info!("[ScriptPopup] 非 Windows 平台：跳过系统弹窗演出（剧本继续）");
}

#[cfg(target_os = "windows")]
fn close_native() {
    windows_impl::close_all_native();
}

#[cfg(not(target_os = "windows"))]
fn close_native() {}

pub fn close_all() {
    RUN_ACTIVE.store(false, Ordering::SeqCst);
    RUN_GENERATION.fetch_add(1, Ordering::SeqCst);
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    close_native();
}

#[cfg(test)]
mod ticket_tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn request() -> PopupSequence {
        PopupSequence {
            title: "RUNTIME".to_string(),
            lines: vec!["safe text".to_string()],
            count: 1,
            interval: 0.1,
            lifetime: 1.0,
            style: "error".to_string(),
        }
    }

    #[test]
    fn tickets_are_single_use_and_canceled_with_the_run() {
        let _guard = TEST_LOCK.lock().unwrap();
        begin_run();
        let first = queue_pending(request()).unwrap();
        assert_eq!(take_pending(first).unwrap().request.title, "RUNTIME");
        assert!(take_pending(first).is_err());

        let canceled = queue_pending(request()).unwrap();
        close_all();
        assert!(take_pending(canceled).is_err());
        assert!(queue_pending(request()).is_err());
    }
}
