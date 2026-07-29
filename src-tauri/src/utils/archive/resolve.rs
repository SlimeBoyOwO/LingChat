//! 目标目录冲突解析：根据 [`ConflictPolicy`] 决定新建/重命名/覆盖。
//!
//! 提供单一函数 [`resolve_target`]；上层导入流水线在拿到压缩包顶层目录名后
//! 调用本模块返回最终落盘路径，配合 [`super::safety`] 的清洗规则使用。

use std::path::Path;

use super::{ArchiveError, ConflictPolicy, TargetResolution};


/// 根据冲突策略解析最终目标目录。
///
/// # 行为
/// - 不存在：直接返回 `action = "created"`
/// - [`ConflictPolicy::Skip`]：返回 `Err(AlreadyExists)`
/// - [`ConflictPolicy::Overwrite`]：返回 `action = "overwritten"`
/// - [`ConflictPolicy::Rename`]：依次尝试 `_2` ... `_999` 后缀；全占用时用 Unix 毫秒时间戳
///
/// # 参数
/// - `base`：目标根目录（通常是 `data/game_data/characters/`）
/// - `preferred`：首选目录名（压缩包顶层目录名）
/// - `policy`：冲突策略
pub fn resolve_target(
    base: &Path,
    preferred: &str,
    policy: ConflictPolicy,
) -> Result<TargetResolution, ArchiveError> {
    let target = base.join(preferred);
    if !target.exists() {
        return Ok(TargetResolution {
            target,
            final_name: preferred.into(),
            action: "created",
        });
    }
    match policy {
        ConflictPolicy::Skip => Err(ArchiveError::AlreadyExists(preferred.into())),
        ConflictPolicy::Overwrite => Ok(TargetResolution {
            target,
            final_name: preferred.into(),
            action: "overwritten",
        }),
        ConflictPolicy::Rename => {
            for n in 2..=999 {
                let name = format!("{preferred}_{n}");
                let candidate = base.join(&name);
                if !candidate.exists() {
                    return Ok(TargetResolution {
                        target: candidate,
                        final_name: name,
                        action: "renamed",
                    });
                }
            }
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let name = format!("{preferred}_{ts}");
            Ok(TargetResolution {
                target: base.join(&name),
                final_name: name,
                action: "renamed",
            })
        }
    }
}