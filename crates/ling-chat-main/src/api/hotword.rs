//! 角色热词命令：读取（单角色 / 场景级 / 全局 / 词库）与保存（热词单事务）。

use std::collections::HashMap;

use sea_orm::{DatabaseConnection, TransactionTrait};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::asr::provider::Hotword;
use crate::api::asr::HotwordInput;
use crate::db::entities::{global_hotword, role_hotword};
use crate::db::managers::hotword_repo::HotwordRepo;

/// 热词的对外表示；比 provider 侧的 `Hotword` 多一个 `role_id`（DB 概念）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleHotword {
    pub text: String,
    pub weight: u8,
    /// 供 UI 归因展示。
    pub role_id: i32,
}

impl RoleHotword {
    /// weight 走实体层的离散归一（`[1, 5] ∪ {50}`）。
    /// 读侧也要归一：DDL 的 `CHECK` 只兜底写入，LAN 镜像会送来旧的非法值，
    /// 直接 `as u8` 还会静默回绕。
    fn from_parts(text: String, weight: i32, role_id: i32) -> Self {
        Self {
            text,
            weight: crate::db::entities::hotword_weight::normalize_weight(weight) as u8,
            role_id,
        }
    }

    fn from_model(row: role_hotword::Model) -> Self {
        Self::from_parts(row.text, row.weight, row.role_id)
    }

    /// 全局热词共用同一对外结构，`role_id` 恒为 0（占位：不入库、不上行）。
    fn from_global(row: global_hotword::Model) -> Self {
        Self::from_parts(row.text, row.weight, 0)
    }
}

/// 词库词条，读写共用。无 id：没有任何东西引用词库行 id。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryWord {
    /// `None` = 未分类。
    pub category_index: Option<usize>,
    pub text: String,
    /// 1..=5 或 50（读侧经离散归一后转入）。
    pub weight: u8,
}

/// 词库的读写形态：分类名数组（顺序 = 插入顺序）+ 词条数组。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotwordLibrary {
    pub categories: Vec<String>,
    pub words: Vec<LibraryWord>,
}

/// 场景级热词：主角 → present 顺序 → 组内 id 顺序。
///
/// `-1`（未选主角的哨兵）与 `0`（User）不是热词归属角色，会被滤掉；
/// 主角已在最前，不重复追加。
async fn scene_hotwords(
    db: &DatabaseConnection,
    main_role_id: Option<i32>,
    present_role_ids: &[i32],
) -> Result<Vec<RoleHotword>, anyhow::Error> {
    let mut role_ids: Vec<i32> = Vec::with_capacity(present_role_ids.len() + 1);
    if let Some(main) = main_role_id.filter(|id| *id > 0) {
        role_ids.push(main);
    }
    for &id in present_role_ids {
        if id > 0 && !role_ids.contains(&id) {
            role_ids.push(id);
        }
    }

    // IN 查询的返回顺序未定义，按 role_ids 重新分组。
    let mut by_role: HashMap<i32, Vec<role_hotword::Model>> = HashMap::new();
    for row in HotwordRepo::list_by_roles(db, &role_ids).await? {
        by_role.entry(row.role_id).or_default().push(row);
    }

    let mut out = Vec::new();
    for role_id in role_ids {
        let Some(mut rows) = by_role.remove(&role_id) else {
            continue;
        };
        rows.sort_by_key(|row| row.id);
        out.extend(rows.into_iter().map(RoleHotword::from_model));
    }
    Ok(out)
}

/// 单个角色的热词（热词设置页读取用）。
#[tauri::command]
pub async fn get_role_hotwords(app: AppHandle, role_id: i32) -> Result<Vec<RoleHotword>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    let rows = HotwordRepo::list_by_role(db, role_id)
        .await
        .map_err(|e| format!("查询热词失败: {e}"))?;
    Ok(rows.into_iter().map(RoleHotword::from_model).collect())
}

/// 场景级热词：一次往返取回主角 ∪ 在场角色的全部热词。
#[tauri::command]
pub async fn get_scene_hotwords(
    app: AppHandle,
    main_role_id: Option<i32>,
    present_role_ids: Vec<i32>,
) -> Result<Vec<RoleHotword>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    scene_hotwords(db, main_role_id, &present_role_ids)
        .await
        .map_err(|e| format!("查询场景热词失败: {e}"))
}

