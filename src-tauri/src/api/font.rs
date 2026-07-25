use serde::Serialize;

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FontFamilyInfo {
    pub name: String,
}

// ========== Tauri 命令 ==========

/// 枚举系统已安装的字体族名，供前端字体选择器使用。
///
/// Windows: 使用 GDI `EnumFontFamiliesExW`（复用仓库已开启的 `Win32_Graphics_Gdi` feature，
/// 零新增依赖）。
/// 其他平台: 暂未实现，返回空列表（前端将回退到“软件默认”项，不会报错卡界面）。
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn list_system_fonts() -> Result<Vec<FontFamilyInfo>, String> {
    use std::cell::RefCell;
    use std::os::windows::ffi::OsStringExt;
    use std::rc::Rc;

    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::Graphics::Gdi::{
        EnumFontFamiliesExW, GetDC, ReleaseDC, DEFAULT_CHARSET, LOGFONTW, TEXTMETRICW,
    };

    // 枚举回调：把每个字体族的 lfFaceName 收集到 Rc<RefCell<Vec<String>>>，并去重。
    // FONTENUMPROCW 签名：*const LOGFONTW, *const TEXTMETRICW, u32, LPARAM -> i32
    unsafe extern "system" fn enum_proc(
        logfont: *const LOGFONTW,
        _metric: *const TEXTMETRICW,
        _flags: u32,
        lparam: LPARAM,
    ) -> i32 {
        if logfont.is_null() {
            return 1; // 继续枚举
        }
        let lf = &*logfont;
        // lfFaceName 是 [u16; 32]，以 0 结尾
        let mut len = 0usize;
        while len < lf.lfFaceName.len() && lf.lfFaceName[len] != 0 {
            len += 1;
        }
        let name = std::ffi::OsString::from_wide(&lf.lfFaceName[..len])
            .to_string_lossy()
            .into_owned();

        let store_ptr = lparam.0 as *const RefCell<Vec<String>>;
        if !store_ptr.is_null() {
            let store = &*store_ptr;
            if let Ok(mut guard) = store.try_borrow_mut() {
                if !name.is_empty()
                    && !guard.iter().any(|n| n.eq_ignore_ascii_case(&name))
                {
                    guard.push(name);
                }
            }
        }
        1 // 非 0 表示继续枚举
    }

    let names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let store_ptr = Rc::as_ptr(&names) as isize;

    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return Err("无法获取屏幕 DC 进行字体枚举".to_string());
        }

        let mut logfont = LOGFONTW::default();
        logfont.lfCharSet = DEFAULT_CHARSET; // 枚举所有字符集的字体族

        // lparam 转递 RefCell 指针给回调
        let lparam = LPARAM(store_ptr);
        let _ = EnumFontFamiliesExW(hdc, &logfont, Some(enum_proc), lparam, 0);

        let _ = ReleaseDC(None, hdc);
    }

    let mut guard = names.borrow_mut();
    guard.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    Ok(guard.drain(..).map(|name| FontFamilyInfo { name }).collect())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn list_system_fonts() -> Result<Vec<FontFamilyInfo>, String> {
    // 非 Windows 暂未实现系统字体枚举：返回空，前端走“软件默认”即可，不报错。
    Ok(Vec::new())
}