//! 业务旁白：把「刚刚发生的事」写成一条提示行，落进台词表。
//!
//! 与 [`super::line`] 的分工：那边只管一条行的形状（信封、属性、是否计入玩家消息
//! 序号），这里管具体业务的文案与落表时机——换装、感情档位。
//!
//! 两者都必须经 [`PromptLine`] 构造，不手工拼 `LineBase`：`line.rs` 开头那几条
//! 不变量只在那一个入口里保证一次，散落的手工构造迟早会漏掉其中一条。

use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::utils::prompt::PromptRole;

use super::PromptLine;

impl GameStatus {
    /// 播一条「感情档位」旁白。
    pub async fn add_affection_state_narration(
        &mut self,
        db: &DatabaseConnection,
        role_name: &str,
        value: i32,
    ) -> Result<()> {
        let v = value.clamp(-100, 100);
        let vector = crate::ai_service::affection::uniform_vector(v);
        let text = crate::ai_service::affection::describe_change_for_line(
            role_name,
            &self.player.user_name,
            &vector,
            &crate::ai_service::types::NegativeVector::default(),
        );
        self.add_line(
            db,
            PromptLine::new(PromptRole::Narrator, text)
                .display_name("系统")
                .into_base(),
        )
        .await
    }

    /// 把角色当前服装改成 `clothes_name`，并补一条换装旁白。
    pub async fn add_character_clothes_change_line(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        clothes_name: &str,
    ) -> Result<()> {
        let role = self
            .role_manager
            .get_loaded_mut(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {} 未加载", role_id))?;

        role.current_clothes = clothes_name.to_string();

        let ai_name = role.settings.ai_name.clone();
        let clothes_prompt = role
            .settings
            .clothes
            .as_ref()
            .and_then(|list| {
                list.iter().find_map(|item| {
                    if item.get("name").map(|s| s.as_str()) == Some(clothes_name) {
                        item.get("prompt").cloned()
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        let prompt = format!(
            "{}换上了新服装：{}，{}",
            ai_name, clothes_name, clothes_prompt
        );

        self.add_line(
            db,
            PromptLine::new(PromptRole::Narrator, prompt).into_base(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("添加换装台词失败: {}", e))?;

        Ok(())
    }

    /// 切换角色服装并生成旁白台词。
    /// 若已是目标服装则跳过。返回是否实际切换。
    pub async fn on_character_change_clothes(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        clothes_name: &str,
    ) -> Result<bool> {
        let role = self
            .role_manager
            .get_loaded_mut(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {} 未加载", role_id))?;

        if role.current_clothes == clothes_name {
            return Ok(false);
        }

        self.add_character_clothes_change_line(db, role_id, clothes_name)
            .await?;

        Ok(true)
    }
}