/// 命令层入参转存储二元组 `(text, weight)`，weight 缺省 4。
/// 不在此校验，存储与发送两条链路各自归一。
fn to_rows(hotwords: Vec<HotwordInput>) -> Vec<(String, i32)> {
    hotwords
        .into_iter()
        .map(|h| match h {
            HotwordInput::Text(text) => (text, i32::from(Hotword::DEFAULT_WEIGHT)),
            HotwordInput::Weighted { text, weight } => {
                (text, i32::from(weight.unwrap_or(Hotword::DEFAULT_WEIGHT)))
            },
        })
        .collect()
}

/// 保存内核：单事务包 `replace_for_role`，校验失败整体回滚。
async fn save_settings(
    db: &DatabaseConnection,
    role_id: i32,
    hotwords: Vec<HotwordInput>,
) -> Result<(), anyhow::Error> {
    let txn = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    HotwordRepo::replace_for_role(&txn, role_id, &to_rows(hotwords)).await?;
    txn.commit().await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// 保存角色的热词，单事务提交。
#[tauri::command]
pub async fn save_role_hotwords(
    app: AppHandle,
    role_id: i32,
    hotwords: Vec<HotwordInput>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    save_settings(db, role_id, hotwords)
        .await
        .map_err(|e| format!("保存角色热词失败: {e}"))
}

// 全局热词（与场景角色热词取并集参与识别）

/// 全局热词。绝不写进 `role_hotword`——那里的 `role_id = 0` 是 User 的真实数据。
async fn global_hotwords(db: &DatabaseConnection) -> Result<Vec<RoleHotword>, anyhow::Error> {
    let rows = HotwordRepo::list_global(db).await?;
    Ok(rows.into_iter().map(RoleHotword::from_global).collect())
}

async fn save_global(
    db: &DatabaseConnection,
    hotwords: Vec<HotwordInput>,
) -> Result<(), anyhow::Error> {
    let txn = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    HotwordRepo::replace_global(&txn, &to_rows(hotwords)).await?;
    txn.commit().await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_global_hotwords(app: AppHandle) -> Result<Vec<RoleHotword>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    global_hotwords(db)
        .await
        .map_err(|e| format!("查询全局热词失败: {e}"))
}

#[tauri::command]
pub async fn save_global_hotwords(
    app: AppHandle,
    hotwords: Vec<HotwordInput>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    save_global(db, hotwords)
        .await
        .map_err(|e| format!("保存全局热词失败: {e}"))
}

// 词库（素材库，不参与识别）

/// 把两张表拼成读写形态，`category_id` 转分类数组下标（悬空归 `None`）。
async fn library(db: &DatabaseConnection) -> Result<HotwordLibrary, anyhow::Error> {
    let (categories, words) = HotwordRepo::list_library(db).await?;

    // id 由 AUTOINCREMENT 分配，不保证从 1 连续，必须查表而非减一。
    let index_of: HashMap<i32, usize> = categories
        .iter()
        .enumerate()
        .map(|(index, row)| (row.id, index))
        .collect();

    Ok(HotwordLibrary {
        categories: categories.into_iter().map(|row| row.name).collect(),
        words: words
            .into_iter()
            .map(|row| LibraryWord {
                // 悬空 category_id（防御旧设备或手改 payload）归未分类
                category_index: row.category_id.and_then(|id| index_of.get(&id).copied()),
                text: row.text,
                weight: crate::db::entities::hotword_weight::normalize_weight(row.weight) as u8,
            })
            .collect(),
    })
}

async fn save_library(
    db: &DatabaseConnection,
    categories: Vec<String>,
    words: Vec<LibraryWord>,
) -> Result<(), anyhow::Error> {
    let rows: Vec<(Option<usize>, String, i32)> = words
        .into_iter()
        .map(|w| (w.category_index, w.text, i32::from(w.weight)))
        .collect();

    let txn = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    HotwordRepo::replace_library(&txn, &categories, &rows).await?;
    txn.commit().await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

/// 读取整个词库。不做缓存：数据量小，识别链路也不读它。
#[tauri::command]
pub async fn get_hotword_library(app: AppHandle) -> Result<HotwordLibrary, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    library(db)
        .await
        .map_err(|e| format!("读取热词库失败: {e}"))
}

/// 保存整个词库（整表替换，两表一个事务，失败即整体回滚）。
#[tauri::command]
pub async fn save_hotword_library(
    app: AppHandle,
    categories: Vec<String>,
    words: Vec<LibraryWord>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    save_library(db, categories, words)
        .await
        .map_err(|e| format!("保存热词库失败: {e}"))
}
