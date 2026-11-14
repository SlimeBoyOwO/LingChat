//! 好感度的状态操作：按（角色 × 玩家角色）读、写、播种。
//!
//! 好感度有两种存法：旧档共享的 `aff.{role_id}`，和新版的
//! `aff.{role_id}.{player_role_id}`。读的时候按「专属 → 旧档共享」回退（见
//! [`affection::resolve_var`]），写的时候只写专属那一档——同一个角色在不同
//! 玩家身份下各记一份，旧档不被改写。

use std::collections::HashMap;

use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::ai_service::affection;
use crate::ai_service::game_system::game_status::GameStatus;

impl GameStatus {
    /// 按当前玩家角色把全局变量里的好感度覆盖到内存中的角色状态。
    pub fn overlay_affections_for_player(&mut self, player_role_id: Option<i32>) {
        self.role_manager
            .overlay_affections_from_vars(&self.global_variables, player_role_id);
    }

    /// 按（角色 × 玩家角色）播种初始好感：已有专属值、或旧档共享默认值存在时不覆盖。
    /// 播种成功且非默认值时，补写一条「感情档位」旁白。
    pub async fn seed_player_affection(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        player_role_id: Option<i32>,
    ) -> Result<()> {
        let (init, role_name) = {
            let Some(role) = self.role_manager.get_loaded(role_id) else {
                return Ok(());
            };
            let identity_override = player_role_id
                .and_then(|id| self.role_manager.relation_affection_of_role(role_id, id));
            (
                affection::effective_initial_affection(&role.settings, identity_override),
                role.display_name
                    .clone()
                    .unwrap_or_else(|| role.settings.ai_name.clone()),
            )
        };
        let Some(init) = init else {
            return Ok(());
        };
        let key = affection::var_key_for(role_id, player_role_id);
        if self.global_variables.contains_key(&key) {
            return Ok(());
        }
        if player_role_id.is_some()
            && self
                .global_variables
                .contains_key(&affection::var_key(role_id))
        {
            return Ok(());
        }
        self.set_player_affection_initial(role_id, player_role_id, init);
        if init != 0 {
            self.add_affection_state_narration(db, &role_name, init)
                .await?;
        }
        Ok(())
    }

    /// 改初始好感度后同步当前存档：写值、补一条感情档位旁白、落一次快照，
    /// 返回需要广播给前端的变更载荷。
    pub async fn sync_player_affection_initial(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        player_role_id: Option<i32>,
        value: i32,
    ) -> affection::AffectionChangedPayload {
        let role_name = self.role_manager.get_loaded(role_id).map(|role| {
            role.display_name
                .clone()
                .unwrap_or_else(|| role.settings.ai_name.clone())
        });
        self.set_player_affection_initial(role_id, player_role_id, value);

        let vector = affection::uniform_vector(value);
        let payload = affection::AffectionChangedPayload {
            role_id,
            deltas: HashMap::new(),
            negative_deltas: HashMap::new(),
            average: vector.average(),
            values: vector,
            negative: crate::ai_service::types::NegativeVector::default(),
            reason: "初始好感度已同步到当前存档".to_string(),
        };

        if value != 0 {
            if let Some(role_name) = role_name.filter(|name| !name.is_empty()) {
                if let Err(e) = self
                    .add_affection_state_narration(db, &role_name, value)
                    .await
                {
                    tracing::warn!("写入感情档位旁白失败: {e}");
                } else {
                    self.persist_lines_if_bound(db).await;
                }
            }
        }
        if let Some(save_id) = self.active_save_id {
            match serde_json::to_string(&self.to_snapshot()) {
                Ok(snapshot_json) => {
                    if let Err(e) = crate::db::managers::save_repo::SaveRepo::update_save_status(
                        db,
                        save_id,
                        &snapshot_json,
                    )
                    .await
                    {
                        tracing::warn!("初始好感度同步后落档失败: {e}");
                    }
                },
                Err(e) => tracing::warn!("序列化状态失败: {e}"),
            }
        }
        payload
    }

    /// 直接把初始好感写成该玩家角色专属那一档，并覆盖到内存。
    pub fn set_player_affection_initial(
        &mut self,
        role_id: i32,
        player_role_id: Option<i32>,
        init: i32,
    ) {
        let vector = affection::uniform_vector(init);
        self.set_variable(
            affection::var_key_for(role_id, player_role_id),
            affection::state_to_value(&affection::state_from_vector(vector)),
        );
        self.overlay_affections_for_player(player_role_id);
    }
}
