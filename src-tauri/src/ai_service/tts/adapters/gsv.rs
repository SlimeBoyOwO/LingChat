//! GPT-SoVITS 适配器，对应 `ling_chat/core/TTS/gsv_adapter.py`。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value as JsonValue, json};
use tokio::sync::{Mutex, OnceCell};
use tokio::time::{Duration, sleep};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

/// GSV 六情绪参考语音的分类名（与立绘系统的情绪文件命名约定一致）。
pub const GSV_EMO_CATEGORIES: [&str; 6] = ["吃惊", "开心", "恐惧", "难过", "生气", "中立"];

/// 把 20 种情绪映射到六分类。未识别的情绪（分类器输出英文等）一律归入中立。
pub fn gsv_emo_category(emo: &str) -> &'static str {
    match emo {
        "惊讶" | "慌张" => "吃惊",
        "高兴" | "兴奋" | "心动" | "调皮" | "害羞" | "自信" => "开心",
        "害怕" | "紧张" | "担心" => "恐惧",
        "哭泣" | "无奈" | "难为情" => "难过",
        "生气" | "厌恶" => "生气",
        _ => "中立",
    }
}

#[derive(Debug, Clone)]
pub struct GsvAdapter {
    api_url: String,
    ref_audio_path: String,
    prompt_text: String,
    prompt_lang: String,
    audio_format: String,
    text_lang: String,
    parallel_infer: bool,
    gpt_model_path: Option<String>,
    sovits_model_path: Option<String>,
    model_initialized: Arc<OnceCell<()>>,
    /// GPT-SoVITS/ROCm cannot safely serve several inference requests at once.
    request_lock: Arc<Mutex<()>>,
    /// 六分类参考提示（开关开启时按情绪分类实时选择）：分类 → (音频路径, 文本, 文本语言)。
    emo_prompts: HashMap<String, (String, String, String)>,
}

impl GsvAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_url: String,
        ref_audio_path: String,
        prompt_text: String,
        prompt_lang: String,
        text_lang: String,
        gpt_model_path: Option<String>,
        sovits_model_path: Option<String>,
        emo_prompts: HashMap<String, (String, String, String)>,
    ) -> Self {
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            ref_audio_path,
            prompt_text,
            prompt_lang,
            audio_format: "wav".into(),
            text_lang,
            parallel_infer: true,
            gpt_model_path,
            sovits_model_path,
            model_initialized: Arc::new(OnceCell::new()),
            request_lock: Arc::new(Mutex::new(())),
            emo_prompts,
        }
    }

    async fn ensure_model_loaded(&self) -> Result<()> {
        let (Some(gpt_model_path), Some(sovits_model_path)) =
            (&self.gpt_model_path, &self.sovits_model_path)
        else {
            return Ok(());
        };
        if gpt_model_path.trim().is_empty() || sovits_model_path.trim().is_empty() {
            return Ok(());
        }

        self.model_initialized
            .get_or_try_init(|| async {
                self.set_model(gpt_model_path, sovits_model_path).await?;
                tracing::info!(
                    gpt_model = %gpt_model_path,
                    sovits_model = %sovits_model_path,
                    "GPT-SoVITS weights loaded"
                );
                Ok::<(), anyhow::Error>(())
            })
            .await?;
        Ok(())
    }

    /// 设置 GPT + SoVITS 权重。对应 Python `set_model`。
    pub async fn set_model(&self, gpt_model_path: &str, sovits_model_path: &str) -> Result<()> {
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
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        // VoiceMaker normally generates segments concurrently. Serialize only
        // GPT-SoVITS so Windows ROCm does not crash after the first segment.
        let _request_guard = self.request_lock.lock().await;
        self.ensure_model_loaded().await?;
        // 六情绪开关开启时按当前片段情绪分类实时切换参考语音与文本（复用立绘
        // 系统的分类思路）；该分类未配置完整时回退到默认 gsv_voice_* 配置。
        let category = gsv_emo_category(emo);
        let (ref_audio_path, prompt_text, prompt_lang) = self
            .emo_prompts
            .get(category)
            .map(|(p, t, l)| (p.as_str(), t.as_str(), l.as_str()))
            .unwrap_or((
                self.ref_audio_path.as_str(),
                self.prompt_text.as_str(),
                self.prompt_lang.as_str(),
            ));
        let body = json!({
            "ref_audio_path": ref_audio_path,
            "prompt_text": prompt_text,
            "prompt_lang": prompt_lang,
            "text_lang": self.text_lang,
            "media_type": self.audio_format,
            "speed_factor": 1.0,
            "text_split_method": "cut0",
            "top_k": 15,
            "top_p": 1.0,
            "temperature": 1.0,
            "parallel_infer": self.parallel_infer,
            "text": text,
        });
        let mut retry_error = None;
        let resp = loop {
            match http_client()
                .post(format!("{}/tts", self.api_url))
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => break resp,
                Err(error) if retry_error.is_none() => {
                    tracing::warn!("GPT-SoVITS request failed, retrying once: {error}");
                    retry_error = Some(error);
                    sleep(Duration::from_millis(500)).await;
                },
                Err(error) => {
                    return Err(anyhow!(
                        "GPT-SoVITS request failed after retry: {error}; first error: {}",
                        retry_error.expect("retry error must exist")
                    ));
                },
            }
        };
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
        m.insert("text_lang".into(), json!(self.text_lang));
        m.insert("gpt_model_path".into(), json!(self.gpt_model_path));
        m.insert("sovits_model_path".into(), json!(self.sovits_model_path));
        m.insert("audio_format".into(), json!(self.audio_format));
        m.insert(
            "emo_prompts".into(),
            json!(self.emo_prompts.keys().cloned().collect::<Vec<_>>()),
        );
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emo_category_covers_all_20_emotions() {
        let cases: &[(&str, &str)] = &[
            ("惊讶", "吃惊"),
            ("慌张", "吃惊"),
            ("高兴", "开心"),
            ("兴奋", "开心"),
            ("心动", "开心"),
            ("调皮", "开心"),
            ("害羞", "开心"),
            ("自信", "开心"),
            ("害怕", "恐惧"),
            ("紧张", "恐惧"),
            ("担心", "恐惧"),
            ("哭泣", "难过"),
            ("无奈", "难过"),
            ("难为情", "难过"),
            ("生气", "生气"),
            ("厌恶", "生气"),
            ("正常", "中立"),
            ("平静", "中立"),
            ("认真", "中立"),
            ("疑惑", "中立"),
        ];
        for (emo, want) in cases {
            assert_eq!(gsv_emo_category(emo), *want, "emotion {emo}");
        }
        // 未知情绪（分类器输出的英文等标签）回退中立
        assert_eq!(gsv_emo_category("AI思考"), "中立");
        assert_eq!(gsv_emo_category("neutral"), "中立");
    }
}
