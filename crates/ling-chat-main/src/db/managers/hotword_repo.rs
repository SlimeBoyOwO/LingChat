use std::collections::HashMap;
use std::hash::Hash;

use anyhow::anyhow;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::db::entities::hotword_weight::normalize_weight;
use crate::db::entities::{global_hotword, hotword_category, library_hotword, role_hotword};

/// 每作用域条数上限（角色与全局同一口径）：几百条会撞服务端上限，且 llama 的 prompt 按字符预算整体截断。
const MAX_ROWS_PER_ROLE: usize = 200;
/// 单条词条的长度判据（**两套，按是否含非 ASCII 分流**，厂商文档口径）：
/// 含非 ASCII 时按**总字符数** ≤ [`MAX_TEXT_CHARS`]；纯 ASCII 时按**空格切分的片段数**
/// ≤ [`MAX_TEXT_SEGMENTS`]。
///
/// ⚠️ **两者不是同一个数的变体**：`DeepSeek-harness` 有 16 字符却合规（1 片段），
/// 而 `厄洛替尼盐酸盐` 只有 7 字符也走字符数分支。**不要压成一个上限。**
const MAX_TEXT_CHARS: usize = 15;
const MAX_TEXT_SEGMENTS: usize = 7;

/// 调用方侧的长度判据：先 `trim`、再判空、最后校验，校验的是 `trim` 后的值，
/// 未通过则返回原因（供调用方记日志并丢弃该行）。
///
/// 必须在调用方（而不是塞进 [`validate_text_len`]）做，因为 `text` 参与归并/唯一键
///（角色/全局按 `text`，词库按「分类 + text」），未 `trim` 就入库会让同一个词出现两个 key
/// （镜像路径尤其需要）。
pub(crate) fn validate_and_trim(text: &mut String) -> Result<(), anyhow::Error> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("热词为空"));
    }
    validate_text_len(trimmed)?;
    if trimmed.len() != text.len() {
        *text = trimmed.to_string();
    }
    Ok(())
}

/// 按是否含非 ASCII 分流校验单条词条长度。
///
/// `pub` 供 `lan_sync` 的镜像导入复用：那条路径原先只归一 `weight`、不做任何校验，
/// 于是「本地拒了、镜像放行」。两侧必须同一份判据。
///
/// ⚠️ **调用方须先 `trim` 并自行丢弃空串**（本仓用 [`validate_and_trim`] 包一层）：
/// 本函数只判长度，纯空白串在 ASCII 分支是 0 片段、会照常放行。
pub fn validate_text_len(text: &str) -> Result<(), anyhow::Error> {
    if text.is_ascii() {
        let segments = text.split_whitespace().count();
        if segments > MAX_TEXT_SEGMENTS {
            return Err(anyhow!(
                "热词超长：{segments} 个片段 > {MAX_TEXT_SEGMENTS}（纯 ASCII 按空格切分计）"
            ));
        }
        return Ok(());
    }
    let chars = text.chars().count();
    if chars > MAX_TEXT_CHARS {
        return Err(anyhow!("热词超长：{chars} 字符 > {MAX_TEXT_CHARS}"));
    }
    Ok(())
}

/// 词库条数上限：库不上行给 provider（不受服务端限制），上限只为有界
/// ——LAN payload 可控在几十 KB、UI 规模可预期。
pub const MAX_ROWS_PER_LIBRARY: usize = 2000;
/// 词库分类数上限。
pub const MAX_CATEGORIES: usize = 100;
/// 分类名字符数上限（按字符计）。
pub const MAX_CATEGORY_NAME_CHARS: usize = 30;

/// 归并内核的单行产出。`first_index` = 该行在**入参**中的首现下标
/// （词库侧据此取回 `category_index`；角色/全局侧用 `..` 忽略）。
pub struct MergedRow {
    pub first_index: usize,
    pub text: String,
    pub weight: i32,
}

/// 校验 + 归并内核：trim、丢空、查超限/超长，按 `key_of` 归并取 max、保留首现位置。
/// 角色 / 全局 / 词库三处共用，只差 `max_rows`（200 vs 2000）与归并键。
fn validate_and_merge_by<K, F>(
    rows: &[(String, i32)],
    max_rows: usize,
    key_of: F,
) -> Result<Vec<MergedRow>, anyhow::Error>
where
    K: Eq + Hash,
    F: Fn(usize, &str) -> K,
{
    if rows.len() > max_rows {
        return Err(anyhow!("热词条数超限：{} > {max_rows}", rows.len()));
    }

    let mut merged: Vec<MergedRow> = Vec::new();
    let mut positions: HashMap<K, usize> = HashMap::new();
    for (index, (text, weight)) in rows.iter().enumerate() {
        let text = text.trim();
        if text.is_empty() {
            continue; // 纯空白条目丢弃
        }
        validate_text_len(text)?;
        let weight = normalize_weight(*weight);
        // 归并键与唯一键同形：避免入参自撞唯一键让整个事务失败。
        let key = key_of(index, text);
        if let Some(&position) = positions.get(&key) {
            merged[position].weight = merged[position].weight.max(weight);
        } else {
            positions.insert(key, merged.len());
            merged.push(MergedRow {
                first_index: index,
                text: text.to_string(),
                weight,
            });
        }
    }
    Ok(merged)
}

