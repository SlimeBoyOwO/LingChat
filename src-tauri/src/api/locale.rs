//! 界面语言文件（i18n）的运行时加载：语言文件存放在数据目录 `data/locales/`
//! 下（如 `zh-CN.json` / `ja.json`），用户可直接编辑，重启或切换语言后生效。
//!
//! 前端每次启动会传入内置词条作为播种内容：文件不存在时写入播种内容，
//! 存在时直接读取文件内容返回（用户修改过的内容优先）。

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

    std::fs::read_to_string(&path).map_err(|e| format!("读取语言文件失败: {e}"))
}
