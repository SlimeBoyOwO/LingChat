use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use sea_orm::*;
use std::collections::HashMap;

use crate::ai_service::types::{GameLine, LineAttributeExt};
use crate::db::entities::{
    line, line_perception, memory_bank, running_script, save,
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
            .filter(save::Column::Title.like(&format!("{}%", prefix)))
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

        let result = save::Entity::delete_by_id(save_id)
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(result.rows_affected > 0)
    }
}

// ========== Line Reconstruction ==========

impl SaveRepo {
    /// Reconstruct ordered line list by walking `parent_line_id` chain
    /// from `save.last_message_id` backwards, then reversing.
    pub async fn get_line_list(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Vec<line::Model>> {
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
    pub async fn get_gameline_list(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<Vec<GameLine>> {
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
                let perceived = perception_map
                    .get(&db_line.id)
                    .cloned()
                    .unwrap_or_default();
                GameLine {
                    base: crate::ai_service::types::LineBase {
                        id: Some(db_line.id),
                        content: db_line.content,
                        original_emotion: db_line.original_emotion,
                        predicted_emotion: db_line.predicted_emotion,
                        tts_content: db_line.tts_content,
                        action_content: db_line.action_content,
                        audio_file: db_line.audio_file,
                        attribute: LineAttributeExt(db_line.attribute),
                        sender_role_id: db_line.sender_role_id,
                        display_name: db_line.display_name,
                    },
                    perceived_role_ids: perceived,
                }
            })
            .collect();

        Ok(game_lines)
    }
}

// ========== Smart Diff: sync_lines ==========

impl SaveRepo {
    /// Port of Python `SaveManager.sync_lines`. Walks DB line chain and
    /// input `GameLine` list in lockstep, finds the divergence point, then
    /// deletes stale DB lines and inserts new input lines after that point.
    pub async fn sync_lines(
        db: &DatabaseConnection,
        save_id: i32,
        input_lines: &[GameLine],
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
                    // Same ID — update if content changed
                    if db_line.content != input_line.base.content
                        || db_line.attribute != input_line.base.attribute.0
                        || db_line.sender_role_id != input_line.base.sender_role_id
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
                        active.display_name = Set(input_line.base.display_name.clone());
                        active
                            .update(db)
                            .await
                            .map_err(|e| anyhow!("{e}"))?;
                    }
                    continue;
                }
            }

            // Try weak match by content + attribute + sender
            if db_line.content == input_line.base.content
                && db_line.attribute == input_line.base.attribute.0
                && db_line.sender_role_id == input_line.base.sender_role_id
            {
                // Same logical line — no update needed for existing DB row
                continue;
            }

            // No match — divergence starts here
            diverge = i;
            break;
        }

        // 3. Delete stale DB lines and their perceptions
        let stale_lines = &db_lines[diverge..];
        if !stale_lines.is_empty() {
            let stale_ids: Vec<i32> = stale_lines.iter().map(|l| l.id).collect();

            line_perception::Entity::delete_many()
                .filter(line_perception::Column::LineId.is_in(stale_ids.clone()))
                .exec(db)
                .await
                .map_err(|e| anyhow!("{e}"))?;

            line::Entity::delete_many()
                .filter(line::Column::Id.is_in(stale_ids))
                .exec(db)
                .await
                .map_err(|e| anyhow!("{e}"))?;
        }

        // 4. Insert new input lines after divergence point
        let new_input_lines = &input_lines[diverge..];
        let mut parent_id: Option<i32> = None;
        if !new_input_lines.is_empty() {
            parent_id = if diverge > 0 {
                Some(db_lines[diverge - 1].id)
            } else {
                None
            };

            for (_j, input_line) in new_input_lines.iter().enumerate() {
                let new_line = line::ActiveModel {
                    content: Set(input_line.base.content.clone()),
                    attribute: Set(input_line.base.attribute.0.clone()),
                    sender_role_id: Set(input_line.base.sender_role_id),
                    display_name: Set(input_line.base.display_name.clone()),
                    original_emotion: Set(input_line.base.original_emotion.clone()),
                    predicted_emotion: Set(input_line.base.predicted_emotion.clone()),
                    tts_content: Set(input_line.base.tts_content.clone()),
                    action_content: Set(input_line.base.action_content.clone()),
                    audio_file: Set(input_line.base.audio_file.clone()),
                    save_id: Set(save_id),
                    parent_line_id: Set(parent_id),
                    ..Default::default()
                };

                let inserted = new_line.insert(db).await.map_err(|e| anyhow!("{e}"))?;
                let new_id = inserted.id;

                // Insert line_perception rows
                for &role_id in &input_line.perceived_role_ids {
                    let perception = line_perception::ActiveModel {
                        line_id: Set(new_id),
                        role_id: Set(role_id),
                    };
                    perception.insert(db).await.map_err(|e| anyhow!("{e}"))?;
                }

                parent_id = Some(new_id);
            }
        }

        // 5. Update save.last_message_id
        // parent_id tracks the last inserted line's ID through the insert loop.
        // If nothing was inserted, keep the last non-stale DB line as chain tail.
        let last_id = if !new_input_lines.is_empty() {
            parent_id
        } else if diverge > 0 {
            Some(db_lines[diverge - 1].id)
        } else {
            None
        };
        Self::update_save_last_message(db, save_id, last_id).await?;

        Ok(())
    }
}

// ========== Memory Bank ==========

impl SaveRepo {
    pub async fn upsert_memory_bank(
        db: &DatabaseConnection,
        save_id: i32,
        role_id: Option<i32>,
        info_json: &str,
    ) -> Result<()> {
        // Delete existing for this (save_id, role_id) pair
        let mut delete = memory_bank::Entity::delete_many()
            .filter(memory_bank::Column::SaveId.eq(save_id));
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

    pub async fn delete_memory_banks_by_save(
        db: &DatabaseConnection,
        save_id: i32,
    ) -> Result<()> {
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

    pub async fn delete_running_script(
        db: &DatabaseConnection,
        script_id: i32,
    ) -> Result<()> {
        running_script::Entity::delete_by_id(script_id)
            .exec(db)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }
}
