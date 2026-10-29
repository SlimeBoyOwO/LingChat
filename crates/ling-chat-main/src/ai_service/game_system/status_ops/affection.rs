//! 好感度的状态操作：按（角色 × 身份）读、写、播种。
//!
//! 好感度有两种存法：旧档共享的 `aff.{role_id}`，和新版的
//! `aff.{role_id}.{identity_id}`。读的时候按「身份专属 → 旧档共享」回退（见
//! [`affection::resolve_var`]），写的时候只写身份专属那一档——同一个角色在不同
//! 身份下各记一份，旧档不被改写。
//!
//! [`affection::resolve_var`]: crate::ai_service::affection::resolve_var

use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::ai_service::affection;
use crate::ai_service::game_system::game_status::GameStatus;

impl GameStatus {
    /// 按当前身份把全局变量里的好感度覆盖到内存中的角色状态。
    pub fn overlay_affections_for_identity(&mut self, identity_id: Option<&str>) {
        self.role_manager
            .overlay_affections_from_vars(&self.global_variables, identity_id);
    }

    /// 按（角色 × 身份）播种初始好感：该身份已有专属值、或旧档共享默认值存在时不覆盖。
    /// 播种成功且非默认值时，补写一条「感情档位」旁白——好感度不做逐轮注入，缺这条
    /// 旁白时角色要等第一次评估才知道自己的感情，设定完的初始值就不会体现在第一轮。
    pub async fn seed_identity_affection(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        identity_id: Option<&str>,
    ) -> Result<()> {
        let (init, role_name) = {
            let Some(role) = self.role_manager.get_loaded(role_id) else {
                return Ok(());
            };
            (
                affection::effective_initial_affection(&role.settings, identity_id),
                role.display_name
                    .clone()
                    .unwrap_or_else(|| role.settings.ai_name.clone()),
            )
        };
        let Some(init) = init else {
            return Ok(());
        };
        let key = affection::var_key_for(role_id, identity_id);
        if self.global_variables.contains_key(&key) {
            return Ok(());
        }
        if identity_id.is_some()
            && self
                .global_variables
                .contains_key(&affection::var_key(role_id))
        {
            return Ok(());
        }
        self.set_identity_affection_initial(role_id, identity_id, init);
        if init != 0 {
            self.add_affection_state_narration(db, &role_name, init)
                .await?;
        }
        Ok(())
    }

    /// 直接把初始好感写成该身份专属那一档，并覆盖到内存。
    pub fn set_identity_affection_initial(
        &mut self,
        role_id: i32,
        identity_id: Option<&str>,
        init: i32,
    ) {
        let vector = affection::uniform_vector(init);
        self.set_variable(
            affection::var_key_for(role_id, identity_id),
            affection::state_to_value(&affection::state_from_vector(vector)),
        );
        self.overlay_affections_for_identity(identity_id);
    }
}
