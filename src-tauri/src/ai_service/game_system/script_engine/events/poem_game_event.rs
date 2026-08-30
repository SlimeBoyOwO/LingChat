//! Interactive word-picking poem game.
//!
//! The original inspiration uses twenty rounds of ten words, hidden affinity
//! scores, hopping feedback markers, a looped writing theme, and a rare corrupt
//! word on later playthroughs. This implementation keeps that interaction model
//! while using script-owned words, art, music, and story variables.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use rand::seq::SliceRandom;
use rand::Rng;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::ai_service::game_system::script_engine::events::{
    register_event, PoemSubmissionChannel, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_POEM_GAME, PoemGamePayload, PoemWordPayload,
};
use crate::ai_service::game_system::script_engine::utils::media::{
    resolve_script_media, MediaType,
};
use crate::ai_service::message_system::events::emit;

const OPTIONS_PER_ROUND: usize = 10;
const MAX_ROUNDS: usize = 20;

#[derive(Clone, Deserialize)]
struct WordDefinition {
    text: String,
    #[serde(default)]
    warm: i64,
    #[serde(default, rename = "script")]
    script_score: i64,
    #[serde(default, rename = "void")]
    void_score: i64,
}

impl WordDefinition {
    fn payload(&self, glitch: bool) -> PoemWordPayload {
        PoemWordPayload {
            text: self.text.clone(),
            warm_points: self.warm.clamp(0, 3),
            script_points: self.script_score.clamp(0, 3),
            void_points: self.void_score.clamp(0, 3),
            glitch,
        }
    }
}

#[derive(Deserialize)]
struct WordListFile {
    words: Vec<WordDefinition>,
    #[serde(default)]
    glitch_words: Vec<WordDefinition>,
}

#[derive(Deserialize)]
struct PoemGameResult {
    winner: String,
    #[serde(default)]
    glitch: bool,
    #[serde(default)]
    warm: i64,
    #[serde(default, rename = "script")]
    script_score: i64,
    #[serde(default, rename = "void")]
    void_score: i64,
}

pub struct PoemGameEvent {
    background_path: String,
    music_path: String,
    glitch_music_path: String,
    warm_sticker_path: String,
    script_sticker_path: String,
    void_sticker_path: String,
    word_list_path: String,
    result_var: String,
    rounds: usize,
    force_glitch: Option<bool>,
    mode: String,
}

impl PoemGameEvent {
    fn from_event_data(data: &Value) -> Self {
        let rounds = data
            .get("rounds")
            .and_then(Value::as_u64)
            .unwrap_or(MAX_ROUNDS as u64)
            .clamp(1, MAX_ROUNDS as u64) as usize;

        let requested_mode = string_field(data, "mode", "normal");
        let mode = match requested_mode.as_str() {
            "normal" | "act2" | "act2_final" => requested_mode,
            _ => {
                tracing::warn!(
                    "[PoemGameEvent] 未知写诗模式 '{}'，回退为 normal",
                    requested_mode
                );
                "normal".to_string()
            }
        };

        Self {
            background_path: string_field(data, "backgroundPath", ""),
            music_path: string_field(data, "musicPath", ""),
            glitch_music_path: string_field(data, "glitchMusicPath", ""),
            warm_sticker_path: string_field(data, "warmStickerPath", ""),
            script_sticker_path: string_field(data, "scriptStickerPath", ""),
            void_sticker_path: string_field(data, "voidStickerPath", ""),
            word_list_path: string_field(data, "wordListPath", ""),
            result_var: string_field(data, "resultVar", "poem_tone"),
            rounds,
            force_glitch: data.get("glitch").and_then(Value::as_bool),
            mode,
        }
    }

