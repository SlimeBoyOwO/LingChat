use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use sea_orm::*;
use std::collections::HashMap;

use crate::ai_service::types::{GameLine, LineAttributeExt};
use crate::db::entities::{
    line, line_perception, memory_bank, running_script, save, save_identity,
};

pub struct SaveRepo;

// ========== Save CRUD ==========

impl SaveRepo {
    pub async fn count_saves(db: &DatabaseConnection) -> Result<u64> {
        save::Entity::find()
            .count(db)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    pub async fn list_saves(
        db: &DatabaseConnection,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<save::Model>> {
        let offset = (page.saturating_sub(1)) * page_size;
        save::Entity::find()
            .order_by_desc(save::Column::UpdateDate)
            .offset(offset)
            .limit(page_size)
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    pub async fn get_save_by_id(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Option<save::Model>> {
        save::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    /// Find the most recent save whose title starts with the given prefix.
    pub async fn find_save_by_title_prefix(
        db: &DatabaseConnection,
        prefix: &str,
    ) -> Result<Option<save::Model>> {
        save::Entity::find()
            .filter(save::Column::Title.like(format!("{}%", prefix)))
            .order_by_desc(save::Column::UpdateDate)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    pub async fn create_save(db: &DatabaseConnection, title: &str) -> Result<save::Model> {
        let now = Utc::now().naive_utc();
        let active = save::ActiveModel {
            title: Set(title.to_string()),
            status: Set("{}".to_string()),
            create_date: Set(now),
            update_date: Set(now),
            ..Default::default()
        };
        active.insert(db).await.map_err(|e| anyhow!("{e}"))
    }

    pub async fn update_save_title(
        db: &DatabaseConnection,
        save_id: i32,
        title: &str,
    ) -> Result<()> {
        let model = save::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .context("Save not found")?;
        let mut active: save::ActiveModel = model.into();
        active.title = Set(title.to_string());
        active.update_date = Set(Utc::now().naive_utc());
        active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn update_save_status(
        db: &DatabaseConnection,
        save_id: i32,
        status_json: &str,
    ) -> Result<()> {
        let model = save::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .context("Save not found")?;
        let mut active: save::ActiveModel = model.into();
        active.status = Set(status_json.to_string());
        active.update_date = Set(Utc::now().naive_utc());
        active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn update_save_main_role(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: Option<i32>,
    ) -> Result<()> {
        let model = save::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .context("Save not found")?;
        let mut active: save::ActiveModel = model.into();
        active.main_role_id = Set(role_id);
        active.update_date = Set(Utc::now().naive_utc());
        active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update_save_last_message(
        db: &DatabaseConnection,
        save_id: i32,
        last_message_id: Option<i32>,
    ) -> Result<()> {
        let model = save::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .context("Save not found")?;
        let mut active: save::ActiveModel = model.into();
        active.last_message_id = Set(last_message_id);
        active.update_date = Set(Utc::now().naive_utc());
        active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn update_save_running_script(
        db: &DatabaseConnection,
        save_id: i32,
        running_script_id: Option<i32>,
    ) -> Result<()> {
        let model = save::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .context("Save not found")?;
        let mut active: save::ActiveModel = model.into();
        active.running_script_id = Set(running_script_id);
        active.update_date = Set(Utc::now().naive_utc());
        active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn delete_save(db: &DatabaseConnection, save_id: i32) -> Result<bool> {
        // Delete all line_perception rows for this save's lines
        let line_ids: Vec<i32> = line::Entity::find()
            .select_only()
            .column(line::Column::Id)
            .filter(line::Column::SaveId.eq(save_id))
            .into_tuple()
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;

        if !line_ids.is_empty() {
            line_perception::Entity::delete_many()
                .filter(line_perception::Column::LineId.is_in(line_ids.clone()))
                .exec(db)
                .await
                .map_err(|e| anyhow!("{e}"))?;
        }

        // Delete all lines for this save
        line::Entity::delete_many()
            .filter(line::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;

        save_identity::Entity::delete_many()
            .filter(save_identity::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;

        let result = save::Entity::delete_by_id(save_id)
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(result.rows_affected > 0)
    }
}

// ========== Line Reconstruction ==========

impl SaveRepo {
    pub async fn get_line_list(db: &DatabaseConnection, save_id: i32) -> Result<Vec<line::Model>> {
        let all_lines = line::Entity::find()
            .filter(line::Column::SaveId.eq(save_id))
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;

        let lines_map: HashMap<i32, line::Model> =
            all_lines.into_iter().map(|l| (l.id, l)).collect();

        let save = Self::get_save_by_id(db, save_id)
            .await?
            .context("Save not found")?;

        let mut history: Vec<line::Model> = Vec::new();
        let mut current_id = save.last_message_id;

        while let Some(id) = current_id {
            if let Some(line) = lines_map.get(&id) {
                history.push(line.clone());
                current_id = line.parent_line_id;
            } else {
                break;
            }
        }

        history.reverse();
        Ok(history)
    }

    /// Convert DB lines to `GameLine` with perception data batch-fetched.
    pub async fn get_gameline_list(db: &DatabaseConnection, save_id: i32) -> Result<Vec<GameLine>> {
        let lines = Self::get_line_list(db, save_id).await?;

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        let line_ids: Vec<i32> = lines.iter().map(|l| l.id).collect();

        let perceptions = line_perception::Entity::find()
            .filter(line_perception::Column::LineId.is_in(line_ids.clone()))
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;

        let mut perception_map: HashMap<i32, Vec<i32>> = HashMap::new();
        for p in perceptions {
            perception_map.entry(p.line_id).or_default().push(p.role_id);
        }

        let game_lines: Vec<GameLine> = lines
            .into_iter()
            .map(|db_line| {
                let perceived = perception_map.get(&db_line.id).cloned().unwrap_or_default();
                Ok(GameLine {
                    base: crate::ai_service::types::LineBase {
                        id: Some(db_line.id),
                        content: db_line.content,
                        original_emotion: db_line.original_emotion,
                        predicted_emotion: db_line.predicted_emotion,
                        tts_content: db_line.tts_content,
                        action_content: db_line.action_content,
                        thinking: db_line.thinking,
                        audio_file: db_line.audio_file,
                        tool_call: db_line.tool_call,
                        round_id: db_line.round_id,
                        attribute: LineAttributeExt(db_line.attribute),
                        sender_role_id: db_line.sender_role_id,
                        display_name: db_line.display_name,
                        identity_id: db_line.identity_id,
                    },
                    perceived_role_ids: perceived,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(game_lines)
    }
}

// ========== Smart Diff: sync_lines ==========

/// 两条台词行除 id 与感知列表外是否完全一致（sync_lines 与 can_sync_into 共用同一把尺）。
fn same_line_fields(db_line: &line::Model, input_line: &GameLine) -> bool {
    db_line.content == input_line.base.content
        && db_line.attribute == input_line.base.attribute.0
        && db_line.sender_role_id == input_line.base.sender_role_id
        && db_line.original_emotion == input_line.base.original_emotion
        && db_line.predicted_emotion == input_line.base.predicted_emotion
        && db_line.tts_content == input_line.base.tts_content
        && db_line.audio_file == input_line.base.audio_file
        && db_line.thinking == input_line.base.thinking
        && db_line.action_content == input_line.base.action_content
        && db_line.display_name == input_line.base.display_name
        && db_line.tool_call == input_line.base.tool_call
        && db_line.round_id == input_line.base.round_id
}

impl SaveRepo {
    /// Port of Python `SaveManager.sync_lines`. Walks DB line chain and
    /// input `GameLine` list in lockstep, finds the divergence point, then
    /// deletes stale DB lines and inserts new input lines after that point.
    ///
    /// `allow_rebuild`：首行即分歧（输入与存档无共同锚点）时是否允许接管该档。
    /// 自动存档传 true——它的档位只跟踪"当前这一局"，被别的会话（切角色、开新档）
    /// 占住时必须整体重建，否则会永久拒绝写入、自动存档彻底停摆；手动存档/语音
    /// 回填等路径传 false，继续由安全网拦截，避免误删另一段对话的历史。
    pub async fn sync_lines(
        db: &DatabaseConnection,
        save_id: i32,
        input_lines: &[GameLine],
        allow_rebuild: bool,
    ) -> Result<()> {
        let db_lines = Self::get_line_list(db, save_id).await?;

        // Walk both lists to find divergence point
        let max_check = db_lines.len().min(input_lines.len());
        let mut diverge = max_check; // default: no divergence within overlap

        for i in 0..max_check {
            let db_line = &db_lines[i];
            let input_line = &input_lines[i];

            // Try ID match first
            if let Some(input_id) = input_line.base.id {
                if input_id == db_line.id {
                    // Same ID — update if any persisted field changed
                    if !same_line_fields(db_line, input_line) {
                        let mut active: line::ActiveModel = db_line.clone().into();
                        active.content = Set(input_line.base.content.clone());
                        active.attribute = Set(input_line.base.attribute.0.clone());
                        active.sender_role_id = Set(input_line.base.sender_role_id);
                        active.original_emotion = Set(input_line.base.original_emotion.clone());
                        active.predicted_emotion = Set(input_line.base.predicted_emotion.clone());
                        active.tts_content = Set(input_line.base.tts_content.clone());
                        active.action_content = Set(input_line.base.action_content.clone());
                        active.audio_file = Set(input_line.base.audio_file.clone());
                        active.thinking = Set(input_line.base.thinking.clone());
                        active.display_name = Set(input_line.base.display_name.clone());
                        active.identity_id = Set(input_line.base.identity_id.clone());
                        active.tool_call = Set(input_line.base.tool_call.clone());
                        active.round_id = Set(input_line.base.round_id.clone());
                    }
                    continue;
                }
            }

            // Try weak match using every persisted field except generated IDs.
            if same_line_fields(db_line, input_line) && db_line.save_id == save_id {
                // Same logical line — no update needed for existing DB row
                continue;
            }

            // No match — divergence starts here
            diverge = i;
            break;
        }

        // 自动存档位允许接管：它本就该随当前这局走，diverge=0 时下面等价于整档重建。
        if diverge == 0 && !db_lines.is_empty() && !input_lines.is_empty() {
            if !allow_rebuild {
                return Err(anyhow!(
                    "sync_lines 安全保护：输入与存档无共同锚点，拒绝全量覆盖（save_id={save_id}，db_lines={}，input_lines={}）",
                    db_lines.len(),
                    input_lines.len()
                ));
            }
            tracing::warn!(
                "sync_lines 接管档位：输入与存档无共同锚点，整体重建（save_id={save_id}，db_lines={}，input_lines={}）",
                db_lines.len(),
                input_lines.len()
            );
        }

        let txn = db.begin().await.map_err(|e| anyhow!("{e}"))?;

        let stale_lines = &db_lines[diverge..];
        if !stale_lines.is_empty() {
            let stale_ids: Vec<i32> = stale_lines.iter().map(|l| l.id).collect();

            line_perception::Entity::delete_many()
                .filter(line_perception::Column::LineId.is_in(stale_ids.clone()))
                .exec(&txn)
                .await
                .map_err(|e| anyhow!("{e}"))?;

            line::Entity::delete_many()
                .filter(line::Column::Id.is_in(stale_ids))
                .exec(&txn)
                .await
                .map_err(|e| anyhow!("{e}"))?;
        }

        let matched_ids: Vec<i32> = db_lines[..diverge].iter().map(|l| l.id).collect();
        let mut existing_perceptions: HashMap<i32, Vec<i32>> = HashMap::new();
        if !matched_ids.is_empty() {
            let rows = line_perception::Entity::find()
                .filter(line_perception::Column::LineId.is_in(matched_ids))
                .all(&txn)
                .await
                .map_err(|e| anyhow!("{e}"))?;
            for row in rows {
                existing_perceptions
                    .entry(row.line_id)
                    .or_default()
                    .push(row.role_id);
            }
        }

        for i in 0..diverge {
            let db_line = &db_lines[i];
            let input_line = &input_lines[i];
            if input_line.base.id != Some(db_line.id) {
                continue;
            }

            // 1) line 表字段
            if db_line.content != input_line.base.content
                || db_line.attribute != input_line.base.attribute.0
                || db_line.sender_role_id != input_line.base.sender_role_id
                || db_line.original_emotion != input_line.base.original_emotion
                || db_line.predicted_emotion != input_line.base.predicted_emotion
                || db_line.tts_content != input_line.base.tts_content
                || db_line.audio_file != input_line.base.audio_file
                || db_line.thinking != input_line.base.thinking
                || db_line.action_content != input_line.base.action_content
                || db_line.display_name != input_line.base.display_name
                || db_line.tool_call != input_line.base.tool_call
                || db_line.round_id != input_line.base.round_id
            {
                let mut active: line::ActiveModel = db_line.clone().into();
                active.content = Set(input_line.base.content.clone());
                active.attribute = Set(input_line.base.attribute.0.clone());
                active.sender_role_id = Set(input_line.base.sender_role_id);
                active.original_emotion = Set(input_line.base.original_emotion.clone());
                active.predicted_emotion = Set(input_line.base.predicted_emotion.clone());
                active.tts_content = Set(input_line.base.tts_content.clone());
                active.action_content = Set(input_line.base.action_content.clone());
                active.audio_file = Set(input_line.base.audio_file.clone());
                active.thinking = Set(input_line.base.thinking.clone());
                active.display_name = Set(input_line.base.display_name.clone());
                active.tool_call = Set(input_line.base.tool_call.clone());
                active.round_id = Set(input_line.base.round_id.clone());
                active.update(&txn).await.map_err(|e| anyhow!("{e}"))?;
            }

            let mut existing = existing_perceptions
                .get(&db_line.id)
                .cloned()
                .unwrap_or_default();
            let mut desired = input_line.perceived_role_ids.clone();
            existing.sort_unstable();
            desired.sort_unstable();
            desired.dedup();
            if existing != desired {
                line_perception::Entity::delete_many()
                    .filter(line_perception::Column::LineId.eq(db_line.id))
                    .exec(&txn)
                    .await
                    .map_err(|e| anyhow!("{e}"))?;
                for role_id in &input_line.perceived_role_ids {
                    line_perception::ActiveModel {
                        line_id: Set(db_line.id),
                        role_id: Set(*role_id),
                    }
                    .insert(&txn)
                    .await
                    .map_err(|e| anyhow!("{e}"))?;
                }
            }
        }

        let new_input_lines = &input_lines[diverge..];
        let mut parent_id: Option<i32> = None;
        if !new_input_lines.is_empty() {
            parent_id = if diverge > 0 {
                Some(db_lines[diverge - 1].id)
            } else {
                None
            };

            for input_line in new_input_lines.iter() {
                let new_line = line::ActiveModel {
                    content: Set(input_line.base.content.clone()),
                    attribute: Set(input_line.base.attribute.0.clone()),
                    sender_role_id: Set(input_line.base.sender_role_id),
                    display_name: Set(input_line.base.display_name.clone()),
                    identity_id: Set(input_line.base.identity_id.clone()),
                    original_emotion: Set(input_line.base.original_emotion.clone()),
                    predicted_emotion: Set(input_line.base.predicted_emotion.clone()),
                    tts_content: Set(input_line.base.tts_content.clone()),
                    action_content: Set(input_line.base.action_content.clone()),
                    audio_file: Set(input_line.base.audio_file.clone()),
                    thinking: Set(input_line.base.thinking.clone()),
                    tool_call: Set(input_line.base.tool_call.clone()),
                    round_id: Set(input_line.base.round_id.clone()),
                    save_id: Set(save_id),
                    parent_line_id: Set(parent_id),
                    ..Default::default()
                };

                let inserted = new_line.insert(&txn).await.map_err(|e| anyhow!("{e}"))?;
                let new_id = inserted.id;

                // Insert line_perception rows
                for &role_id in &input_line.perceived_role_ids {
                    let perception = line_perception::ActiveModel {
                        line_id: Set(new_id),
                        role_id: Set(role_id),
                    };
                    perception.insert(&txn).await.map_err(|e| anyhow!("{e}"))?;
                }

                parent_id = Some(new_id);
            }
        }

        let last_id = if !new_input_lines.is_empty() {
            parent_id
        } else if diverge > 0 {
            Some(db_lines[diverge - 1].id)
        } else {
            None
        };
        let save_model = save::Entity::find_by_id(save_id)
            .one(&txn)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .context("Save not found")?;
        let mut active: save::ActiveModel = save_model.into();
        active.last_message_id = Set(last_id);
        active.update_date = Set(Utc::now().naive_utc());
        active.update(&txn).await.map_err(|e| anyhow!("{e}"))?;

        txn.commit().await.map_err(|e| anyhow!("{e}"))?;

        Ok(())
    }

    /// 输入台词表能否安全写入该存档：存档为空，或首行与输入首行匹配（与 sync_lines 的锚点规则一致）。
    pub async fn can_sync_into(
        db: &DatabaseConnection,
        save_id: i32,
        input_lines: &[GameLine],
    ) -> Result<bool> {
        let db_lines = Self::get_line_list(db, save_id).await?;
        if db_lines.is_empty() || input_lines.is_empty() {
            return Ok(true);
        }
        let db_line = &db_lines[0];
        let input_line = &input_lines[0];
        if let Some(input_id) = input_line.base.id {
            if input_id == db_line.id {
                return Ok(true);
            }
        }
        Ok(same_line_fields(db_line, input_line))
    }

    /// 内存行拿到 id 后，后续锚点按 id 命中——人设行重建改动了首行内容也不会再触发"首行分歧"。
    pub async fn bind_line_ids(
        db: &DatabaseConnection,
        save_id: i32,
        lines: &mut [GameLine],
    ) -> Result<()> {
        let db_lines = Self::get_line_list(db, save_id).await?;
        for (i, db_line) in db_lines.iter().enumerate() {
            if i >= lines.len() {
                break;
            }
            if lines[i].base.id.is_none() {
                lines[i].base.id = Some(db_line.id);
            }
        }
        Ok(())
    }
}

// ========== Memory Bank ==========

impl SaveRepo {
    #[allow(dead_code)]
    pub async fn upsert_memory_bank(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: Option<i32>,
        info_json: &str,
    ) -> Result<()> {
        // Delete existing for this (save_id, role_id) pair
        let mut delete =
            memory_bank::Entity::delete_many().filter(memory_bank::Column::SaveId.eq(save_id));
        if let Some(rid) = role_id {
            delete = delete.filter(memory_bank::Column::RoleId.eq(rid));
        } else {
            delete = delete.filter(memory_bank::Column::RoleId.is_null());
        }
        delete.exec(db).await.map_err(|e| anyhow!("{e}"))?;

        // Insert new
        let active = memory_bank::ActiveModel {
            info: Set(info_json.to_string()),
            save_id: Set(save_id),
            role_id: Set(role_id),
            ..Default::default()
        };
        active.insert(db).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_memory_banks(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Vec<memory_bank::Model>> {
        memory_bank::Entity::find()
            .filter(memory_bank::Column::SaveId.eq(save_id))
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    pub async fn delete_memory_banks_by_save(db: &DatabaseConnection, save_id: i32) -> Result<()> {
        memory_bank::Entity::delete_many()
            .filter(memory_bank::Column::SaveId.eq(save_id))
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }
}

// ========== Running Script ==========

impl SaveRepo {
    pub async fn get_running_script(
        db: &DatabaseConnection,
        script_id: i32,
    ) -> Result<Option<running_script::Model>> {
        running_script::Entity::find_by_id(script_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))
    }

    pub async fn upsert_running_script(
        db: &DatabaseConnection,
        save_id: i32,
        script_folder: &str,
        variable_info: &str,
        current_chapter: &str,
        event_sequence: i32,
    ) -> Result<i32> {
        // Check if save already has a running_script
        let save_model = Self::get_save_by_id(db, save_id)
            .await?
            .context("Save not found")?;

        if let Some(existing_id) = save_model.running_script_id {
            // Update existing
            let rs = running_script::Entity::find_by_id(existing_id)
                .one(db)
                .await
                .map_err(|e| anyhow!("{e}"))?;
            if let Some(rs) = rs {
                let mut active: running_script::ActiveModel = rs.into();
                active.script_folder = Set(script_folder.to_string());
                active.variable_info = Set(variable_info.to_string());
                active.current_chapter = Set(current_chapter.to_string());
                active.event_sequence = Set(event_sequence);
                active.update(db).await.map_err(|e| anyhow!("{e}"))?;
                return Ok(existing_id);
            }
        }

        // Insert new
        let active = running_script::ActiveModel {
            script_folder: Set(script_folder.to_string()),
            variable_info: Set(variable_info.to_string()),
            current_chapter: Set(current_chapter.to_string()),
            event_sequence: Set(event_sequence),
            save_id: Set(save_id),
            ..Default::default()
        };
        let inserted = active.insert(db).await.map_err(|e| anyhow!("{e}"))?;

        // Link to save
        Self::update_save_running_script(db, save_id, Some(inserted.id)).await?;

        Ok(inserted.id)
    }

    pub async fn delete_running_script(db: &DatabaseConnection, script_id: i32) -> Result<()> {
        running_script::Entity::delete_by_id(script_id)
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub async fn upsert_save_identity(
        db: &DatabaseConnection,
        save_id: i32,
        identity_id: &str,
    ) -> Result<()> {
        let now = Utc::now().naive_utc().to_string();

        if let Some(existing) = save_identity::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
        {
            let mut active: save_identity::ActiveModel = existing.into();
            active.identity_id = Set(identity_id.to_string());
            active.updated_at = Set(now);
            active.update(db).await.map_err(|e| anyhow!("{e}"))?;
        } else {
            let active = save_identity::ActiveModel {
                save_id: Set(save_id),
                identity_id: Set(identity_id.to_string()),
                created_at: Set(now.clone()),
                updated_at: Set(now),
            };
            active.insert(db).await.map_err(|e| anyhow!("{e}"))?;
        }

        Ok(())
    }

    pub async fn get_save_identity(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Option<String>> {
        Ok(save_identity::Entity::find_by_id(save_id)
            .one(db)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map(|m| m.identity_id))
    }

    pub async fn find_saves_using_identity(
        db: &DatabaseConnection,
        identity_id: &str,
    ) -> Result<Vec<i32>> {
        save_identity::Entity::find()
            .filter(save_identity::Column::IdentityId.eq(identity_id))
            .all(db)
            .await
            .map_err(|e| anyhow!("{e}"))
            .map(|rows| rows.into_iter().map(|r| r.save_id).collect())
    }

    pub async fn clear_running_script(db: &DatabaseConnection, save_id: i32) -> Result<()> {
        let save_model = Self::get_save_by_id(db, save_id)
            .await?
            .context("Save not found")?;
        if let Some(rs_id) = save_model.running_script_id {
            Self::update_save_running_script(db, save_id, None).await?;
            Self::delete_running_script(db, rs_id).await?;
        }
        Ok(())
    }
}