/// 角色 / 全局的归并键：同一作用域内 `text` 唯一。
fn validate_and_merge(
    rows: &[(String, i32)],
    max_rows: usize,
) -> Result<Vec<MergedRow>, anyhow::Error> {
    validate_and_merge_by(rows, max_rows, |_, text| text.to_string())
}

pub struct HotwordRepo;

impl HotwordRepo {
    /// 按 id 升序 = 插入顺序（即场景热词的组内顺序）。
    pub async fn list_by_role<C: ConnectionTrait>(
        db: &C,
        role_id: i32,
    ) -> Result<Vec<role_hotword::Model>, anyhow::Error> {
        role_hotword::Entity::find()
            .filter(role_hotword::Column::RoleId.eq(role_id))
            .order_by_asc(role_hotword::Column::Id)
            .all(db)
            .await
            .map_err(|e| anyhow!(e))
    }

    /// 与 `list_by_role` 同一口径（按 id 升序）；空入参返回空。
    pub async fn list_by_roles<C: ConnectionTrait>(
        db: &C,
        role_ids: &[i32],
    ) -> Result<Vec<role_hotword::Model>, anyhow::Error> {
        if role_ids.is_empty() {
            return Ok(Vec::new());
        }
        role_hotword::Entity::find()
            .filter(role_hotword::Column::RoleId.is_in(role_ids.iter().copied()))
            .order_by_asc(role_hotword::Column::Id)
            .all(db)
            .await
            .map_err(|e| anyhow!(e))
    }

