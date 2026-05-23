//! GPT-SoVITS 适配器，对应 `ling_chat/core/TTS/gsv_adapter.py`。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct GsvAdapter {
    api_url: String,
    ref_audio_path: String,
    prompt_text: String,
    prompt_lang: String,
    audio_format: String,
    text_lang: String,
    parallel_infer: bool,
}

impl GsvAdapter {
    pub fn new(api_url: String, ref_audio_path: String, prompt_text: String, prompt_lang: String) -> Self {
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            ref_audio_path,
            prompt_text,
            prompt_lang,
            audio_format: "wav".into(),
            text_lang: "auto".into(),
            parallel_infer: true,
        }
    }

    /// 设置 GPT + SoVITS 权重。对应 Python `set_model`。
    pub async fn set_model(
        &self,
        gpt_model_path: &str,
        sovits_model_path: &str,
    ) -> Result<()> {
        if !Path::new(gpt_model_path).exists() {
            return Err(anyhow!("GPT 模型文件不存在: {gpt_model_path}"));
        }
        if !Path::new(sovits_model_path).exists() {
            return Err(anyhow!("SoVITS 模型文件不存在: {sovits_model_path}"));
        }
        if !gpt_model_path.ends_with(".ckpt") {
            return Err(anyhow!("GPT 模型扩展名必须为 .ckpt"));
        }
        if !sovits_model_path.ends_with(".pth") {
            return Err(anyhow!("SoVITS 模型扩展名必须为 .pth"));
        }

        let client = http_client();
        let r = client
            .get(format!("{}/set_gpt_weights", self.api_url))
            .query(&[("weights_path", gpt_model_path)])
            .send()
            .await?;
        if !r.status().is_success() {
            return Err(anyhow!("GPT 模型设置失败: HTTP {}", r.status()));
        }

        let r = client
            .get(format!("{}/set_sovits_weights", self.api_url))
            .query(&[("weights_path", sovits_model_path)])
            .send()
            .await?;
        if !r.status().is_success() {
            return Err(anyhow!("SoVITS 模型设置失败: HTTP {}", r.status()));
        }
        Ok(())
    }
}

#[async_trait]
impl TtsAdapter for GsvAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let body = json!({
            "ref_audio_path": self.ref_audio_path,
            "prompt_text": self.prompt_text,
            "prompt_lang": self.prompt_lang,
            "text_lang": self.text_lang,
            "media_type": self.audio_format,
            "speed_factor": 1.0,
            "text_split_method": "cut0",
            "top_k": 15,
            "top_p": 100.0,
            "temperature": 1.0,
            "parallel_infer": self.parallel_infer,
            "text": text,
        });
        let resp = http_client()
            .post(format!("{}/tts", self.api_url))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("GSV 请求失败: HTTP {status}: {text}"));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("ref_audio_path".into(), json!(self.ref_audio_path));
        m.insert("prompt_text".into(), json!(self.prompt_text));
        m.insert("prompt_lang".into(), json!(self.prompt_lang));
        m.insert("audio_format".into(), json!(self.audio_format));
        m
    }
}
