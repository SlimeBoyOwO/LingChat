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
        let local_manifest_path = data_dir.join("data_manifest.json");
        let was_seeded = seeded.exists();
        let needs_full = !was_seeded;
        // 比较打包清单与本地清单。若 .seeded 已存在但打包内容有新增，
        // 仅增量补齐缺失文件，不覆盖本地已有/已改动数据。
        let packaged_manifest = read_packaged_manifest(app)?;
        let local_manifest = local_manifest_path
            .exists()
            .then(|| crate::manifest::DataManifest::load(&local_manifest_path).ok())
            .flatten();
        let needs_refresh = needs_full
            || match (&packaged_manifest, &local_manifest) {
                (Some(p), Some(l)) => l.diff(p).files_to_add.iter().any(|_| true),
                (Some(_), None) => true,
                _ => false,
            };

        if !needs_refresh {
            return Ok(());
        }

        let skip_existing = was_seeded && needs_refresh && !needs_full;
        seed_via_fs_plugin(app, &data_dir, skip_existing)?;
        if let Some(p) = packaged_manifest {
            p.save(&local_manifest_path)?;
        }
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
fn seed_via_fs_plugin(
    app: &tauri::AppHandle,
    data_dir: &std::path::Path,
    skip_existing: bool,
) -> anyhow::Result<()> {
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

    tracing::info!("Extracting data.7z ({} bytes, skip_existing={})", archive_bytes.len(), skip_existing);

    // 归档在内存中，extract_sevenz 需要文件路径，先落盘到临时文件再解压。
    let tmp_archive = data_dir.join(".data_seed_tmp.7z");
    std::fs::write(&tmp_archive, &archive_bytes)
        .context("failed to stage data.7z to temp file")?;

    let result = crate::utils::archive::extract_sevenz(
        &tmp_archive,
        data_dir,
        skip_existing,
        &tokio_util::sync::CancellationToken::new(),
        &|_| {},
    );
    let _ = std::fs::remove_file(&tmp_archive);
    result.context("failed to extract data.7z")?;

    tracing::info!("Data extraction from data.7z complete");
    Ok(())
}

/// 读取打包在资源目录（data.7z 之外）的 `data/data_manifest.json`。
///
/// 构建脚本 `prepare-bundled-resources.mjs` 会把清单额外复制一份放到
/// `data/data_manifest.json`，供这里快速读取，无需全量解压 data.7z 即可
/// 得知默认资源文件集合，从而判断是否要增量补齐缺失文件。
#[cfg(any(target_os = "android", target_os = "ios"))]
fn read_packaged_manifest(
    app: &tauri::AppHandle,
) -> anyhow::Result<Option<crate::manifest::DataManifest>> {
    use anyhow::Context;
    use tauri::Manager;
    use tauri_plugin_fs::FsExt;

    let base = app
        .path()
        .resource_dir()
        .context("failed to resolve resource_dir on mobile")?;
    let base = base.to_string_lossy();
    let base = base.trim_end_matches('/');
    let asset = format!("{}/data/data_manifest.json", base);

    match app.fs().read(std::path::Path::new(&asset)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| anyhow::anyhow!("failed to parse packaged manifest: {e}")),
        Err(_) => Ok(None),
    }
}
