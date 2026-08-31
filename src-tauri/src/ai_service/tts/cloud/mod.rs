//! CosyVoice 云端音色服务：注册（提交即返回，审核异步）/状态查询/列表/删除。
//!
//! 审核流程（参考 N.E.K.O. 四层模型）：
//! - 提交 create_voice 后立即返回，本地缓存 status="deploying"
//! - 前端定时轮询 [`status`]，结果写回缓存（settings.json）
//! - 合成前自愈：缓存非 "ok" 时实时查一次，通过才放行

pub mod commands;
pub mod enrollment;
#[cfg(test)]
pub mod enrollment_test;
pub mod upload;

use std::path::Path;

use anyhow::Result;

use crate::config::tts::CosyVoiceRecord;

pub use enrollment::*;
pub use upload::*;

/// 音色名 sanitize 成 ASCII 字母数字前缀（官方要求仅数字字母 ≤10 字符）；
/// 中文等不可转写字符会被滤掉，全滤空时回退 "voice"。
/// voice_id 格式：{target_model}-{prefix}-{唯一标识}。
fn sanitize_prefix(name: &str) -> String {
    let out: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(10)
        .collect::<String>()
        .to_lowercase();
    if out.is_empty() {
        "voice".to_string()
    } else {
        out
    }
}

/// 云端音色服务：注册（提交即返回，审核异步）+ 状态查询 + 列表 + 删除。
#[derive(Debug, Clone)]
pub struct CloudVoiceService {
    api_key: String,
}

impl CloudVoiceService {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// 本地文件注册：上传 OSS → create_voice → 立即返回（status="deploying"，审核异步）。
    pub async fn submit_from_file(
        &self,
        model: &str,
        name: &str,
        file_path: &Path,
        language: &str,
        progress: impl Fn(&str),
    ) -> Result<CosyVoiceRecord> {
        let prefix = sanitize_prefix(name);
        // progress 回调传 phase key（uploading/submitting/submitted），文案由前端本地化
        progress("uploading");
        let url = upload_audio(&self.api_key, model, file_path).await?;
        progress("submitting");
        let voice_id = create_voice(&self.api_key, model, &prefix, &url, Some(&[language])).await?;
        tracing::info!(
            "CosyVoice 音色已提交审核: model={} prefix={} name={} lang={} voice_id={}",
            model,
            prefix,
            name,
            language,
            voice_id
        );
        progress("submitted");
        Ok(CosyVoiceRecord {
            voice_id,
            name: name.to_string(),
            model: model.to_string(),
            created_at: Some(unix_seconds_str()),
            status: Some("deploying".to_string()),
        })
    }

    /// 查询单音色审核状态（小写：ok / undeployed / deploying…）。
    pub async fn status(&self, voice_id: &str) -> Result<String> {
        let status = query_voice(&self.api_key, voice_id).await?;
        tracing::debug!("CosyVoice 音色状态查询: {voice_id} -> {status}");
        Ok(status.to_lowercase())
    }

    pub async fn delete(&self, voice_id: &str) -> Result<()> {
        delete_voice(&self.api_key, voice_id).await
    }
}

fn unix_seconds_str() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_prefix_ascii_alnum_lowercased() {
        assert_eq!(sanitize_prefix("NuoYi123"), "nuoyi123");
    }

    #[test]
    fn sanitize_prefix_truncates_at_10() {
        assert_eq!(sanitize_prefix("abcdefghijklmnop"), "abcdefghij");
    }

    #[test]
    fn sanitize_prefix_filters_non_ascii() {
        assert_eq!(sanitize_prefix("诺一_One"), "one");
        assert_eq!(sanitize_prefix("诺一"), "voice");
        assert_eq!(sanitize_prefix(""), "voice");
    }
}