    fn load_words(&self, script_path: &Path) -> Result<WordListFile> {
        let relative = Path::new(&self.word_list_path);
        if relative.components().count() != 1 || relative.file_name().is_none() {
            return Err(anyhow!(
                "poem_game 的 wordListPath 只能是剧本根目录下的文件名"
            ));
        }

        let path = script_path.join(relative);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("无法读取写诗词库: {}", path.display()))?;
        let words: WordListFile = serde_yaml::from_str(&text)
            .with_context(|| format!("无法解析写诗词库: {}", path.display()))?;
        if words.words.len() < OPTIONS_PER_ROUND {
            return Err(anyhow!(
                "写诗词库至少需要 {} 个普通词，当前只有 {} 个",
                OPTIONS_PER_ROUND,
                words.words.len()
            ));
        }
        Ok(words)
    }

    fn build_rounds(&self, words: &WordListFile, corrupted: bool) -> Vec<Vec<PoemWordPayload>> {
        let mut rng = rand::thread_rng();
        let mut pool: Vec<&WordDefinition> = words.words.iter().collect();
        let mut rounds = Vec::with_capacity(self.rounds);

        for round_index in 0..self.rounds {
            let mut options = Vec::with_capacity(OPTIONS_PER_ROUND);
            for _ in 0..OPTIONS_PER_ROUND {
                // Ren'Py uses randint(0, 400) == 0: 1/401 per visible slot.
                // The twentieth screen is excluded because progress == numWords.
                let is_glitch = corrupted
                    && round_index + 1 < self.rounds
                    && !words.glitch_words.is_empty()
                    && rng.gen_ratio(1, 401);

                if is_glitch {
                    if let Some(glitch_word) = words.glitch_words.choose(&mut rng) {
                        options.push(glitch_word.payload(true));
                        continue;
                    }
                }

                // The original removes every displayed normal word from the
                // per-game pool, not merely the selected word. Refill only for
                // compact third-party word lists; a 200-word list never repeats.
                if pool.is_empty() {
                    pool.extend(words.words.iter());
                }
                let index = rng.gen_range(0..pool.len());
                options.push(pool.swap_remove(index).payload(false));
            }
            rounds.push(options);
        }

        rounds
    }
}

fn string_field(data: &Value, key: &str, default: &str) -> String {
    data.get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .trim()
        .to_string()
}

