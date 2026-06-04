//! Windows-specific implementation for window enumeration and capture.
//!
//! This module works around a limitation in the `xcap` crate where
//! `is_valid_window()` unconditionally filters out all windows owned by the
//! current process (PID check). This prevents a Tauri app from listing
//! or screenshotting its own windows.
//!
//! Our `enumerate_all_windows()` replicates xcap's filtering logic but
//! WITHOUT the PID filter, allowing the app's own windows to appear.

use std::{ffi::c_void, mem, ptr};

use image::RgbaImage;
use scopeguard::guard;
use widestring::U16CString;
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{BOOL, HANDLE, HWND, LPARAM, MAX_PATH, RECT, TRUE},
        Graphics::{
            Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS},
            Gdi::{
                BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
                GetCurrentObject, GetDIBits, GetObjectW, GetWindowDC, IsRectEmpty, ReleaseDC,
                SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP, HDC,
                OBJ_BITMAP, SRCCOPY,
            },
        },
        Storage::{
            FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
            Xps::{PrintWindow, PRINT_WINDOW_FLAGS},
        },
        System::{
            ProcessStatus::{GetModuleBaseNameW, GetModuleFileNameExW},
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW,
            GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
            GWL_EXSTYLE, WINDOW_EX_STYLE, WS_EX_TOOLWINDOW,
        },
    },
};

use crate::commands::ScreenshotableWindow;

// ---------------------------------------------------------------------------
// Window bounds (replicates xcap's get_window_bounds)
// ---------------------------------------------------------------------------

/// Get the visible window bounds via DWM (excludes invisible resize borders).
fn get_window_bounds(hwnd: HWND) -> Result<RECT, String> {
    let mut rect = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut c_void,
            mem::size_of::<RECT>() as u32,
        )
        .map_err(|e| format!("DwmGetWindowAttribute failed: {e}"))?;
    }
    Ok(rect)
}

// ---------------------------------------------------------------------------
// Window title
// ---------------------------------------------------------------------------

/// Get the title text of a window.
///
/// # Deadlock Safety
///
/// `GetWindowTextW` can potentially deadlock if called on a window owned by
/// the current process AND the calling thread is the window's message loop
/// thread. In a Tauri app this is not a concern because:
/// - Tauri IPC commands run on a tokio async thread pool
/// - The Windows message loop runs on the main UI thread
/// - They are always different threads
unsafe fn get_window_title(hwnd: HWND) -> Result<String, String> {
    let text_length = GetWindowTextLengthW(hwnd);
    let mut wide_buffer = vec![0u16; (text_length + 1) as usize];
    GetWindowTextW(hwnd, &mut wide_buffer);
    U16CString::from_vec_truncate(wide_buffer)
        .to_string()
        .map_err(|e| format!("Failed to convert window title: {e}"))
}

// ---------------------------------------------------------------------------
// Window PID
// ---------------------------------------------------------------------------

unsafe fn get_window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    pid
}

// ---------------------------------------------------------------------------
// DWM cloaking check
// ---------------------------------------------------------------------------

fn is_window_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked = 0u32;
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            mem::size_of::<u32>() as u32,
        );
        result.is_ok() && cloaked != 0
    }
}

// ---------------------------------------------------------------------------
// App name resolution (simplified from xcap)
// ---------------------------------------------------------------------------

fn get_module_basename(handle: HANDLE) -> Result<String, String> {
    unsafe {
        let mut module_base_name_w = [0u16; MAX_PATH as usize];
        let result = GetModuleBaseNameW(handle, None, &mut module_base_name_w);

        if result == 0 {
            GetModuleFileNameExW(Some(handle), None, &mut module_base_name_w);
        }

        U16CString::from_vec_truncate(module_base_name_w)
            .to_string()
            .map_err(|e| format!("Failed to convert module name: {e}"))
    }
}

#[derive(Debug, Default)]
struct LangCodePage {
    pub w_language: u16,
    pub w_code_page: u16,
}

