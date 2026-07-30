//! 资源同步核心逻辑 — 从 resource_dir 的 .official/ 复制文件到 data/。
//!
//! 仅涉及本地文件操作，不走网络。

use std::collections::HashMap;
use std::path::Path;

use tracing::info;

use crate::manifest::DataManifest;

use super::ResourceSyncResult;

/// 首次全量播种：将 .official/game_data/* 复制到 data/game_data/，
/// 以及 .official/data_manifest.json 复制到 data/data_manifest.json。
pub fn seed_full_from_official(data_dir: &Path, official_dir: &Path) -> anyhow::Result<()> {
    let game_data_src = official_dir.join("game_data");
    let game_data_dst = data_dir.join("game_data");
    let manifest_src = official_dir.join("data_manifest.json");
    let manifest_dst = data_dir.join("data_manifest.json");

    // 1. 复制 game_data/
    if game_data_src.exists() {
        copy_dir_recursive(&game_data_src, &game_data_dst)?;
        info!(
            "Seeded game_data from {} to {}",
            game_data_src.display(),
            game_data_dst.display()
        );
    }

    // 2. 复制 manifest
    if manifest_src.exists() {
        std::fs::copy(&manifest_src, &manifest_dst)?;
        info!("Seeded manifest to {}", manifest_dst.display());
    }

    info!("Full seed from .official complete");
    Ok(())
}

/// 从 .official/game_data/ 复制用户选中的文件到 data/game_data/，
/// 更新本地 manifest，最后删除 .official/。
pub fn apply_selected_files(
    data_dir: &Path,
    selected_files: &[String],
) -> anyhow::Result<ResourceSyncResult> {
    let official_dir = data_dir.join(".official");
    let official_manifest_path = official_dir.join("data_manifest.json");

    let official_manifest = DataManifest::load(&official_manifest_path)
        .map_err(|e| anyhow::anyhow!("读取官方清单失败: {e}"))?;

    let local_manifest_path = data_dir.join("data_manifest.json");
    let mut local_manifest = if local_manifest_path.exists() {
        DataManifest::load(&local_manifest_path).unwrap_or(DataManifest {
            data_version: 0,
            files: HashMap::new(),
        })
    } else {
        DataManifest {
            data_version: 0,
            files: HashMap::new(),
        }
    };

    let mut synced = 0usize;

    for path in selected_files {
        // manifest 中的 path 已包含 "game_data/" 前缀，如 "game_data/backgrounds/白天.webp"
        // .official/ 的结构与之对应: .official/game_data/backgrounds/...
        let src = official_dir.join(path);
        let dst = data_dir.join(path);

        if !src.exists() {
            tracing::warn!("官方目录中缺少文件: {path}，跳过");
            continue;
        }

        // 原子写入：先写 .tmp，再 rename
        let tmp = data_dir.join(format!(".{}.tmp", path.replace(['/', '\\'], "_")));
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::copy(&src, &tmp)
            .map_err(|e| anyhow::anyhow!("复制 {} 失败: {}", path, e))?;
        std::fs::rename(&tmp, &dst)
            .map_err(|e| anyhow::anyhow!("原子写入 {} 失败: {}", path, e))?;

        // 更新本地 manifest 条目
        if let Some(entry) = official_manifest.files.get(path) {
            local_manifest
                .files
                .insert(path.clone(), entry.clone());
        }

        synced += 1;
    }

    // 更新清单版本号
    local_manifest.data_version = official_manifest.data_version;
    local_manifest.save(&local_manifest_path)?;

    // 清理 .official/
    std::fs::remove_dir_all(&official_dir)?;
    info!(
        "Synced {} files, .official removed, data_version={}",
        synced, local_manifest.data_version
    );

    Ok(ResourceSyncResult {
        success: true,
        files_synced: synced,
        message: format!("成功同步 {} 个文件", synced),
    })
}

// ─── 辅助函数 ────────────────────────────────────────────────

/// 递归复制目录内容。
fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
