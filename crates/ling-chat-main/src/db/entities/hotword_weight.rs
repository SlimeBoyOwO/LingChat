//! 热词 `weight` 的取值口径，三张热词表共用。
//!
//! 放在实体层是因为 `lan_sync/db_sync.rs` 的镜像导入要在写入前归一远端 payload 的
//! 脏值，而它只依赖实体；口径须与 `AsrProvider` 侧的 `Hotword`（`ai_service/asr/provider_types.rs`）一致。

pub const MIN_WEIGHT: i32 = 1;
pub const WEIGHT_CAP: i32 = 5;
/// 超级热词的特殊值（百炼口径：`[1, 5]` 为普通热词，`50` 为超级热词）。
pub const SUPER_WEIGHT: i32 = 50;
/// 一次请求内超级热词的条数上限，超出由发送层降级。
pub const SUPER_COUNT_LIMIT: usize = 50;

/// 把任意 `weight` 归一到 `[1, 5] ∪ {50}`：非正数抬到 1，`6..=49` 降为 5，`>= 50` 归为 50。
pub fn normalize_weight(weight: i32) -> i32 {
    if weight <= 0 {
        MIN_WEIGHT
    } else if weight <= WEIGHT_CAP {
        weight
    } else if weight < SUPER_WEIGHT {
        WEIGHT_CAP
    } else {
        SUPER_WEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 权重是离散取值，三个区段各自钉住边界。
    #[test]
    fn normalize_weight_is_discrete() {
        assert_eq!(normalize_weight(-7), MIN_WEIGHT);
        assert_eq!(normalize_weight(0), MIN_WEIGHT);
        // 普通档原样保留
        assert_eq!(normalize_weight(1), 1);
        assert_eq!(normalize_weight(5), WEIGHT_CAP);
        // 非法区降到普通档上界
        assert_eq!(normalize_weight(6), WEIGHT_CAP);
        assert_eq!(normalize_weight(49), WEIGHT_CAP);
        // 超级热词含越界存量
        assert_eq!(normalize_weight(SUPER_WEIGHT), SUPER_WEIGHT);
        assert_eq!(normalize_weight(999), SUPER_WEIGHT);
    }
}
