//! Native desktop windows used by the horror-script `console_window` event.
//!
//! Windows never launches PowerShell/pwsh here. Error and warning beats use a
//! native TaskDialog, notes launch Notepad directly, and console beats launch
//! cmd.exe directly (blue or blood-red). Every object is generation-owned and
//! bounded by the event validator; windows stay open until the player closes
//! them, and any leftovers are torn down when the script run ends.

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
    match take_pending(request_id) {
        Ok(pending) => {
            tracing::info!(
                "[ScriptPopup] 票据 {} 消费成功，拉起系统窗口（style={}）",
                request_id,
                pending.request.style
            );
            spawn_sequence(pending.request, pending.generation);
            Ok(())
        }
        Err(error) => {
            tracing::warn!("[ScriptPopup] 票据 {} 消费失败: {}", request_id, error);
            Err(error)
        }
    }
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
    use windows::core::{s, w, BOOL, HRESULT, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, TRUE, WPARAM};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    use windows::Win32::UI::Controls::{
        TASKDIALOGCONFIG, TDCBF_OK_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION, TDF_SIZE_TO_CONTENT,
        TDN_CREATED, TDN_DESTROYED, TD_ERROR_ICON, TD_WARNING_ICON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
        PostMessageW, SetWindowPos, HWND_TOPMOST, SC_CLOSE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
        WM_CLOSE, WM_SYSCOMMAND,
    };

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
        _wparam: WPARAM,
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
        } else if notification == TDN_DESTROYED {
            DIALOGS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&context.id);
        }
        HRESULT(0)
    }

    /// TaskDialogIndirect 只存在于 comctl32 v6（应用清单声明 Common-Controls 6.0
    /// 才加载）。正式 exe 嵌有清单；cargo test 生成的测试二进制没有清单，静态导入会
    /// 让 loader 直接报 0xc0000139。改为运行时解析：v6 不可用时优雅跳过弹窗，
    /// 测试进程也能正常存活。
    type TaskDialogIndirectFn =
        unsafe extern "system" fn(*const TASKDIALOGCONFIG, *mut i32, *mut i32, *mut i32) -> HRESULT;

    fn resolve_task_dialog_indirect() -> Option<TaskDialogIndirectFn> {
        let module = unsafe { GetModuleHandleW(w!("comctl32.dll")) }.ok()?;
        let proc = unsafe { GetProcAddress(module, s!("TaskDialogIndirect")) }?;
        Some(unsafe { std::mem::transmute::<_, TaskDialogIndirectFn>(proc) })
    }

    fn spawn_task_dialog(
        title: String,
        text: String,
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
            let context = Box::new(DialogContext { id, generation });
            let context_ptr = Box::into_raw(context);
            let mut config = TASKDIALOGCONFIG::default();
            config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
            // 弹窗不自动关闭：玩家自己点确定/关闭；剧本结束由 close_all 统一清理。
            config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT;
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

            let Some(task_dialog_indirect) = resolve_task_dialog_indirect() else {
                tracing::warn!("[ScriptPopup] comctl32 v6 不可用，跳过原生系统弹窗");
                drop(unsafe { Box::from_raw(context_ptr) });
                release_slot();
                return;
            };
            let result = unsafe {
                task_dialog_indirect(
                    &config,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            };
            // SAFETY: TaskDialogIndirect has returned and can no longer call the
            // callback, so the context has no remaining borrowers.
            drop(unsafe { Box::from_raw(context_ptr) });
            DIALOGS
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&id);
            release_slot();
            if result.is_err() {
                tracing::warn!("[ScriptPopup] 原生系统弹窗失败: {}", result.message());
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

    /// 直接拼进 cmd /K 的启动命令串（不走 .bat 文件）。
    /// .bat 里 chcp 65001 有个老坑：默认代码页（如 936）启动的 cmd 读到这行后，
    /// 批处理解析器按新代码页重扫缓冲会错位吞行——演出窗口黑屏只剩标题。
    /// 命令行参数本身走 Unicode，不经过代码页文件读取，没有这个问题。
    /// 文件路径转成 8.3 短名：串内不再出现嵌套引号（cmd /K 引号规则会把它拆散），
    /// 也天然免疫含空格的用户目录。
    /// type 必须重定向到 CON：宿主是无控制台的 GUI 进程，cmd 继承不到有效 std，
    /// 直写 stdout 会全丢——CON 才是它新控制台这块屏幕。末尾长 ping 只为吊命：
    /// /K 在 std 无效时无法进入交互，没有它窗口执行完即消失（玩家自己点 X 结束）。
    fn cmd_launch_line(title: &str, text_path: &std::path::Path, blood_red: bool) -> String {
        use windows::Win32::Storage::FileSystem::GetShortPathNameW;
        let color = if blood_red { "4F" } else { "1F" };
        let short_path = {
            let wide: Vec<u16> = text_path
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let needed = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), None) };
            if needed == 0 {
                text_path.to_string_lossy().into_owned()
            } else {
                let mut buffer = vec![0u16; needed as usize];
                let copied = unsafe { GetShortPathNameW(PCWSTR(wide.as_ptr()), Some(&mut buffer)) };
                if copied == 0 {
                    text_path.to_string_lossy().into_owned()
                } else {
                    String::from_utf16_lossy(&buffer[..copied as usize])
                }
            }
        };
        format!(
            "chcp 65001 >nul & title {} & color {color} & cls & type {short_path} > CON & ping -n 999999 127.0.0.1 >nul",
            cmd_literal(title),
        )
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

    fn window_process_id(raw: isize) -> Option<u32> {
        let hwnd = HWND(raw as *mut core::ffi::c_void);
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return None;
        }
        let mut process_id = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        (process_id != 0).then_some(process_id)
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
            // 弹窗不自动关闭：玩家自己关窗后进程退出，这里再收尾临时文件与窗口槽位；
            // 剧本运行结束时 close_all 仍会统一清理所有残留窗口。
            loop {
                std::thread::sleep(Duration::from_millis(400));
                if !generation_is_current(generation) {
                    return;
                }
                let exited = {
                    let mut processes = PROCESSES
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    match processes.get_mut(&id) {
                        Some(popup) => matches!(popup.child.try_wait(), Ok(Some(_))),
                        None => return,
                    }
                };
                if exited {
                    close_process(id);
                    return;
                }
            }
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
            let mut notepad_tracking_uncertain = false;
            if let Some(marker) = popup.window_marker.as_deref() {
                // Windows 11 Notepad may hand the document to a pre-existing shared process.
                // Only close marker windows owned by the exact launcher PID; a marker in any
                // other process is preserved so we never close the user's existing tabs.
                let mut marker_seen = false;
                for _ in 0..10 {
                    let marker_handles = matching_windows(marker);
                    if !marker_handles.is_empty() {
                        marker_seen = true;
                        for raw in marker_handles {
                            if window_process_id(raw) == Some(popup_process_id) {
                                let hwnd = HWND(raw as *mut core::ffi::c_void);
                                let _ = unsafe {
                                    PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))
                                };
                                tracked_notepad_handles.push(raw);
                            } else {
                                notepad_document_open = true;
                            }
                        }
                        break;
                    }

                    // Classic Notepad can keep the exact spawned PID without putting the
                    // UUID in its title immediately; that PID remains a safe boundary.
                    tracked_notepad_handles = process_windows(popup_process_id);
                    if !tracked_notepad_handles.is_empty() {
                        for raw in &tracked_notepad_handles {
                            let hwnd = HWND(*raw as *mut core::ffi::c_void);
                            let _ =
                                unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) };
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                notepad_tracking_uncertain = !marker_seen && tracked_notepad_handles.is_empty();
                if !tracked_notepad_handles.is_empty() {
                    std::thread::sleep(Duration::from_millis(160));
                    let marker_lower = marker.to_lowercase();
                    for raw in &tracked_notepad_handles {
                        if let Some(title) = tracked_window_title(*raw) {
                            if title.to_lowercase().contains(&marker_lower) {
                                notepad_document_open = true;
                            } else {
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
                && (notepad_document_open
                    || notepad_tracking_uncertain
                    || (tracked_notepad_handles.is_empty() && child_running));
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
        blood_red: bool,
        generation: u64,
    ) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        // 剧本文字只进入临时纯文本文件，绝不成为 cmd.exe 命令的一部分；
        // 启动命令串仅含固定命令、净化后的标题与随机安全文件路径。
        let id = Uuid::new_v4();
        let text_path =
            std::env::temp_dir().join(format!("lingchat-console-{id}.txt"));
        let body = format!("{}\r\n", lines.join("\r\n"));
        fs::write(&text_path, body.as_bytes())
            .map_err(|error| format!("写入 CMD 演出文本失败: {error}"))?;

        let mut command = Command::new("cmd.exe");
        // cmd /K 的命令串按 cmd 自己的引号规则解析（不识别 Rust arg() 的 MSVCRT \" 转义）。
        // raw_arg 手工包外层引号 + /S 剥掉它：串内嵌套的 type "..." 引号原样保留。
        // CREATE_NEW_CONSOLE 必须给：宿主是无控制台的 GUI 进程，不给就是无窗后台 cmd；
        // 窗口内内容靠命令串里的 type > CON 落到这块新屏幕上。
        command
            .raw_arg(format!(
                "/D /Q /S /K \"{}\"",
                cmd_launch_line(title, &text_path, blood_red)
            ))
            .creation_flags(CREATE_NEW_CONSOLE);
        match command.spawn() {
            Ok(child) => {
                tracing::info!("[ScriptPopup] cmd.exe 已拉起（pid={}）", child.id());
                register_process(child, vec![text_path], None, true, generation);
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(&text_path);
                Err(format!("启动真实 CMD 失败: {error}"))
            }
        }
    }

    fn notepad_command(path: &std::path::Path) -> Command {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("notepad.exe");
        // Windows 11 记事本没有文档化的 /newWindow 开关；部分版本会把它当成
        // 第一个文件名并弹“文件名无效”。只传已存在的 UTF-8 临时文件路径。
        command.arg(path).creation_flags(CREATE_NO_WINDOW);
        command
    }

    fn spawn_notepad(
        title: &str,
        lines: &[String],
        generation: u64,
    ) -> Result<(), String> {
        let marker = format!("lingchat-note-{}", Uuid::new_v4());
        let path = std::env::temp_dir().join(format!("{marker}.txt"));
        let body = format!("{title}\r\n\r\n{}\r\n", lines.join("\r\n"));
        let mut encoded = vec![0xEF, 0xBB, 0xBF];
        encoded.extend_from_slice(body.as_bytes());
        fs::write(&path, encoded).map_err(|error| format!("写入临时记事本残页失败: {error}"))?;
        let mut command = notepad_command(&path);
        match command.spawn() {
            Ok(child) => {
                register_process(child, vec![path], Some(marker), false, generation);
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
        tracing::info!("[ScriptPopup] spawn_one 开始：style={} title={:?}", request.style, request.title);
        let result = match request.style.as_str() {
            "error" => {
                spawn_task_dialog(request.title.clone(), text, false, generation);
                Ok(())
            }
            "warning" => {
                spawn_task_dialog(request.title.clone(), text, true, generation);
                Ok(())
            }
            "notepad" => spawn_notepad(&request.title, &request.lines, generation),
            "blood_cmd" => spawn_cmd(&request.title, &request.lines, true, generation),
            _ => spawn_cmd(&request.title, &request.lines, false, generation),
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
        use super::{cmd_launch_line, notepad_command};

        #[test]
        fn blood_cmd_uses_red_background_without_powershell() {
            let command = cmd_launch_line(
                "RUNTIME",
                std::path::Path::new(r"C:\Temp\lingchat-console-test.txt"),
                true,
            );
            assert!(command.contains("color 4F"));
            assert!(command.contains("type "));
            assert!(!command.to_ascii_lowercase().contains("powershell"));
            assert!(!command.to_ascii_lowercase().contains("pwsh"));
            assert!(!command.contains("safe story text"));
        }

        #[test]
        fn normal_cmd_uses_blue_background() {
            let command = cmd_launch_line(
                "RUNTIME",
                std::path::Path::new(r"C:\Temp\lingchat-console-test.txt"),
                false,
            );
            assert!(command.contains("color 1F"));
        }

        #[test]
        fn notepad_receives_only_the_existing_file_path() {
            let path = std::path::Path::new(r"C:\Temp Folder\被撕掉的台词.txt");
            let command = notepad_command(path);
            let args: Vec<_> = command.get_args().collect();
            assert_eq!(args, vec![path.as_os_str()]);
            assert!(!args
                .iter()
                .any(|arg| arg.to_string_lossy().eq_ignore_ascii_case("/newWindow")));
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
