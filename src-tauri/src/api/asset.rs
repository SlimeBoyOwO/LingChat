use std::fs;

use base64::Engine as _;

use crate::api::voice_dir;

use super::validate_path_in_base;

/// 读取文件并返回 base64 data URL（绕过 asset protocol scope 限制）
#[tauri::command]
pub fn get_asset_base64(file_path: String) -> Result<String, String> {
    let bytes = fs::read(&file_path).map_err(|e| format!("读取文件失败: {} — {}", file_path, e))?;

    let mime = detect_mime_type(&file_path);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

/// 获取 TTS 生成的语音文件，返回 base64 data URL。
/// `file_name` 为 basename（如 `abc_part_1.wav`），自动在 `<data_dir>/voice/` 下查找。
#[tauri::command]
pub fn get_voice_audio(file_name: String) -> Result<String, String> {
    let base = voice_dir();
    let resolved = base.join(&file_name);

    // 路径穿越防护
    validate_path_in_base(&resolved, &base)?;

    if !resolved.exists() {
        return Err(format!("语音文件不存在: {}", file_name));
    }

    let bytes = fs::read(&resolved)
        .map_err(|e| format!("读取语音文件失败: {} — {}", resolved.display(), e))?;

    let mime = detect_mime_type(&file_name);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

fn detect_mime_type(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "tif" | "tiff" => "image/tiff",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        _ => "application/octet-stream",
    }
}
