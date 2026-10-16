//! 存档边界：一次捕获会话快照，在释放游戏状态锁后原子提交。
use anyhow::{Context, Result};
use sea_orm::{DatabaseConnection, TransactionTrait};
use tokio::sync::Mutex;

use super::auto_save::hash_of_real_lines;
use super::game_status::{GameStatus, GameStatusSnapshot};
use crate::ai_service::service::SharedAIService;
use crate::ai_service::types::{GameLine, GameMemoryBank};
use crate::db::managers::{memory_repo::MemoryRepo, save_repo::SaveRepo};

pub enum SaveTarget {
    Create(String),
    Update(i32),
    Auto { prefix: String, title: String },
}

struct ScriptSnapshot {
    folder: String,
    vars: serde_json::Value,
    chapter: String,
    event: i32,
}

struct SaveSnapshot {
    session_epoch: u64,
    preview_generation: u64,
    lines: Vec<GameLine>,
    main_role_id: Option<i32>,
    status: GameStatusSnapshot,
    memories: Vec<(i32, GameMemoryBank)>,
    script: Option<ScriptSnapshot>,
}

impl SaveSnapshot {
    fn capture(gs: &mut GameStatus) -> Result<Self> {
        let memories = gs.role_manager.snapshot_memory_banks();
        let script = gs.script_status.as_ref().map(|s| {
            Ok::<_, anyhow::Error>(ScriptSnapshot {
                folder: s.folder_key.clone(),
                vars: serde_json::to_value(&s.vars)?,
                chapter: s.current_chapter_key.clone(),
                event: s.current_event_process,
            })
        }).transpose()?;
        Ok(Self {
            session_epoch: gs.session_epoch,
            preview_generation: gs.preview_generation,
            lines: gs.line_list.clone(),
            main_role_id: gs.main_role_id,
            status: gs.to_snapshot(),
            memories,
            script,
        })
    }

    fn belongs_to(&self, session_epoch: u64, preview_generation: u64) -> bool {
        self.session_epoch == session_epoch && self.preview_generation == preview_generation
    }

    async fn persist(&self, db: &DatabaseConnection, target: SaveTarget) -> Result<i32> {
        let status = serde_json::to_string(&self.status)?;
        let memories = self.memories.iter().map(|(id, bank)| {
            Ok::<_, anyhow::Error>((*id, serde_json::to_string(bank)?))
        }).collect::<Result<Vec<_>>>()?;
        let txn = db.begin().await?;
        let save_id = match target {
            SaveTarget::Create(title) => SaveRepo::create_save(&txn, &title).await?.id,
            SaveTarget::Update(id) => {
                SaveRepo::get_save_by_id(&txn, id).await?.context("存档不存在")?;
                id
            },
            SaveTarget::Auto { prefix, title } => {
                if let Some(save) = SaveRepo::find_save_by_title_prefix(&txn, &prefix).await? {
                    SaveRepo::update_save_title(&txn, save.id, &title).await?;
                    save.id
                } else {
                    SaveRepo::create_save(&txn, &title).await?.id
                }
            },
        };
        SaveRepo::sync_lines_in_transaction(&txn, save_id, &self.lines).await?;
        SaveRepo::update_save_main_role(&txn, save_id, self.main_role_id).await?;
        SaveRepo::update_save_status(&txn, save_id, &status).await?;
        // 保留未加载角色的记忆；角色是惰性加载的，不能删除整个存档的记忆库。
        for (role_id, json) in memories {
            MemoryRepo::upsert_memory(&txn, save_id, role_id, &json, None).await?;
        }
        if let Some(script) = &self.script {
            SaveRepo::upsert_running_script(&txn, save_id, &script.folder,
                &serde_json::to_string(&script.vars)?, &script.chapter, script.event).await?;
        } else {
            SaveRepo::update_save_running_script(&txn, save_id, None).await?;
        }
        txn.commit().await?;
        Ok(save_id)
    }
}

pub struct SaveOutcome {
    pub save_id: i32,
    pub baseline_hash: Option<u64>,
}

/// 锁顺序：保存协调锁 → AIService → GameStatus。
/// 数据库阶段只持有协调锁，不占用 AIService / GameStatus。
#[derive(Default)]
pub struct SaveService {
    pub(crate) gate: Mutex<()>,
}

