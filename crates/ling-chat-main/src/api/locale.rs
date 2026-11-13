//! 界面语言文件（i18n）的运行时加载：语言文件存放在数据目录 `data/locales/`
//! 下（如 `zh-CN.json` / `ja.json`），用户可直接编辑，重启或切换语言后生效。

use serde_json::Value;

/// 从语言文件 JSON 中读取内置词条版本标记（无标记返回 None，视为旧文件）。
fn locale_version_of(content: &str) -> Option<String> {
    let v: Value = serde_json::from_str(content).ok()?;
    v.get("__locale_version")?.as_str().map(|s| s.to_string())
}

/// 读取界面语言文件；不存在时用内置词条播种后返回。
///
/// `locale` 仅允许字母数字与连字符（防路径穿越）。
#[tauri::command]
pub fn get_locale_messages(locale: String, seed_content: String) -> Result<String, String> {
    if locale.is_empty()
        || !locale
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(format!("非法 locale 名: {locale}"));
    }

    let dir = super::data_dir().join("locales");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建语言目录失败: {e}"))?;

    let path = dir.join(format!("{locale}.json"));
    if !path.exists() {
        std::fs::write(&path, &seed_content).map_err(|e| format!("播种语言文件失败: {e}"))?;
        tracing::info!("已播种语言文件: {}", path.display());
        return Ok(seed_content);
    }

    let existing = std::fs::read_to_string(&path).map_err(|e| format!("读取语言文件失败: {e}"))?;

    // 内置词条版本变化（或旧文件没有版本标记）时重新播种，
    // 修复旧播种词条覆盖新词条的问题；版本一致则保留文件（含用户编辑）
    let seed_version = locale_version_of(&seed_content);
    let existing_version = locale_version_of(&existing);
    if let Some(seed_version) = seed_version {
        if existing_version.as_deref() != Some(seed_version.as_str()) {
            std::fs::write(&path, &seed_content).map_err(|e| format!("更新语言文件失败: {e}"))?;
            tracing::info!(
                "内置词条版本变化（{} → {}），已重新播种 {}",
                existing_version.as_deref().unwrap_or("无版本标记"),
                seed_version,
                path.display()
            );
            return Ok(seed_content);
        }
    }

    Ok(existing)
}