    /// 整表替换（delete + insert）。入参内按 trim 后的 `text` 归并取 max weight，
    /// 保留首次出现的位置。
    ///
    /// 全部校验通过后才动库：任一条不合格直接 `Err`，整表保持原样。
    /// 事务由调用方开启——在事务里必须传 `&txn`（SQLite 单连接池，传 `&db` 会自锁）。
    pub async fn replace_for_role<C: ConnectionTrait>(
        db: &C,
        role_id: i32,
        rows: &[(String, i32)],
    ) -> Result<(), anyhow::Error> {
        let merged = validate_and_merge(rows, MAX_ROWS_PER_ROLE)?;
        role_hotword::Entity::delete_many()
            .filter(role_hotword::Column::RoleId.eq(role_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        for MergedRow { text, weight, .. } in merged {
            role_hotword::ActiveModel {
                role_id: Set(role_id),
                text: Set(text),
                weight: Set(weight),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|e| anyhow!(e))?;
        }
        Ok(())
    }

    pub async fn replace_global<C: ConnectionTrait>(
        db: &C,
        rows: &[(String, i32)],
    ) -> Result<(), anyhow::Error> {
        let merged = validate_and_merge(rows, MAX_ROWS_PER_ROLE)?;
        global_hotword::Entity::delete_many()
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        for MergedRow { text, weight, .. } in merged {
            global_hotword::ActiveModel {
                text: Set(text),
                weight: Set(weight),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|e| anyhow!(e))?;
        }
        Ok(())
    }

    pub async fn list_global<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Vec<global_hotword::Model>, anyhow::Error> {
        global_hotword::Entity::find()
            .order_by_asc(global_hotword::Column::Id)
            .all(db)
            .await
            .map_err(|e| anyhow!(e))
    }

    /// 词库两张表，**皆按 id 升序 = 插入顺序**（整表替换按序插入，两者同序）。
    pub async fn list_library<C: ConnectionTrait>(
        db: &C,
    ) -> Result<(Vec<hotword_category::Model>, Vec<library_hotword::Model>), anyhow::Error> {
        let categories = hotword_category::Entity::find()
            .order_by_asc(hotword_category::Column::Id)
            .all(db)
            .await
            .map_err(|e| anyhow!(e))?;
        let words = library_hotword::Entity::find()
            .order_by_asc(library_hotword::Column::Id)
            .all(db)
            .await
            .map_err(|e| anyhow!(e))?;
        Ok((categories, words))
    }

    /// 词库整表替换（两表一起重建）。`categories` 的顺序 = 插入顺序；
    /// `words` 的 `category_index` 是 `categories` 的下标（None = 未分类）。
    ///
    /// 全部校验通过后才动库：任一条不合格直接 `Err`，整库保持原样。
    /// 事务由调用方开启，`&txn` 的原因见 `replace_for_role`。
    pub async fn replace_library<C: ConnectionTrait>(
        db: &C,
        categories: &[String],
        words: &[(Option<usize>, String, i32)],
    ) -> Result<(), anyhow::Error> {
        // ── 校验先行 ──────────────────────────────────────────
        if categories.len() > MAX_CATEGORIES {
            return Err(anyhow!(
                "分类数超限：{} > {MAX_CATEGORIES}",
                categories.len()
            ));
        }
        let mut names: Vec<String> = Vec::with_capacity(categories.len());
        for name in categories {
            let name = name.trim();
            if name.is_empty() {
                return Err(anyhow!("分类名不能为空"));
            }
            let chars = name.chars().count();
            if chars > MAX_CATEGORY_NAME_CHARS {
                return Err(anyhow!(
                    "分类名超长：{chars} 字符 > {MAX_CATEGORY_NAME_CHARS}"
                ));
            }
            // 与 DB 的 UNIQUE(name)（BINARY）同口径：trim 后精确比较
            if names.iter().any(|n| n == name) {
                return Err(anyhow!("分类名重复：{name}"));
            }
            names.push(name.to_string());
        }

        for (index, _, _) in words.iter() {
            if let Some(index) = index {
                if *index >= names.len() {
                    return Err(anyhow!("词条的分类下标越界：{index} >= {}", names.len()));
                }
            }
        }

        let projection: Vec<(String, i32)> = words
            .iter()
            .map(|(_, text, weight)| (text.clone(), *weight))
            .collect();
        // 词库按 (分类, text) 归并：同词可同时存在于不同分类，同分类内仍唯一。
        let merged = validate_and_merge_by(&projection, MAX_ROWS_PER_LIBRARY, |index, text| {
            (words[index].0, text.to_string())
        })?;

        // ── 写入 ──────────────────────────────────────────────
        library_hotword::Entity::delete_many()
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        hotword_category::Entity::delete_many()
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;

        // 按序插分类，收集新 id（id 由 AUTOINCREMENT 重分配，无外部引用者）
        let mut ids: Vec<i32> = Vec::with_capacity(names.len());
        for name in names {
            let inserted = hotword_category::ActiveModel {
                name: Set(name),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|e| anyhow!(e))?;
            ids.push(inserted.id);
        }

        // 归并后按首现下标取回原入参的分类；越界已在上面挡过
        for row in merged {
            let category_id = words[row.first_index].0.map(|index| ids[index]);
            library_hotword::ActiveModel {
                category_id: Set(category_id),
                text: Set(row.text),
                weight: Set(row.weight),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|e| anyhow!(e))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> (tempfile::TempDir, sea_orm::DatabaseConnection) {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let db = crate::db::init_db(dir.path())
            .await
            .expect("初始化数据库失败");
        (dir, db)
    }

    /// 同名热词分属不同分类时必须各存一行，不能被 `text` 归并掉。
    #[tokio::test]
    async fn library_keeps_same_text_in_different_categories() {
        let (_dir, db) = test_db().await;
        HotwordRepo::replace_library(
            &db,
            &["人物".to_string(), "产品".to_string()],
            &[
                (Some(0), "钦灵".to_string(), 4),
                (Some(1), "钦灵".to_string(), 5),
            ],
        )
        .await
        .expect("保存失败");

        let (categories, words) = HotwordRepo::list_library(&db).await.expect("读取失败");
        assert_eq!(words.len(), 2, "不同分类的同名词条应各存一行");
        assert_eq!(words[0].category_id, Some(categories[0].id));
        assert_eq!(words[1].category_id, Some(categories[1].id));
    }

    /// 同一分类内的同名词仍按原语义归并取最大权重。
    #[tokio::test]
    async fn library_merges_same_text_within_one_category() {
        let (_dir, db) = test_db().await;
        HotwordRepo::replace_library(
            &db,
            &["人物".to_string()],
            &[
                (Some(0), "钦灵".to_string(), 4),
                (Some(0), "钦灵".to_string(), 5),
            ],
        )
        .await
        .expect("保存失败");

        let (_, words) = HotwordRepo::list_library(&db).await.expect("读取失败");
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].weight, 5);
    }

    /// 未分类之间同样保持唯一；SQLite 的复合唯一索引对 NULL 不做约束，需单独保证。
    #[tokio::test]
    async fn library_merges_same_text_when_uncategorized() {
        let (_dir, db) = test_db().await;
        HotwordRepo::replace_library(
            &db,
            &[],
            &[(None, "通用".to_string(), 4), (None, "通用".to_string(), 5)],
        )
        .await
        .expect("保存失败");

        let (_, words) = HotwordRepo::list_library(&db).await.expect("读取失败");
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].weight, 5);
    }
}
