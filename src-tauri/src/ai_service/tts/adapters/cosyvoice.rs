//! CosyVoice 云端语音合成适配器。
//!
//! 官方端点：`POST /api/v1/services/audio/tts/SpeechSynthesizer`。
//! 注意：非流式响应是 **JSON**（含 `output.audio.url` 与 `finish_reason`），
//! 不是直接音频字节——需校验 finish_reason 后提取 url 再 GET 下载（参考已验证实现）。

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value as JsonValue, json};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

const BASE_URL: &str = "https://dashscope.aliyuncs.com/api/v1";
const SYNTHESIS_PATH: &str = "/services/audio/tts/SpeechSynthesizer";

#[derive(Debug, Clone)]
pub struct CosyvoiceAdapter {
    api_key: String,
    model: String,
    voice_id: String,
    /// 合成目标语言（如 "zh"/"ja"…，提升数字/小语种合成效果；None = 自动检测）
    language_hints: Option<String>,
    /// 方言/指令控制（如"用四川话说话。"，仅复刻音色支持；None = 不指定）
    instruction: Option<String>,
}

impl CosyvoiceAdapter {
    pub fn new(api_key: String, model: String, voice_id: String) -> Self {
        Self {
            api_key,
            model,
            voice_id,
            language_hints: None,
            instruction: None,
        }
    }

    /// 指定合成目标语言（官方 language_hints 参数，如 "zh"/"en"/"ja"）。
    pub fn with_language_hints(mut self, lang: &str) -> Self {
        self.language_hints = Some(lang.to_string());
        self
    }

    /// 指定方言/指令（如"用四川话说话。"——复刻音色需指令才输出方言）。
    pub fn with_instruction(mut self, instruction: &str) -> Self {
        if !instruction.trim().is_empty() {
            self.instruction = Some(instruction.to_string());
        }
        self
    }
}

#[async_trait]
impl TtsAdapter for CosyvoiceAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            return Err(anyhow!("CosyVoice 输入文本为空"));
        }
        tracing::debug!(
            "CosyVoice synthesize model={} voice={}",
            self.model,
            self.voice_id
        );

        let mut input = serde_json::Map::new();
        input.insert("text".into(), json!(text));
        input.insert("voice".into(), json!(self.voice_id));
        input.insert("format".into(), json!("wav"));
        input.insert("sample_rate".into(), json!(24000));
        input.insert("volume".into(), json!(50));
        input.insert("rate".into(), json!(1.0));
        input.insert("pitch".into(), json!(1.0));
        if let Some(lang) = &self.language_hints {
            input.insert("language_hints".into(), json!([lang]));
        }
        if let Some(instruction) = &self.instruction {
            input.insert("instruction".into(), json!(instruction));
        }
        let body = json!({
            "model": self.model,
            "input": input,
        });
        let resp = http_client()
            .post(format!("{BASE_URL}{SYNTHESIS_PATH}"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body: JsonValue = resp.json().await.unwrap_or_default();
            let body_str = body.to_string();
            let code = body["code"].as_str().unwrap_or("HTTP_ERROR");
            let message = body["message"].as_str().unwrap_or(&body_str);
            return Err(anyhow!(
                "CosyVoice 合成失败: {code}: {message} (HTTP {status})"
            ));
        }
        let body: JsonValue = resp.json().await?;
        // finish_reason 非 stop 说明合成未完成（截断/错误）
        if let Some(f) = body["output"]["finish_reason"].as_str() {
            if f != "stop" {
                return Err(anyhow!("CosyVoice 合成未完成: finish_reason={f}"));
            }
        }
        let audio_url = body["output"]["audio"]["url"]
            .as_str()
            .ok_or_else(|| anyhow!("CosyVoice 响应缺少 output.audio.url: {body}"))?;
        let started = std::time::Instant::now();
        let bytes = http_client()
            .get(audio_url)
            .send()
            .await?
            .bytes()
            .await?
            .to_vec();
        tracing::debug!(
            "CosyVoice 合成完成: {} bytes, 耗时 {:.1}s",
            bytes.len(),
            started.elapsed().as_secs_f64()
        );
        Ok(bytes)
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("model".into(), json!(self.model));
        m.insert("voice_id".into(), json!(self.voice_id));
        m
    }
}