impl SaveService {
    pub async fn save(
        service: &SharedAIService,
        target: SaveTarget,
        previous_hash: Option<u64>,
    ) -> Result<Option<SaveOutcome>> {
        let coordinator = service.lock().await.save_service.clone();
        let _guard = coordinator.gate.lock().await;
        let (db, game_status, snapshot) = {
            // 同时持有外层锁，避免在读档流程的中间状态捕获快照。
            let svc = service.lock().await;
            let mut gs = svc.game_status.lock().await;
            (svc.db.clone(), svc.game_status.clone(), SaveSnapshot::capture(&mut gs)?)
        };
        let baseline_hash = hash_of_real_lines(&snapshot.lines);
        // 保留现有自动保存门槛与基线语义。
        if matches!(&target, SaveTarget::Auto { .. })
            && (baseline_hash.is_none() || baseline_hash == previous_hash)
        {
            return Ok(None);
        }
        let save_id = snapshot.persist(&db, target).await?;
        let mut gs = game_status.lock().await;
        if snapshot.belongs_to(gs.session_epoch, gs.preview_generation) {
            gs.active_save_id = Some(save_id);
        }
        Ok(Some(SaveOutcome { save_id, baseline_hash }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::LineBase;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
    use sea_orm_migration::MigratorTrait;

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        crate::migration::Migrator::up(&db, None).await.unwrap();
        db.execute(Statement::from_string(DbBackend::Sqlite,
            "INSERT INTO role (id, name, role_type) VALUES (1, 'test', 'npc')".to_string()
        )).await.unwrap();
        db
    }

    fn snapshot(text: &str) -> SaveSnapshot {
        SaveSnapshot {
            session_epoch: 1,
            preview_generation: 0,
            lines: vec![GameLine::from_base(LineBase {
                content: text.to_string(),
                sender_role_id: Some(1),
                ..Default::default()
            }, vec![1])],
            main_role_id: Some(1),
            status: GameStatusSnapshot {
                background: text.to_string(),
                ..Default::default()
            },
            memories: vec![(1, GameMemoryBank::default())],
            script: Some(ScriptSnapshot {
                folder: "test".into(),
                vars: serde_json::json!({"checkpoint": text}),
                chapter: text.into(),
                event: 1,
            }),
        }
    }

    async fn fail_script_writes(db: &DatabaseConnection) {
        for operation in ["INSERT", "UPDATE"] {
            db.execute(Statement::from_string(DbBackend::Sqlite, format!(
                "CREATE TRIGGER fail_script_{operation} BEFORE {operation} ON running_script \
                 BEGIN SELECT RAISE(ABORT, 'injected script failure'); END"
            ))).await.unwrap();
        }
    }

    #[test]
    fn completion_rejects_replaced_or_preview_sessions() {
        let saved = snapshot("old");
        assert!(saved.belongs_to(1, 0));
        assert!(!saved.belongs_to(2, 0));
        assert!(!saved.belongs_to(1, 1));
    }

    #[tokio::test]
    async fn failed_late_write_preserves_entire_previous_save() {
        let db = database().await;
        let id = snapshot("old").persist(&db, SaveTarget::Create("slot".into())).await.unwrap();
        let before = SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap();
        let memory_before = SaveRepo::get_memory_banks(&db, id).await.unwrap();
        let script_before = SaveRepo::get_running_script(&db, before.running_script_id.unwrap())
            .await.unwrap().unwrap();
        fail_script_writes(&db).await;
        let mut changed = snapshot("new");
        changed.memories[0].1.data.long_term = "new memory".into();
        assert!(changed.persist(&db, SaveTarget::Update(id)).await.is_err());
        let after = SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap();
        assert_eq!(before, after);
        assert_eq!(SaveRepo::get_line_list(&db, id).await.unwrap()[0].content, "old");
        assert_eq!(memory_before, SaveRepo::get_memory_banks(&db, id).await.unwrap());
        assert_eq!(script_before, SaveRepo::get_running_script(&db, after.running_script_id.unwrap())
            .await.unwrap().unwrap());
    }

    #[tokio::test]
    async fn failed_creation_leaves_no_partial_slot() {
        let db = database().await;
        fail_script_writes(&db).await;
        assert!(snapshot("new").persist(&db, SaveTarget::Create("slot".into())).await.is_err());
        assert_eq!(SaveRepo::count_saves(&db).await.unwrap(), 0);
        for table in ["line", "line_perception", "memory_bank", "running_script"] {
            let row = db.query_one(Statement::from_string(DbBackend::Sqlite,
                format!("SELECT COUNT(*) AS n FROM {table}")
            )).await.unwrap().unwrap();
            assert_eq!(row.try_get::<i64>("", "n").unwrap(), 0, "{table}");
        }
    }

    #[tokio::test]
    async fn overwrite_clears_script_link_and_preserves_unloaded_memories() {
        let db = database().await;
        let id = snapshot("old").persist(&db, SaveTarget::Create("slot".into())).await.unwrap();
        let mut changed = snapshot("new");
        changed.script = None;
        changed.memories.clear();
        let banks = SaveRepo::get_memory_banks(&db, id).await.unwrap();
        changed.persist(&db, SaveTarget::Update(id)).await.unwrap();
        let saved = SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap();
        assert!(saved.running_script_id.is_none());
        assert_eq!(serde_json::from_str::<GameStatusSnapshot>(&saved.status).unwrap().background, "new");
        assert_eq!(SaveRepo::get_line_list(&db, id).await.unwrap()[0].content, "new");
        assert_eq!(banks, SaveRepo::get_memory_banks(&db, id).await.unwrap());
    }
}
