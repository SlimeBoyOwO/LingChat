//! TTS 适配器 trait + 统一 Provider。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value as JsonValue;

use super::adapters::aivis::AivisAdapter;
use super::adapters::bv2::Bv2Adapter;
use super::adapters::gsv::GsvAdapter;
use super::adapters::indextts::IndexTtsAdapter;
use super::adapters::sbv2::Sbv2Adapter;
use super::adapters::sbv2api::Sbv2ApiAdapter;
use super::adapters::vits::VitsAdapter;

/// TTS 一次合成返回原始音频字节。
#[async_trait]
pub trait TtsAdapter: Send + Sync {
    /// `emo` 参数仅 IndexTTS2 使用，其它 adapter 忽略。
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>>;

    /// 返回当前适配器参数（对应 Python `get_params`）。
    fn get_params(&self) -> HashMap<String, JsonValue>;

    /// 流式语音合成。默认返回 `None`（不支持流式）。
    async fn generate_voice_stream(&self, _text: &str) -> Option<Vec<u8>> {
        None
    }
}

/// TTS Provider：持有所有已初始化的适配器并按 `tts_type` 路由。
/// 对应 Python `TTS` 类。
#[derive(Clone)]
pub struct TtsProvider {
    pub audio_format: String,
    enable: Arc<AtomicBool>,

    pub sva: Option<Arc<VitsAdapter>>,
    pub sbv2: Option<Arc<Sbv2Adapter>>,
    pub sbv2api: Option<Arc<Sbv2ApiAdapter>>,
    pub bv2: Option<Arc<Bv2Adapter>>,
    pub gsv: Option<Arc<GsvAdapter>>,
    pub aivis: Option<Arc<AivisAdapter>>,
    pub indextts: Option<Arc<IndexTtsAdapter>>,
}

impl Default for TtsProvider {
    fn default() -> Self {
        Self {
            audio_format: String::new(),
            enable: Arc::new(AtomicBool::new(true)),
            sva: None,
            sbv2: None,
            sbv2api: None,
            bv2: None,
            gsv: None,
            aivis: None,
            indextts: None,
        }
    }
}

impl std::fmt::Debug for TtsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TtsProvider")
            .field("audio_format", &self.audio_format)
            .field("enable", &self.enable)
            .field("sva", &self.sva.is_some())
            .field("sbv2", &self.sbv2.is_some())
            .field("sbv2api", &self.sbv2api.is_some())
            .field("bv2", &self.bv2.is_some())
            .field("gsv", &self.gsv.is_some())
            .field("aivis", &self.aivis.is_some())
            .field("indextts", &self.indextts.is_some())
            .finish()
    }
}

impl TtsProvider {
    pub fn new(audio_format: impl Into<String>) -> Self {
        Self {
            audio_format: audio_format.into(),
            enable: Arc::new(AtomicBool::new(true)),
            ..Default::default()
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enable.load(Ordering::Relaxed)
    }

    pub fn disable(&self) {
        self.enable.store(false, Ordering::Relaxed);
        log::warn!("TTS 服务已被禁用（请使用 reactivate_tts 重新启用）");
    }

    pub fn reactivate(&self) {
        self.enable.store(true, Ordering::Relaxed);
        log::info!("TTS 服务已重新启用");
    }

    fn select(&self, tts_type: &str) -> Result<Arc<dyn TtsAdapter>> {
        let adapter: Arc<dyn TtsAdapter> = match tts_type {
            "sva-vits" => self
                .sva
                .clone()
                .ok_or_else(|| anyhow!("Vits 适配器未初始化"))?,
            "sbv2" => self
                .sbv2
                .clone()
                .ok_or_else(|| anyhow!("Style-Bert-Vits2 适配器未初始化"))?,
            "sbv2api" => self
                .sbv2api
                .clone()
                .ok_or_else(|| anyhow!("sbv2-api 适配器未初始化"))?,
            "sva-bv2" => self
                .bv2
                .clone()
                .ok_or_else(|| anyhow!("Bert-Vits2 适配器未初始化"))?,
            "gsv" => self
                .gsv
                .clone()
                .ok_or_else(|| anyhow!("GPT-SoVITS 适配器未初始化"))?,
            "aivis" => self
                .aivis
                .clone()
                .ok_or_else(|| anyhow!("AIVIS 适配器未初始化"))?,
            "indextts2" => self
                .indextts
                .clone()
                .ok_or_else(|| anyhow!("IndexTTS2 适配器未初始化"))?,
            "" => {
                // 旧版：未指定时优先 sbv2
                if let Some(a) = self.sbv2.clone() {
                    log::warn!("未指定 tts_type，默认使用 sbv2");
                    a
                } else {
                    return Err(anyhow!("没有可用的 TTS 适配器"));
                }
            }
            other => return Err(anyhow!("未知的 TTS 类型: {other}")),
        };
        Ok(adapter)
    }

    /// 合成并把字节写入 `file_path`。失败时自动禁用 TTS（匹配 Python 行为）。
    pub async fn generate_voice(
        &self,
        text: &str,
        file_path: &std::path::Path,
        tts_type: &str,
        emo: &str,
    ) -> Result<()> {
        if !self.is_enabled() {
            return Err(anyhow!("TTS 服务未启用"));
        }
        if text.trim().is_empty() {
            log::debug!("TTS 输入文本为空，跳过");
            return Ok(());
        }
        let adapter = self.select(tts_type)?;
        match adapter.generate_voice(text, emo).await {
            Ok(bytes) => {
                tokio::fs::write(file_path, &bytes).await?;
                log::debug!("TTS 生成成功: {}", file_path.display());
                Ok(())
            }
            Err(e) => {
                self.disable();
                Err(e)
            }
        }
    }
}