unsafe fn get_app_name(pid: u32) -> String {
    let scope_guard_handle = match OpenProcess(
        PROCESS_QUERY_LIMITED_INFORMATION,
        false,
        pid,
    ) {
        Ok(handle) => handle,
        Err(_) => return String::new(),
    };

    let mut filename = [0u16; MAX_PATH as usize];
    GetModuleFileNameExW(Some(scope_guard_handle), None, &mut filename);

    let pcw_filename = PCWSTR::from_raw(filename.as_ptr());

    let file_version_info_size_w = GetFileVersionInfoSizeW(pcw_filename, None);
    if file_version_info_size_w == 0 {
        return get_module_basename(scope_guard_handle).unwrap_or_default();
    }

    let mut file_version_info = vec![0u16; file_version_info_size_w as usize];

    if GetFileVersionInfoW(
        pcw_filename,
        None,
        file_version_info_size_w,
        file_version_info.as_mut_ptr().cast(),
    )
    .is_err()
    {
        return get_module_basename(scope_guard_handle).unwrap_or_default();
    }

    let mut lang_code_pages_ptr = ptr::null_mut();
    let mut lang_code_pages_length = 0u32;

    let ok = VerQueryValueW(
        file_version_info.as_ptr().cast(),
        &HSTRING::from("\\VarFileInfo\\Translation"),
        &mut lang_code_pages_ptr,
        &mut lang_code_pages_length,
    )
    .as_bool();

    if !ok {
        return get_module_basename(scope_guard_handle).unwrap_or_default();
    }

    let lang_code_pages: &[LangCodePage] =
        core::slice::from_raw_parts(lang_code_pages_ptr.cast(), lang_code_pages_length as usize);

    // Try these keys in order (preferring FileDescription)
    let keys = [
        "FileDescription",
        "ProductName",
        "InternalName",
        "OriginalFilename",
    ];

    for key in keys {
        for lang_code_page in lang_code_pages {
            let query_key = HSTRING::from(format!(
                "\\StringFileInfo\\{:04x}{:04x}\\{key}",
                lang_code_page.w_language, lang_code_page.w_code_page
            ));

            let mut value_ptr = ptr::null_mut();
            let mut value_length: u32 = 0;

            let ok = VerQueryValueW(
                file_version_info.as_ptr().cast(),
                &query_key,
                &mut value_ptr,
                &mut value_length,
            )
            .as_bool();

            if !ok {
                continue;
            }

            let value: &[u16] =
                core::slice::from_raw_parts(value_ptr.cast(), value_length as usize);
            if let Ok(attr) = U16CString::from_vec_truncate(value).to_string() {
                let attr = attr.trim().to_string();
                if !attr.is_empty() {
                    return attr;
                }
            }
        }
    }

    get_module_basename(scope_guard_handle).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Window validation (same as xcap's is_valid_window but WITHOUT PID filter)
// ---------------------------------------------------------------------------

/// Determine whether a window is valid for screenshot capture.
///
/// This mirrors xcap's `is_valid_window()` with one critical difference:
/// we do NOT filter out windows owned by the current process. This is the
/// fix for issue #8 — the Tauri app's own windows should be screenshotable.
unsafe fn is_valid_window(hwnd: HWND) -> bool {
    // 1. Must be a real, visible window
    if !IsWindow(Some(hwnd)).as_bool() || !IsWindowVisible(hwnd).as_bool() {
        return false;
    }

    // 2. Must have a valid class name
    let mut lp_class_name = [0u16; MAX_PATH as usize];
    let lp_class_name_length = GetClassNameW(hwnd, &mut lp_class_name) as usize;
    if lp_class_name_length < 1 {
        return false;
    }

    let class_name =
        U16CString::from_vec_truncate(&lp_class_name[0..lp_class_name_length])
            .to_string()
            .unwrap_or_default();
    if class_name.is_empty() {
        return false;
    }

    // 3. Check extended style and title
    let gwl_ex_style = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32);
    let title = get_window_title(hwnd).unwrap_or_default();

    // 4. Filter out tool windows with empty titles (except the taskbar)
    if gwl_ex_style.contains(WS_EX_TOOLWINDOW)
        && class_name.as_str() != "Shell_TrayWnd"
        && title.is_empty()
    {
        return false;
    }

    // 5. Skip shell/desktop windows
    if class_name == "Progman" || class_name == "Button" {
        return false;
    }

    // 6. Skip DWM-cloaked windows
    if is_window_cloaked(hwnd) {
        return false;
    }

    // 7. Must have non-empty frame bounds
    let mut rect = RECT::default();
    let get_rect_result = DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut rect as *mut RECT as *mut c_void,
        mem::size_of::<RECT>() as u32,
    );

    if get_rect_result.is_err() || IsRectEmpty(&rect).as_bool() {
        return false;
    }

    // NOTE: We intentionally do NOT filter by PID.
    // xcap filters out own-process windows here, but we need them.
    true
}

