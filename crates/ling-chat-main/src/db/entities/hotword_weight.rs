//! 热词 `weight` 的取值口径。
//!
//! **为什么放实体层而不是仓储层**：`lan_sync/db_sync.rs` 的镜像导入要在写入前归一
//! 对端 payload 的脏值，而它已经 import 了实体、不认识仓储。放这里，同步层不必反向
//! 依赖 `managers::hotword_repo`（上层依赖下层）。
//!
//! 三张带 `weight` 的热词表共用，口径必须与 `AsrProvider` 侧的
//! `Hotword`（`ai_service/asr/provider_types.rs`）一致。
//!
//! ⚠️ **权重不是连续区间**（百炼《提升识别准确率》）：取值只允许 `[1, 5]` 或 `50`。
//! `1..=5` 为普通热词、值越大偏好越强；`50` 为超级热词（召回率大幅提升，
//! **最多不超过 50 个**）。`6..=49` 是**无效值**，一律降为 5。
//!
//! DDL 的 `CHECK (weight BETWEEN 1 AND 50)` 是这条口径的**超集**（历史包袱，
//! 见 `m20261004`）——它只兜底，不表达离散性；离散性由本文件的函数表达。

pub const MIN_WEIGHT: i32 = 1;
pub const WEIGHT_CAP: i32 = 5;
/// 超级热词的特殊值（也是 DDL `CHECK (weight BETWEEN 1 AND 50)` 的上界）。
pub const SUPER_WEIGHT: i32 = 50;
/// 一次请求内超级热词（`weight == SUPER_WEIGHT`）的条数上限。
///
/// 超过这个数服务端行为未知，故由发送层（`api/asr.rs::demote_excess_super`）降级处理。
pub const SUPER_COUNT_LIMIT: usize = 50;

/// 把任意 `weight` 归一到 `[1, 5] ∪ {50}`（保存、归档导入、LAN 镜像、读侧共用同一口径）。
///
/// `6..=49` 取**就近的合法值 5**：它语义上是「比普通最强还强」，降为普通档上界；
/// `>= 50` 归为超级热词，与旧的「999 → 50」行为一致。
/// 归一结果恒落在 DDL 的 `CHECK (weight BETWEEN 1 AND 50)` 内。
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

    /// 权重是离散口径而非连续区间，三个 band 各自钉住边界。
    ///
    /// 这条是**本地保存路径本来就没有的覆盖**：LAN 镜像测试原先顺带断言过
    /// 「999 → 50」，但它们已在收尾时删除，若不补这条，整个仓库就没有任何测试
    /// 约束这个函数——把 `>= 50` 分支改坏不会有测试变红。
    #[test]
    fn normalize_weight_is_discrete() {
        // 下界：非正数一律抬到 1
        assert_eq!(normalize_weight(-7), MIN_WEIGHT);
        assert_eq!(normalize_weight(0), MIN_WEIGHT);
        // 普通档：原样保留
        assert_eq!(normalize_weight(1), 1);
        assert_eq!(normalize_weight(5), WEIGHT_CAP);
        // 非法区：6..=49 就近降到普通档上界（不是抬到 50）
        assert_eq!(normalize_weight(6), WEIGHT_CAP);
        assert_eq!(normalize_weight(49), WEIGHT_CAP);
        // 超级热词：>= 50 一律归 50（含越界存量 999）
        assert_eq!(normalize_weight(SUPER_WEIGHT), SUPER_WEIGHT);
        assert_eq!(normalize_weight(999), SUPER_WEIGHT);
    }
}
