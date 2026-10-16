use anyhow::{Context, Result};
use sea_orm::{DatabaseConnection, TransactionTrait};
use tokio::sync::Mutex;

use super::auto_save::hash_of_real_lines;
use super::game_status::{GameStatus, GameStatusSnapshot};
use crate::ai_service::service::SharedAIService;
use crate::ai_service::types::{GameLine, GameMemoryBank, ScriptStatus};
use crate::db::managers::{memory_repo::MemoryRepo, save_repo::SaveRepo};

pub enum SaveTarget {
    Create(String),
    Update(i32),
    Auto { prefix: String, title: String },
}

#[derive(Default)]
pub struct PlayerAnchor {
    pub chapter: Option<String>,
    pub event_index: Option<i32>,
    pub line_count: Option<i32>,
    pub vars_json: Option<String>,
}

struct ScriptSnapshot {
    folder: String,
    vars: serde_json::Map<String, serde_json::Value>,
    chapter: String,
    event: i32,
}

impl ScriptSnapshot {
    fn capture(script: &ScriptStatus, anchor: &PlayerAnchor) -> Self {
        let parsed = match (&anchor.chapter, anchor.event_index, anchor.line_count, &anchor.vars_json) {
            (Some(chapter), Some(event), Some(count), Some(json)) => {
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json)
                    .ok().map(|mut vars| {
                        vars.insert("__line_count".into(), serde_json::json!(count));
                        (chapter.clone(), event, vars)
                    })
            },
            _ => None,
        };
        let (chapter, event, vars) = parsed.unwrap_or_else(|| (
            script.current_chapter_key.clone(), script.current_event_process, script.vars.clone(),
        ));
        Self { folder: script.path_key().replace('\\', "/"), vars, chapter, event }
    }
}

struct SaveSnapshot {
    generation: u64,
    lines: Vec<GameLine>,
    main_role_id: Option<i32>,
    status: GameStatusSnapshot,
    memories: Vec<(i32, GameMemoryBank)>,
    script: Option<ScriptSnapshot>,
}

impl SaveSnapshot {
    fn capture(gs: &mut GameStatus, anchor: &PlayerAnchor) -> Self {
        let memories = gs.role_manager.snapshot_memory_banks();
        Self {
            generation: gs.preview_generation,
            lines: gs.line_list.clone(),
            main_role_id: gs.main_role_id,
            status: gs.to_snapshot(),
            memories,
            script: gs.script_status.as_ref().map(|s| ScriptSnapshot::capture(s, anchor)),
        }
    }