// ---------------------------------------------------------------------------
// Window enumeration
// ---------------------------------------------------------------------------

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, state: LPARAM) -> BOOL {
    let state = Box::leak(Box::from_raw(state.0 as *mut Vec<HWND>));

    if is_valid_window(hwnd) {
        state.push(hwnd);
    }

    TRUE
}

/// Enumerate all screenshotable windows on Windows.
///
/// Unlike `xcap::Window::all()`, this includes windows owned by the
/// current process (i.e. the Tauri app's own windows).
pub fn enumerate_all_windows() -> Result<Vec<ScreenshotableWindow>, String> {
    let hwnds_mut_ptr: *mut Vec<HWND> = Box::into_raw(Box::default());

    let hwnds = unsafe {
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(hwnds_mut_ptr as isize),
        )
        .map_err(|e| format!("EnumWindows failed: {e}"))?;
        Box::from_raw(hwnds_mut_ptr)
    };

    let mut screenshotable_windows = vec![];

    for &hwnd in hwnds.iter() {
        unsafe {
            let id = hwnd.0 as u32;

            // Skip minimized windows
            if IsIconic(hwnd).as_bool() {
                continue;
            }

            let pid = get_window_pid(hwnd);
            let title = get_window_title(hwnd).unwrap_or_default();
            let app_name = get_app_name(pid);

            let name = if title.is_empty() || app_name.eq(&title) {
                app_name.clone()
            } else {
                format!("{} - {}", app_name, title)
            };

            screenshotable_windows.push(ScreenshotableWindow {
                id,
                name,
                title,
                app_name,
            });
        }
    }

    Ok(screenshotable_windows)
}

// ---------------------------------------------------------------------------
// BGRA to RGBA conversion (replicates xcap's bgra_to_rgba)
// ---------------------------------------------------------------------------

fn bgra_to_rgba(mut buffer: Vec<u8>) -> Vec<u8> {
    for src in buffer.chunks_exact_mut(4) {
        src.swap(0, 2);
        // Fix for old Windows: fully transparent pixels should be opaque
        if src[3] == 0 {
            src[3] = 255;
        }
    }
    buffer
}

fn to_rgba_image(
    hdc_mem: HDC,
    h_bitmap: HBITMAP,
    width: i32,
    height: i32,
) -> Result<RgbaImage, String> {
    let buffer_size = width * height * 4;
    let mut bitmap_info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biSizeImage: buffer_size as u32,
            biCompression: 0,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut buffer = vec![0u8; buffer_size as usize];

    unsafe {
        let failed = GetDIBits(
            hdc_mem,
            h_bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        ) == 0;

        if failed {
            return Err("GetDIBits failed".to_string());
        }
    }

    RgbaImage::from_raw(width as u32, height as u32, bgra_to_rgba(buffer))
        .ok_or_else(|| "RgbaImage::from_raw failed".to_string())
}

// ---------------------------------------------------------------------------
// Window capture (replicates xcap's gdi::capture_window)
// ---------------------------------------------------------------------------

fn delete_bitmap_object(val: HBITMAP) {
    unsafe {
        let _ = DeleteObject(val.into());
    }
}

