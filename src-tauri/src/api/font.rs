use serde::Serialize;
use std::path::Path;

use crate::api;

// ========== 响应类型 ==========

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FontFamilyInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ImportedFontInfo {
    /// 字体族名（文件名去扩展名），供 CSS font-family 使用
    pub name: String,
    /// 在 data/fonts/ 中的文件名
    pub file_name: String,
    /// 字体文件绝对路径，供前端 convertFileSrc 使用
    pub file_path: String,
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

// ========== 导入字体管理（全平台通用）==========

/// 导入用户选择的字体文件到 data/fonts/ 目录。
///
/// 前端通过 `@tauri-apps/plugin-dialog` 的 `open()` 选择文件后，
/// 将文件路径传入此命令，后端负责校验和复制。
#[tauri::command]
pub fn import_font(path: String) -> Result<ImportedFontInfo, String> {
    let src = Path::new(&path);

    // 校验扩展名
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "ttf" && ext != "woff2" {
        return Err(format!(
            "不支持的字体格式: .{}——仅支持 .ttf 和 .woff2",
            ext
        ));
    }

    // 防路径穿越：只取文件名部分
    let original_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无效的文件名".to_string())?
        .to_string();

    // stem 作为 CSS font-family 名称
    let stem = Path::new(&original_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "无法解析字体名称".to_string())?
        .to_string();

    let dest_path = api::fonts_dir().join(&original_name);

    // 确保字体目录存在
    std::fs::create_dir_all(api::fonts_dir())
        .map_err(|e| format!("无法创建字体目录: {}", e))?;

    // 同名文件不允许导入
    if dest_path.exists() {
        return Err(format!("字体 \"{}\" 已存在，不能重复导入", stem));
    }

    // 复制文件
    std::fs::copy(&src, &dest_path)
        .map_err(|e| format!("复制字体文件失败: {}", e))?;

    Ok(ImportedFontInfo {
        name: stem,
        file_name: original_name,
        file_path: dest_path.to_string_lossy().into_owned(),
    })
}

/// 列出 data/fonts/ 目录下所有已导入的字体文件。
#[tauri::command]
pub fn list_imported_fonts() -> Result<Vec<ImportedFontInfo>, String> {
    let dir = api::fonts_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut fonts: Vec<ImportedFontInfo> = Vec::new();
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("无法读取字体目录: {}", e))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "ttf" && ext != "woff2" {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        fonts.push(ImportedFontInfo {
            name,
            file_name,
            file_path: path.to_string_lossy().into_owned(),
        });
    }

    fonts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(fonts)
}

/// 删除一个已导入的字体文件。
///
/// 注意：`name` 参数应为带扩展名的文件名（如 `"MyFont.ttf"`），
/// 即 `ImportedFontInfo.file_name`，而非去扩展名的 `name`。
#[tauri::command]
pub fn delete_imported_font(name: String) -> Result<(), String> {
    // 防路径穿越
    let safe_name = Path::new(&name)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "无效的字体文件名".to_string())?
        .to_string();

    let file_path = api::fonts_dir().join(&safe_name);

    // 安全校验：确保路径在 fonts_dir 内
    crate::utils::path::validate_path_in_base(&file_path, &api::fonts_dir())?;

    if !file_path.exists() {
        return Err(format!("字体文件不存在: {}", safe_name));
    }

    std::fs::remove_file(&file_path)
        .map_err(|e| format!("删除字体文件失败: {}", e))
}