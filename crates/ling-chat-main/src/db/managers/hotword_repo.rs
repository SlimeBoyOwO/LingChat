use anyhow::anyhow;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};

use crate::db::entities::hotword_weight::normalize_weight;
use crate::db::entities::{global_hotword, hotword_category, library_hotword, role_hotword};

/// 每作用域条数上限（角色与全局同一口径）。
const MAX_ROWS_PER_ROLE: usize = 200;
/// 含非 ASCII 时按总字符数计，纯 ASCII 时按空格切分的片段数计；两者不是同一个上限。
const MAX_TEXT_CHARS: usize = 15;
const MAX_TEXT_SEGMENTS: usize = 7;

/// `trim` 后判空并校验长度，校验的是 `trim` 后的值。
///
/// `text` 是归并/唯一键，未 `trim` 就入库会让同一个词占两个 key。
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

/// 按是否含非 ASCII 分流校验单条词条长度，`lan_sync` 镜像导入复用同一判据。
///
/// 调用方须先 `trim` 并丢弃空串：本函数只判长度，纯空白串会放行。
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

/// 词库条数上限：词库不上行给 provider，上限只为让 LAN payload 和 UI 规模有界。
pub const MAX_ROWS_PER_LIBRARY: usize = 2000;
/// 词库分类数上限。
pub const MAX_CATEGORIES: usize = 100;
/// 分类名字符数上限（按字符计）。
pub const MAX_CATEGORY_NAME_CHARS: usize = 30;

/// 归并内核的单行产出；`first_index` 是该行在入参中的首现下标
/// （词库侧据此取回 `category_index`，角色/全局侧忽略）。
pub struct MergedRow {
    pub first_index: usize,
    pub text: String,
    pub weight: i32,
}

/// 校验并归并：trim、丢空、查超限/超长，按 `text` 归并取 max、保留首现位置。
/// 角色 / 全局 / 词库三处共用，只差 `max_rows`。归并键与发送层 `build_options` 同为 `text`。
fn validate_and_merge(
    rows: &[(String, i32)],
    max_rows: usize,
) -> Result<Vec<MergedRow>, anyhow::Error> {
    if rows.len() > max_rows {
        return Err(anyhow!("热词条数超限：{} > {max_rows}", rows.len()));
    }

    let mut merged: Vec<MergedRow> = Vec::new();
    for (index, (text, weight)) in rows.iter().enumerate() {
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        validate_text_len(text)?;
        let weight = normalize_weight(*weight);
        // 归并键与唯一键同形，避免入参自撞唯一键让事务失败。
        match merged.iter_mut().find(|r| r.text == text) {
            Some(exist) => exist.weight = exist.weight.max(weight),
            None => merged.push(MergedRow {
                first_index: index,
                text: text.to_string(),
                weight,
            }),
        }
    }
    Ok(merged)
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

    /// 整表替换（delete + insert），入参按 trim 后的 `text` 归并取 max weight。
    /// 全部校验通过后才动库。事务由调用方开启，有事务时必须传 `&txn`
    /// （SQLite 单连接池，传 `&db` 会自锁）。
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

    /// 词库两张表，皆按 id 升序 = 插入顺序（整表替换按序插入）。
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

    /// 词库整表替换（两表一起重建）。`categories` 的顺序 = 插入顺序，
    /// `words` 的 `category_index` 是 `categories` 的下标（None = 未分类）。
    /// 全部校验通过后才动库；事务由调用方开启，`&txn` 的原因见 `replace_for_role`。
    pub async fn replace_library<C: ConnectionTrait>(
        db: &C,
        categories: &[String],
        words: &[(Option<usize>, String, i32)],
    ) -> Result<(), anyhow::Error> {
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
        let merged = validate_and_merge(&projection, MAX_ROWS_PER_LIBRARY)?;

        library_hotword::Entity::delete_many()
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;
        hotword_category::Entity::delete_many()
            .exec(db)
            .await
            .map_err(|e| anyhow!(e))?;

        // 按序插分类并收集新 id（AUTOINCREMENT 重分配，无外部引用者）
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

        // 归并后按首现下标取回原入参的分类（越界已在上方校验）
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