/// Capture an own-process window using GDI.
///
/// This replicates xcap's `gdi::capture_window()` and is used as a fallback
/// when xcap's `Window::all()` doesn't include the target window (because
/// it was filtered out by the PID check).
///
/// The capture chain: PrintWindow(PW_RENDERFULLCONTENT) →
/// PrintWindow(0) → PrintWindow(4) → BitBlt fallback
pub fn capture_own_window(id: u32) -> Result<RgbaImage, String> {
    // Reconstruct HWND from the u32 id
    let hwnd = HWND(id as usize as *mut c_void);

    // Get the true window bounds (without DWM invisible resize borders)
    let window_bounds = get_window_bounds(hwnd)?;
    let window_width = window_bounds.right - window_bounds.left;
    let window_height = window_bounds.bottom - window_bounds.top;

    unsafe {
        let mut bitmap_width = window_width;
        let mut bitmap_height = window_height;

        // Get the window DC
        let scope_guard_hdc_window = guard(
            GetWindowDC(Some(hwnd)),
            |val| {
                ReleaseDC(Some(hwnd), val);
            },
        );

        // Check if the window DC has its own bitmap (GDI-rendered windows)
        let hgdi_obj = GetCurrentObject(*scope_guard_hdc_window, OBJ_BITMAP);
        let mut bitmap = BITMAP::default();

        if GetObjectW(
            hgdi_obj,
            mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut c_void),
        ) != 0
        {
            bitmap_width = bitmap.bmWidth;
            bitmap_height = bitmap.bmHeight;
        }

        // Create memory DC and bitmap
        let scope_guard_hdc_mem = guard(
            CreateCompatibleDC(Some(*scope_guard_hdc_window)),
            |val| {
                let _ = DeleteDC(val);
            },
        );
        let scope_guard_h_bitmap = guard(
            CreateCompatibleBitmap(*scope_guard_hdc_window, bitmap_width, bitmap_height),
            delete_bitmap_object,
        );

        let previous_object =
            SelectObject(*scope_guard_hdc_mem, (*scope_guard_h_bitmap).into());

        // Try PrintWindow chain: PW_RENDERFULLCONTENT → PW(0) → PW(4) → BitBlt
        // PW_RENDERFULLCONTENT (2) - available on Windows 8+
        let mut is_success =
            PrintWindow(hwnd, *scope_guard_hdc_mem, PRINT_WINDOW_FLAGS(2)).as_bool();

        // PW_CLIENTONLY (0) - if DWM composition is enabled
        if !is_success {
            if let Ok(dwm_enabled) =
                windows::Win32::Graphics::Dwm::DwmIsCompositionEnabled()
            {
                if dwm_enabled.as_bool() {
                    is_success = PrintWindow(
                        hwnd,
                        *scope_guard_hdc_mem,
                        PRINT_WINDOW_FLAGS(0),
                    )
                    .as_bool();
                }
            }
        }

        // PW_CLIENTONLY variant (4)
        if !is_success {
            is_success =
                PrintWindow(hwnd, *scope_guard_hdc_mem, PRINT_WINDOW_FLAGS(4)).as_bool();
        }

        // GDI BitBlt fallback
        if !is_success {
            is_success = BitBlt(
                *scope_guard_hdc_mem,
                0,
                0,
                bitmap_width,
                bitmap_height,
                Some(*scope_guard_hdc_window),
                0,
                0,
                SRCCOPY,
            )
            .is_ok();
        }

        // Restore the previous GDI object
        SelectObject(*scope_guard_hdc_mem, previous_object);

        if !is_success {
            return Err("All capture methods failed for own-process window".to_string());
        }

        // Convert to RGBA image
        let image = to_rgba_image(
            *scope_guard_hdc_mem,
            *scope_guard_h_bitmap,
            bitmap_width,
            bitmap_height,
        )?;

        // Crop to the actual window bounds (removing DWM invisible borders)
        let mut window_rect = RECT::default();
        GetWindowRect(hwnd, &mut window_rect)
            .map_err(|e| format!("GetWindowRect failed: {e}"))?;

        let scale_factor = bitmap_width as f32 / window_width as f32;
        let x = ((window_bounds.left - window_rect.left) as f32 * scale_factor).ceil();
        let y = ((window_bounds.top - window_rect.top) as f32 * scale_factor).ceil();

        // Check if the window is a standard Dialog (#32770) — DWM adds
        // invisible borders to these, but PrintWindow doesn't include them,
        // so no cropping is needed.
        let mut lp_class_name = [0u16; MAX_PATH as usize];
        let lp_class_name_length = GetClassNameW(hwnd, &mut lp_class_name) as usize;
        let class_name =
            U16CString::from_vec_truncate(&lp_class_name[0..lp_class_name_length])
                .to_string()
                .unwrap_or_default();

        let (crop_x, crop_y) = if class_name == "#32770" {
            (0.0f32, 0.0f32)
        } else {
            (x, y)
        };

        use image::{DynamicImage, imageops::FilterType};

        Ok(DynamicImage::ImageRgba8(image)
            .resize(
                window_width as u32,
                window_height as u32,
                FilterType::CatmullRom,
            )
            .crop(
                crop_x as u32,
                crop_y as u32,
                window_width as u32,
                window_height as u32,
            )
            .to_rgba8())
    }
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Check if a window is minimized.
pub fn is_window_minimized(id: u32) -> bool {
    let hwnd = HWND(id as usize as *mut c_void);
    unsafe { IsIconic(hwnd).as_bool() }
}
