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

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
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
}

impl PoemGameEvent {
    fn from_event_data(data: &Value) -> Self {
        let rounds = data
            .get("rounds")
            .and_then(Value::as_u64)
            .unwrap_or(MAX_ROUNDS as u64)
            .clamp(1, MAX_ROUNDS as u64) as usize;

        Self {
            background_path: string_field(data, "backgroundPath", "深夜诗笺-半页.png"),
            music_path: string_field(data, "musicPath", "4.ogg"),
            glitch_music_path: string_field(data, "glitchMusicPath", "4g.ogg"),
            warm_sticker_path: string_field(data, "warmStickerPath", "写诗Q版-她.png"),
            script_sticker_path: string_field(data, "scriptStickerPath", "写诗Q版-剧本.png"),
            void_sticker_path: string_field(data, "voidStickerPath", "写诗Q版-空白.png"),
            word_list_path: string_field(data, "wordListPath", "poem_words.yaml"),
            result_var: string_field(data, "resultVar", "poem_tone"),
            rounds,
            force_glitch: data.get("glitch").and_then(Value::as_bool),
        }
    }

    fn load_words(&self, script_path: &Path) -> Result<WordListFile> {
        let relative = Path::new(&self.word_list_path);
        if relative.components().count() != 1 || relative.file_name().is_none() {
            return Err(anyhow!("poem_game 的 wordListPath 只能是剧本根目录下的文件名"));
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
        let mut glitch_inserted = false;
        let mut rounds = Vec::with_capacity(self.rounds);

        for round_index in 0..self.rounds {
            let mut options: Vec<PoemWordPayload> = words
                .words
                .choose_multiple(&mut rng, OPTIONS_PER_ROUND)
                .map(|word| word.payload(false))
                .collect();

            // Match the original interaction's per-slot 1/401 anomaly check.
            // We cap it at one pre-generated corrupt word because this client
            // builds all rounds up front rather than one screen at a time.
            if corrupted
                && !glitch_inserted
                && round_index + 1 < self.rounds
                && !words.glitch_words.is_empty()
            {
                for slot in 0..OPTIONS_PER_ROUND {
                    if rng.gen_ratio(1, 401) {
                        if let Some(glitch_word) = words.glitch_words.choose(&mut rng) {
                            options[slot] = glitch_word.payload(true);
                            glitch_inserted = true;
                        }
                        break;
                    }
                }
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

        let payload = PoemGamePayload {
            background_path,
            music_path,
            glitch_music_path,
            warm_sticker_path,
            script_sticker_path,
            void_sticker_path,
            rounds: self.build_rounds(&words, corrupted),
            // DDLC 的原始 loop 标记：普通曲从 19.451s、故障曲从 1.000s 回环。
            normal_loop_start: 19.451,
            glitch_loop_start: 1.0,
        };

        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut channels = ctx.channels.lock().await;
            channels.choice_tx = Some(tx);
            channels.choice_allow_free = false;
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
