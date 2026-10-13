use std::path::PathBuf;
use std::sync::OnceLock;

use crate::manifest::DataManifest;

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
    // Android: 应用专属外部存储 (AppStorage + AppDir::Data)
    // 对应 /storage/emulated/0/Android/data/<package>/files
    // 无需额外权限，卸载应用时自动清理
    app.android_fs()
        .app_storage()
        .resolve_path(None, AppDir::Data)
        .expect("failed to resolve Android external files dir")
}

#[cfg(target_os = "ios")]
fn resolve_data_dir_impl(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    // iOS：使用沙盒内的 Documents 目录（<container>/Documents）。
    // 配合 src-tauri/Info.ios.plist 中的 UIFileSharingEnabled /
    // LSSupportsOpeningDocumentsInPlace，用户可以在系统「文件」App 里
    // 直接看到并访问整个 data/ 目录（游戏数据、语音、截图等）。
    // 注意不要改回 app_data_dir()（Library/Application Support），
    // 那部分对「文件」App 不可见。
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
            // 升级安装：从包内清单与本地清单做 diff，只增量补齐新增/变更的
            // 默认资源（如可选的本地 embedding 模型），不覆盖用户自建文件。
            // 增量补齐失败只记警告：旧资源仍可用，不应阻断应用启动。
            if let Err(e) = apply_bundled_updates(app, &data_dir) {
                tracing::warn!(
                    "incremental bundled update failed (will retry next launch): {}",
                    e
                );
            }
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
///
/// 所有游戏资源文件在构建时被打包成一个 7z（ASCII 文件名），
/// 该 7z 由构建脚本放入移动端资源目录。
/// 这种方式从根本上避开了 asset:// 协议处理中文路径的问题。
///
/// 路径差异（实测：iOS 的 resource_dir() 返回的就是 bundle 内的 assets/ 目录，
/// 因此两端统一读 `{resource_dir}/data/data.7z`）：
/// - Android：data.7z 在 APK 的 assets/data/data.7z，resource_dir() 即 assets 根
///   → 读取 `{resource_dir}/data/data.7z`
/// - iOS：data.7z 在 `gen/apple/assets/data/data.7z`，XcodeGen 的 folder reference
///   把 `assets/` **整个目录**拷进 app bundle 后即 resource_dir()
///   → 读取 `{resource_dir}/data/data.7z`（不要再拼一层 `assets/`，否则变成
///   `<bundle>/assets/assets/data/data.7z` 导致播种失败）
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

    // 读取 data.7z（唯一需要从 asset:// 读取的文件，纯 ASCII 路径）
    // iOS：tauri 的 resource_dir() 返回 bundle 内的 assets/ 目录
    //   （gen/apple/assets/ 以 folder reference 打进 bundle 后即 resource_dir），
    //   因此 data.7z 的读取路径是 {resource_dir}/data/data.7z。
    // Android：resource_dir() 即 APK assets 根，同样读 {resource_dir}/data/data.7z。
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

/// 升级安装时的增量补齐：对比包内 data_manifest.json 与本地清单，
/// 只从 data.7z 中解压新增/变更的默认资源文件。
///
/// - 包内清单不存在（旧 APK，构建脚本尚未旁列清单）→ 跳过，保持旧行为
/// - 本地清单缺失或已损坏 → 跳过（`.seeded` 已存在，说明首装已全量解压过）
/// - 清单一致 → 无需任何 IO，直接返回
/// - 有差异 → 按条目增量解压到 data_dir，随后把本地清单更新为包内清单
///
/// 关键语义：
/// - 仅当清单声明的所有目标条目都成功落盘，才更新本地清单；否则保留旧清单，
///   让下次启动重新 diff 补齐（避免"没补齐却标记为已同步"的永久丢失）。
/// - 空文件（size 0、无数据流，如 .gitkeep）同样需要落盘。
/// - 写盘采用目标同目录临时文件 + rename，保证原子性且失败无残留。
#[cfg(any(target_os = "android", target_os = "ios"))]
fn apply_bundled_updates(app: &tauri::AppHandle, data_dir: &std::path::Path) -> anyhow::Result<()> {
    use anyhow::Context;
    use tauri::Manager;
    use tauri_plugin_fs::FsExt;

    let resource_dir = app
        .path()
        .resource_dir()
        .context("failed to resolve resource_dir on mobile")?;
    let base = resource_dir.to_string_lossy();
    let base = base.trim_end_matches('/');

    // 包内声明清单（构建脚本与 data.7z 平级旁列，27KB，便宜）
    let bundled_asset = format!("{}/data/data_manifest.json", base);
    let bundled_bytes = match app.fs().read(std::path::Path::new(&bundled_asset)) {
        Ok(b) => b,
        Err(_) => {
            tracing::debug!(
                "no bundled data_manifest.json at {}, skip incremental update",
                bundled_asset
            );
            return Ok(());
        },
    };
    let bundled: DataManifest = match serde_json::from_slice(&bundled_bytes) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("failed to parse bundled data_manifest.json: {}", e);
            return Ok(());
        },
    };

    let local_path = data_dir.join("data_manifest.json");
    let mut local = match DataManifest::load(&local_path) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("failed to load local data_manifest.json: {}", e);
            return Ok(());
        },
    };
    // 历史包（Windows 上用旧脚本 `sep` 构建）可能生成 `\` 分隔的清单 key，
    // 与包内 `/` 分隔的 key 无法在 data_dir 用 / 分隔的路径 anchor。就地归一化，
    // 让 diff 只把真正的新文件判为 add，避免旧本地清单被全量重解压。
    normalize_manifest_keys(&mut local);

    let diff = local.diff(&bundled);
    if diff.files_to_add.is_empty() && diff.files_to_modify.is_empty() {
        tracing::info!("Bundled data resources up to date, no sync needed");
        return Ok(());
    }

    tracing::info!(
        "Incremental sync: {} to add, {} to modify",
        diff.files_to_add.len(),
        diff.files_to_modify.len()
    );

    // 需要补齐/更新的条目集合。清单 key 与 7z 条目名统一使用 `/` 分隔，
    // 这里再做一次防御性规范化（兼容历史 Windows 构建可能遗留的 `\`）。
    let want: std::collections::HashSet<String> = diff
        .files_to_add
        .iter()
        .chain(diff.files_to_modify.iter())
        .map(|k| normalize_manifest_key(k))
        .collect();

    let archive_asset = format!("{}/data/data.7z", base);
    let archive_bytes = app
        .fs()
        .read(std::path::Path::new(&archive_asset))
        .with_context(|| format!("failed to read data.7z from {}", archive_asset))?;
    let mut cursor = std::io::Cursor::new(archive_bytes);
    tracing::info!("Opening data.7z ({} bytes)", cursor.get_ref().len());

    let mut archive = sevenz_rust2::ArchiveReader::new(cursor, sevenz_rust2::Password::empty())
        .context("failed to open data.7z for incremental sync")?;

    let pid = std::process::id();
    let mut synced: u64 = 0;
    archive
        .for_each_entries(|entry, reader| {
            // 目录/反条目不产生数据流，也不是清单声明的文件
            if entry.is_directory || entry.is_anti_item {
                return Ok(true);
            }

            let name = normalize_manifest_key(&entry.name);
            if !want.contains(&name) {
                // 必须消费该文件的数据流：块解码器按文件顺序推进，
                // 提前跳过会导致后续文件读取位置错位、CRC 校验失败。
                std::io::copy(reader, &mut std::io::sink())?;
                return Ok(true);
            }

            let dst = data_dir.join(&name);
            let Some(parent) = dst.parent() else {
                return Err(sevenz_rust2::Error::Other(
                    "syncing entry has no parent".into(),
                ));
            };
            std::fs::create_dir_all(parent)?;

            // 空文件（size 0、无数据流）也要落盘，防止清单与磁盘不一致
            let mut data = Vec::with_capacity(entry.size as usize);
            reader.read_to_end(&mut data)?;

            // 目标同目录写临时文件再 rename：同一文件系统内原子、失败无残留。
            let dst_name = dst.file_name().and_then(|n| n.to_str()).unwrap_or("file");
            let tmp = parent.join(format!(".syncing-{}-{}", pid, dst_name));
            if let Err(e) = (|| -> std::io::Result<()> {
                std::fs::write(&tmp, &data)?;
                std::fs::rename(&tmp, &dst)
            })() {
                let _ = std::fs::remove_file(&tmp);
                return Err(sevenz_rust2::Error::Io(e, "incremental sync write".into()));
            }
            synced += 1;
            Ok(true)
        })
        .context("failed to incrementally extract data.7z")?;

    if synced == want.len() as u64 {
        tracing::info!("Incremental data sync complete: {} file(s) written", synced);
        // 全部目标条目都成功，本地清单才可以更新为包内清单。
        bundled.save(&local_path)?;
    } else {
        // 部分条目缺失（理论上只在包与清单不一致时发生）：
        // 保留旧清单，下次启动重新 diff，避免静默丢失。
        tracing::warn!(
            "incremental sync: {}/{} manifest entries were found in data.7z; local manifest kept for retry",
            synced,
            want.len()
        );
    }
    Ok(())
}

/// 规范化清单/条目路径 key：一律使用 `/` 分隔。
/// 防御历史构建在 Windows 上以 `\` 生成 key、或旧清单混用分隔符的情况。
fn normalize_manifest_key(key: &str) -> String {
    key.replace('\\', "/")
}

/// 就地规范化一个清单的所有 key（`\` → `/`）。
/// 在 diff 之前对本地清单调用，保证新旧清单在同一坐标系下比较，
/// 避免历史 Windows 包（`\` 分隔）被误判为全量新增。
fn normalize_manifest_keys(manifest: &mut DataManifest) {
    let keys: Vec<String> = manifest.files.keys().cloned().collect();
    for k in keys {
        let nk = normalize_manifest_key(&k);
        if nk != k {
            if let Some(v) = manifest.files.remove(&k) {
                // 归一化后理论上仍保持双射；若意外碰撞，后者覆盖前者。
                manifest.files.insert(nk, v);
            }
        }
    }
}