    async fn persist(&self, db: &DatabaseConnection, target: SaveTarget) -> Result<i32> {
        let status = serde_json::to_string(&self.status)?;
        let memories = self.memories.iter().map(|(id, bank)| {
            Ok::<_, anyhow::Error>((*id, serde_json::to_string(bank)?))
        }).collect::<Result<Vec<_>>>()?;
        let allow_rebuild = matches!(&target, SaveTarget::Auto { .. });
        let txn = db.begin().await?;
        let id = match target {
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
        SaveRepo::sync_lines_in_transaction(&txn, id, &self.lines, allow_rebuild).await?;
        SaveRepo::update_save_main_role(&txn, id, self.main_role_id).await?;
        SaveRepo::update_save_status(&txn, id, &status).await?;
        // 角色惰性加载，保留本次快照未包含的角色记忆。
        for (role, json) in memories {
            MemoryRepo::upsert_memory(&txn, id, role, &json, None).await?;
        }
        if let Some(script) = &self.script {
            SaveRepo::upsert_running_script(&txn, id, &script.folder,
                &serde_json::to_string(&script.vars)?, &script.chapter, script.event).await?;
        } else {
            SaveRepo::clear_running_script(&txn, id).await?;
        }
        txn.commit().await?;
        Ok(id)
    }
}

pub struct SaveOutcome {
    pub save_id: i32,
    pub baseline_hash: Option<u64>,
}

#[derive(Default)]
pub struct SaveService {
    pub(crate) gate: Mutex<()>,
}

impl SaveService {
    pub async fn save(
        service: &SharedAIService,
        target: SaveTarget,
        previous_hash: Option<u64>,
        anchor: PlayerAnchor,
    ) -> Result<Option<SaveOutcome>> {
        let coordinator = service.lock().await.save_service.clone();
        let _guard = coordinator.gate.lock().await;
        // 锁顺序为保存协调锁、AIService、GameStatus，数据库阶段仅保留协调锁。
        let (db, game_status, snapshot) = {
            let svc = service.lock().await;
            let mut gs = svc.game_status.lock().await;
            (svc.db.clone(), svc.game_status.clone(), SaveSnapshot::capture(&mut gs, &anchor))
        };
        let baseline_hash = hash_of_real_lines(&snapshot.lines);
        if matches!(&target, SaveTarget::Auto { .. })
            && (baseline_hash.is_none() || baseline_hash == previous_hash)
        {
            return Ok(None);
        }
        let save_id = snapshot.persist(&db, target).await?;
        let mut gs = game_status.lock().await;
        // dev 已在切角色、读档、清空和试玩边界递增此代号。
        if gs.preview_generation == snapshot.generation {
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
            generation: 1,
            lines: vec![GameLine::from_base(LineBase {
                content: text.into(), sender_role_id: Some(1), ..Default::default()
            }, vec![1])],
            main_role_id: Some(1),
            status: GameStatusSnapshot { background: text.into(), ..Default::default() },
            memories: vec![(1, GameMemoryBank::default())],
            script: Some(ScriptSnapshot {
                folder: "test".into(), vars: Default::default(), chapter: text.into(), event: 1,
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
    fn reading_anchor_is_all_or_nothing() {
        let script = ScriptStatus {
            folder_key: "chapter".into(), name: String::new(), description: String::new(),
            intro_chapter: String::new(), settings: Default::default(),
            script_path: std::path::PathBuf::from("data/scripts/group/story"),
            recommand_start: String::new(), adventure: Default::default(), running_client_id: None,
            current_chapter_key: "engine".into(), current_event_process: 9,
            vars: serde_json::from_value(serde_json::json!({"source": "engine"})).unwrap(),
            plugin_id: None,
        };
        let mut anchor = PlayerAnchor {
            chapter: Some("reader".into()), event_index: Some(3), line_count: Some(5),
            vars_json: Some(r#"{"source":"reader"}"#.into()),
        };
        let captured = ScriptSnapshot::capture(&script, &anchor);
        assert_eq!(captured.folder, "group/story");
        assert_eq!(captured.chapter, "reader");
        assert_eq!(captured.event, 3);
        assert_eq!(captured.vars["__line_count"], 5);
        for json in [Some("invalid".into()), None] {
            anchor.vars_json = json;
            let fallback = ScriptSnapshot::capture(&script, &anchor);
            assert_eq!(fallback.chapter, "engine");
            assert_eq!(fallback.event, 9);
            assert_eq!(fallback.vars, script.vars);
            assert!(!fallback.vars.contains_key("__line_count"));
        }
    }

    #[tokio::test]
    async fn late_failure_rolls_back_all_components() {
        let db = database().await;
        let id = snapshot("old").persist(&db, SaveTarget::Create("slot".into())).await.unwrap();
        let before = SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap();
        let lines = SaveRepo::get_gameline_list(&db, id).await.unwrap();
        let banks = SaveRepo::get_memory_banks(&db, id).await.unwrap();
        let script = SaveRepo::get_running_script(&db, before.running_script_id.unwrap()).await.unwrap();
        fail_script_writes(&db).await;
        let mut changed = snapshot("new");
        changed.lines[0].base.id = lines[0].base.id;
        changed.lines[0].perceived_role_ids.clear();
        changed.memories[0].1.data.long_term = "new memory".into();
        let error = changed.persist(&db, SaveTarget::Update(id)).await.unwrap_err();
        assert!(format!("{error:#}").contains("injected script failure"));
        assert_eq!(before, SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap());
        assert_eq!(lines, SaveRepo::get_gameline_list(&db, id).await.unwrap());
        assert_eq!(banks, SaveRepo::get_memory_banks(&db, id).await.unwrap());
        assert_eq!(script, SaveRepo::get_running_script(&db, before.running_script_id.unwrap()).await.unwrap());
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
    async fn manual_guard_and_auto_takeover_are_preserved() {
        let db = database().await;
        let id = snapshot("old").persist(&db, SaveTarget::Create("auto slot".into())).await.unwrap();
        let old = SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap();
        let mut changed = snapshot("new");
        assert!(changed.persist(&db, SaveTarget::Update(id)).await.is_err());
        changed.script = None;
        changed.memories.clear();
        let banks = SaveRepo::get_memory_banks(&db, id).await.unwrap();
        let saved_id = changed.persist(&db, SaveTarget::Auto {
            prefix: "auto".into(), title: "auto new".into(),
        }).await.unwrap();
        assert_eq!(saved_id, id);
        let saved = SaveRepo::get_save_by_id(&db, id).await.unwrap().unwrap();
        assert!(saved.running_script_id.is_none());
        assert!(SaveRepo::get_running_script(&db, old.running_script_id.unwrap()).await.unwrap().is_none());
        assert_eq!(SaveRepo::get_line_list(&db, id).await.unwrap()[0].content, "new");
        assert_eq!(banks, SaveRepo::get_memory_banks(&db, id).await.unwrap());
    }
}
