use std::path::PathBuf;
use std::sync::OnceLock;

use tauri::Manager;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化 data 目录缓存（必须在 App 启动时调用一次）。
pub fn init_data_dir(app: &tauri::AppHandle) {
    let dir = resolve_data_dir(app);
    DATA_DIR.set(dir).expect("data_dir already initialized");
}

/// 获取已缓存的 data 目录（必须先调用 `init_data_dir`）。
pub fn get_data_dir() -> &'static PathBuf {
    DATA_DIR
        .get()
        .expect("data_dir not initialized — call init_data_dir first")
}

/// 解析 data 目录路径。
///
/// - 移动端（android/ios）：始终使用平台沙盒内的应用数据目录
/// - 桌面端开发模式（debug）：项目根目录下的 `data/`
/// - 桌面端发布模式（release portable）：exe 所在目录下的 `data/`
///
/// 所有可读写数据（数据库、game_data、存档等）都放在此目录下。
fn resolve_data_dir(app: &tauri::AppHandle) -> PathBuf {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        // 移动端必须使用平台沙盒路径，无论 debug/release
        app.path()
            .app_data_dir()
            .expect("failed to resolve app_data_dir on mobile")
    } else if cfg!(debug_assertions) {
        // 桌面端开发模式：项目根目录的 data/
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
    } else {
        // 桌面端便携发布：data 目录放在 exe 旁边
        std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .join("data")
    }
}

/// 首次启动时将 APK 内嵌资源播种到 data 目录（仅移动端）。
///
/// 桌面端直接使用已有的 data/ 目录，此函数为 no-op。
pub fn seed_data_dir(app: &tauri::AppHandle) -> anyhow::Result<()> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let data_dir = get_data_dir().clone();
        let marker = data_dir.join(".seeded");
        let manifest = data_dir.join("data_manifest.json");

        if marker.exists() && manifest.exists() {
            return Ok(());
        }

        // 可能存在上一次损坏的解压残留（如 Windows zip 的反斜杠路径），
        // 删除标记以触发干净的重新解压
        let _ = std::fs::remove_file(&marker);

        seed_via_fs_plugin(app, &data_dir)?;

        std::fs::write(&marker, b"")
            .map_err(|e| anyhow::anyhow!("failed to write .seeded marker: {}", e))?;
        tracing::info!("Data directory seeding complete");
    }

    // Desktop: data/ dir already exists, nothing to seed
    let _ = app;
    Ok(())
}

/// 通过 tauri-plugin-fs 读取打包的 data.zip 并解压到 data_dir。
///
/// 所有游戏资源文件在构建时被打包成一个 zip（ASCII 文件名），
/// 该 zip 由 tauri.conf.json 的 bundle.resources 映射到 APK assets 中。
/// 这种方式从根本上避开了 Android asset:// 协议处理中文路径的问题。
#[cfg(any(target_os = "android", target_os = "ios"))]
fn seed_via_fs_plugin(app: &tauri::AppHandle, data_dir: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::Context;
    use tauri_plugin_fs::FsExt;

    let resource_dir = app.path().resource_dir()
        .context("failed to resolve resource_dir on mobile")?;

    let base = resource_dir.to_string_lossy();
    let base = base.trim_end_matches('/');

    // 读取 data.zip（唯一需要从 asset:// 读取的文件，纯 ASCII 路径）
    let zip_asset = format!("{}/data/data.zip", base);
    let zip_bytes = app.fs()
        .read(std::path::Path::new(&zip_asset))
        .with_context(|| format!("failed to read data.zip from {}", zip_asset))?;

    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .context("failed to open data.zip archive")?;

    let total = archive.len();
    tracing::info!("Extracting {} entries from data.zip", total);

    let mut extracted = 0usize;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .with_context(|| format!("failed to read entry {} in data.zip", i))?;

        let raw_name = file.name().to_string();

        // 跳过目录条目和 data.zip 自身
        if raw_name.ends_with('/') || raw_name.ends_with('\\') || raw_name == "data.zip" {
            continue;
        }

        // Windows zip 用反斜杠做分隔符，在 Unix (Android) 上必须转为正斜杠
        let name = raw_name.replace('\\', "/");

        let dest = data_dir.join(&name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut out = std::fs::File::create(&dest)
            .with_context(|| format!("failed to create {:?}", dest))?;
        std::io::copy(&mut file, &mut out)
            .with_context(|| format!("failed to extract {} from data.zip", name))?;
        extracted += 1;
    }

    tracing::info!("Extracted {} files from data.zip", extracted);
    Ok(())
}
