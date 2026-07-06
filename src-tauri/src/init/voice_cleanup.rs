//! 启动时自动清理：删除未被任何台词引用的孤立语音文件。

use std::collections::HashSet;

use anyhow::{anyhow, Result};
use sea_orm::*;

use crate::db::entities::line;

/// 清理统计信息。
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    pub deleted_count: u64,
}

/// 查询数据库中所有被引用的语音文件名，删除 voice/ 目录下未被引用的文件。
pub async fn cleanup_orphan_voice_files(db: &DatabaseConnection) -> Result<CleanupStats> {
    // 1. 查询 line 表中所有非空 audio_file 值
    let referenced: HashSet<String> = line::Entity::find()
        .select_only()
        .column(line::Column::AudioFile)
        .filter(line::Column::AudioFile.is_not_null())
        .into_tuple::<Option<String>>()
        .all(db)
        .await
        .map_err(|e| anyhow!("查询语音文件引用失败: {e}"))?
        .into_iter()
        .filter_map(|x| x)
        .collect();

    tracing::info!("数据库中引用了 {} 个语音文件", referenced.len());

    // 2. 检查 voice/ 目录是否存在（首次运行可能还没有）
    let voice_dir = crate::api::voice_dir();
    if !voice_dir.exists() {
        tracing::info!("语音目录不存在，跳过清理");
        return Ok(CleanupStats::default());
    }

    // 3. 遍历 voice/ 目录，删除不在引用集合中的文件
    let mut read_dir = tokio::fs::read_dir(&voice_dir)
        .await
        .map_err(|e| anyhow!("无法读取语音目录 {:?}: {e}", voice_dir))?;
    let mut deleted_count: u64 = 0;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| anyhow!("遍历语音目录时出错: {e}"))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|e| anyhow!("获取文件类型失败: {e}"))?;

        // 跳过非普通文件（目录、符号链接等）
        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy().to_string();

        if !referenced.contains(&file_name_str) {
            tokio::fs::remove_file(entry.path())
                .await
                .map_err(|e| anyhow!("删除孤立语音文件 {:?} 失败: {e}", entry.path()))?;
            deleted_count += 1;
            tracing::debug!("已删除孤立语音文件: {}", file_name_str);
        }
    }

    if deleted_count > 0 {
        tracing::info!("已清理 {} 个孤立语音文件", deleted_count);
    } else {
        tracing::info!("没有发现孤立语音文件");
    }

    Ok(CleanupStats { deleted_count })
}