#[async_trait]
impl ScriptEvent for PoemGameEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let missing_paths: Vec<&str> = [
            ("backgroundPath", self.background_path.as_str()),
            ("musicPath", self.music_path.as_str()),
            ("glitchMusicPath", self.glitch_music_path.as_str()),
            ("warmStickerPath", self.warm_sticker_path.as_str()),
            ("scriptStickerPath", self.script_sticker_path.as_str()),
            ("voidStickerPath", self.void_sticker_path.as_str()),
            ("wordListPath", self.word_list_path.as_str()),
        ]
        .into_iter()
        .filter_map(|(key, value)| value.is_empty().then_some(key))
        .collect();
        if !missing_paths.is_empty() {
            return Err(anyhow!(
                "poem_game 缺少必填字段: {}",
                missing_paths.join(", ")
            ));
        }
        if self.result_var.is_empty() {
            return Err(anyhow!("poem_game 的 resultVar 不能为空"));
        }

        let script = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;
        let words = self.load_words(&script.script_path)?;
        let playthrough = script
            .vars
            .get("playthrough")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        let corrupted = self.force_glitch.unwrap_or(playthrough > 1);

        let background_path = resolve_script_media(
            ctx.data_dir,
            Some(&script.script_path),
            &self.background_path,
            MediaType::Background,
        )
        .ok_or_else(|| anyhow!("写诗背景不存在: {}", self.background_path))?;
        let music_path = resolve_script_media(
            ctx.data_dir,
            Some(&script.script_path),
            &self.music_path,
            MediaType::Music,
        )
        .ok_or_else(|| anyhow!("写诗 BGM 不存在: {}", self.music_path))?;
        let glitch_music_path = resolve_script_media(
            ctx.data_dir,
            Some(&script.script_path),
            &self.glitch_music_path,
            MediaType::Music,
        )
        .ok_or_else(|| anyhow!("写诗故障 BGM 不存在: {}", self.glitch_music_path))?;
        let warm_sticker_path = resolve_script_media(
            ctx.data_dir,
            Some(&script.script_path),
            &self.warm_sticker_path,
            MediaType::Pic,
        )
        .ok_or_else(|| anyhow!("写诗 Q 版角色不存在: {}", self.warm_sticker_path))?;
        let script_sticker_path = resolve_script_media(
            ctx.data_dir,
            Some(&script.script_path),
            &self.script_sticker_path,
            MediaType::Pic,
        )
        .ok_or_else(|| anyhow!("写诗 Q 版角色不存在: {}", self.script_sticker_path))?;
        let void_sticker_path = resolve_script_media(
            ctx.data_dir,
            Some(&script.script_path),
            &self.void_sticker_path,
            MediaType::Pic,
        )
        .ok_or_else(|| anyhow!("写诗 Q 版角色不存在: {}", self.void_sticker_path))?;

        let request_id = Uuid::new_v4().to_string();
        let payload = PoemGamePayload {
            request_id: request_id.clone(),
            background_path,
            music_path,
            glitch_music_path,
            warm_sticker_path,
            script_sticker_path,
            void_sticker_path,
            mode: self.mode.clone(),
            rounds: self.build_rounds(&words, corrupted),
            // DDLC 的原始 loop 标记：普通曲从 19.451s、故障曲从 1.000s 回环。
            normal_loop_start: 19.451,
            glitch_loop_start: 1.0,
        };

        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut channels = ctx.channels.lock().await;
            channels.poem_tx = Some(PoemSubmissionChannel { request_id, tx });
            rx
        };

        let _ = emit(ctx.app, SCRIPT_POEM_GAME, &payload);
        let raw = rx.await.map_err(|_| anyhow!("写诗互动通道已关闭"))?;
        let result: PoemGameResult =
            serde_json::from_str(&raw).context("写诗互动返回了无效结果")?;
        if !matches!(result.winner.as_str(), "warm" | "script" | "void") {
            return Err(anyhow!("写诗互动返回了未知倾向: {}", result.winner));
        }

        let score_cap = (self.rounds as i64) * 3;
        let mut gs = ctx.game_status.lock().await;
        let status = gs
            .script_status
            .as_mut()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;
        status.set_variable(self.result_var.clone(), Value::String(result.winner));
        status.set_variable("poem_glitch", Value::Bool(result.glitch));
        status.set_variable(
            "poem_warm_score",
            Value::from(result.warm.clamp(0, score_cap)),
        );
        status.set_variable(
            "poem_script_score",
            Value::from(result.script_score.clamp(0, score_cap)),
        );
        status.set_variable(
            "poem_void_score",
            Value::from(result.void_score.clamp(0, score_cap)),
        );
        Ok(None)
    }

    fn event_type() -> &'static str {
        "poem_game"
    }
}

pub fn register() {
    register_event(PoemGameEvent::event_type(), |data| {
        Box::new(PoemGameEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::json;

    use super::{PoemGameEvent, WordDefinition, WordListFile, MAX_ROUNDS};

    fn words(count: usize) -> WordListFile {
        WordListFile {
            words: (0..count)
                .map(|index| WordDefinition {
                    text: format!("word-{index}"),
                    warm: 3,
                    script_score: 1,
                    void_score: 0,
                })
                .collect(),
            glitch_words: Vec::new(),
        }
    }

    #[test]
    fn normal_mode_uses_every_word_once_when_pool_is_large_enough() {
        let event = PoemGameEvent::from_event_data(&json!({ "rounds": MAX_ROUNDS }));
        let rounds = event.build_rounds(&words(200), false);
        let texts: Vec<&str> = rounds
            .iter()
            .flatten()
            .map(|word| word.text.as_str())
            .collect();
        let unique: HashSet<&str> = texts.iter().copied().collect();

        assert_eq!(texts.len(), 200);
        assert_eq!(unique.len(), 200);
    }

    #[test]
    fn compact_word_lists_refill_without_short_rounds() {
        let event = PoemGameEvent::from_event_data(&json!({ "rounds": 3 }));
        let rounds = event.build_rounds(&words(10), false);

        assert_eq!(rounds.len(), 3);
        assert!(rounds.iter().all(|round| round.len() == 10));
    }

    #[test]
    fn unknown_mode_falls_back_to_normal() {
        let event = PoemGameEvent::from_event_data(&json!({ "mode": "act9" }));
        assert_eq!(event.mode, "normal");
    }
}
