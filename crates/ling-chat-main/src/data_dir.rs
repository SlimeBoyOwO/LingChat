use std::path::PathBuf;
use std::sync::OnceLock;

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

/// 获取已缓存的 data 目录；没初始化时返回 `None`。
///
/// 给「直接构造服务、不走 App 启动流程」的调用方用（单测、集成测试、外部工具）——
/// 它们拿不到 AppHandle，调用 [`get_data_dir`] 会直接 panic。
pub fn try_get_data_dir() -> Option<&'static PathBuf> {
    DATA_DIR.get()
}

/// 解析 data 目录路径。
///
/// 优先级：
/// 1. Android：应用专属外部存储 (/storage/emulated/0/Android/data/<package>/files)
/// 2. iOS：沙盒 Documents 目录（配合 Info.ios.plist 的文件共享键，「文件」App 可见）
/// 3. 桌面端开发模式（debug）：项目根目录下的 `data/`
/// 4. 桌面端发布模式（release portable）：exe 所在目录下的 `data/`
fn resolve_data_dir(app: &tauri::AppHandle) -> PathBuf {
    resolve_data_dir_impl(app)
}

#[cfg(target_os = "android")]
fn resolve_data_dir_impl(app: &tauri::AppHandle) -> PathBuf {
    use tauri_plugin_android_fs::{AndroidFsExt, AppDir};
    app.android_fs()
        .app_storage()
        .resolve_path(None, AppDir::Data)
        .expect("failed to resolve Android external files dir")
}

#[cfg(target_os = "ios")]
fn resolve_data_dir_impl(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    app.path()
        .document_dir()
        .expect("failed to resolve document_dir on iOS")
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn resolve_data_dir_impl(_app: &tauri::AppHandle) -> PathBuf {
    if cfg!(debug_assertions) {
        // 桌面端开发模式：项目根目录的 data/
        // 本 crate 位于 <repo>/crates/ling-chat-main，故需上跳两级回仓库根。
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
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

/// 首次启动时将内嵌资源播种到 data 目录。
///
/// - 移动端（android/ios）：从 APK 中解压 data.7z
/// - 桌面端：从安装包的 `data/.official/` 复制 default 资源
pub fn seed_data_dir(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let data_dir = get_data_dir().clone();
    let seeded = data_dir.join(".seeded");

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let manifest = data_dir.join("data_manifest.json");
        if seeded.exists() && manifest.exists() {
            return Ok(());
        }
        let _ = std::fs::remove_file(&seeded);
        seed_via_fs_plugin(app, &data_dir)?;
        std::fs::write(&seeded, b"")
            .map_err(|e| anyhow::anyhow!("failed to write .seeded marker: {}", e))?;
        tracing::info!("Data directory seeding complete (mobile)");
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        seed_desktop(app, &data_dir, &seeded)?;
    }

    Ok(())
}

/// 桌面端播种逻辑。
///
/// - `.official/` 不存在 → 无需操作
/// - `.seeded` 不存在 → 首次启动，全量 seed ← 删除 .official
/// - `.seeded` 存在 + `.official` 存在 → 更新待同步（留给前端 check_resource_sync）
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn seed_desktop(
    _app: &tauri::AppHandle,
    data_dir: &std::path::Path,
    seeded: &std::path::Path,
) -> anyhow::Result<()> {
    let official = data_dir.join(".official");

    if !official.exists() {
        return Ok(());
    }

    if !seeded.exists() {
        // 首次启动：全量播种
        tracing::info!("First launch — seeding from .official/");
        crate::resource_sync::sync::seed_full_from_official(data_dir, &official)?;
        std::fs::write(seeded, b"")?;
        std::fs::remove_dir_all(&official)?;
        tracing::info!("Seed complete, .official removed");
    }
    // seeded 存在 + official 存在 → 更新待处理，不自动操作

    Ok(())
}

/// 通过 tauri-plugin-fs 读取打包的 data.7z 并解压到 data_dir。
#[cfg(any(target_os = "android", target_os = "ios"))]
fn seed_via_fs_plugin(app: &tauri::AppHandle, data_dir: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::Context;
    use tauri::Manager;
    use tauri_plugin_fs::FsExt;

    let resource_dir = app
        .path()
        .resource_dir()
        .context("failed to resolve resource_dir on mobile")?;

    let base = resource_dir.to_string_lossy();
    let base = base.trim_end_matches('/');

    let archive_asset = format!("{}/data/data.7z", base);

    let archive_bytes = app
        .fs()
        .read(std::path::Path::new(&archive_asset))
        .with_context(|| format!("failed to read data.7z from {}", archive_asset))?;

    let cursor = std::io::Cursor::new(archive_bytes);
    tracing::info!("Extracting data.7z ({} bytes)", cursor.get_ref().len());

    sevenz_rust2::decompress(cursor, data_dir).context("failed to extract data.7z")?;

    tracing::info!("Data extraction from data.7z complete");
    Ok(())
}
