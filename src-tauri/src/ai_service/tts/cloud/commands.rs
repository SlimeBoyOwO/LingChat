//! CosyVoice 相关 Tauri commands。

use std::sync::OnceLock;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tokio::sync::Mutex as AsyncMutex;

use super::CloudVoiceService;
use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::keys;
use crate::config::tts::{CosyVoiceRecord, TtsConfig};

/// 串行化 `COSYVOICE_VOICES` 的读-改-写（整数组覆写 settings.json）。
/// 前端会并发轮询多个音色，若不互斥，后写者会用旧快照覆盖先写者的状态更新，
/// 导致本地缓存永远停在 deploying。
static VOICES_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
fn voices_lock() -> &'static AsyncMutex<()> {
    VOICES_LOCK.get_or_init(|| AsyncMutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosyvoiceConfig {
    pub api_key_configured: bool,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosyVoiceView {
    pub voice_id: String,
    pub name: String,
    pub model: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CosyvoiceProgress {
    pub phase: String,
}

fn service(app: &AppHandle) -> Result<CloudVoiceService> {
    let cfg = TtsConfig::load(app);
    let key = cfg.cosyvoice_api_key.unwrap_or_default();
    if key.trim().is_empty() {
        return Err(anyhow::anyhow!("CosyVoice API Key 未配置，请在设置中填写"));
    }
    Ok(CloudVoiceService::new(key))
}

fn read_voice_records(app: &AppHandle) -> Vec<CosyVoiceRecord> {
    TtsConfig::load(app).cosyvoice_voices
}

fn write_voice_records(app: &AppHandle, records: Vec<CosyVoiceRecord>) -> Result<()> {
    let store = crate::config::settings_store(app)?;
    store.set(keys::COSYVOICE_VOICES, serde_json::json!(records));
    store.save()?;
    Ok(())
}
#[tauri::command]
pub async fn cosyvoice_get_config(app: AppHandle) -> Result<CosyvoiceConfig, String> {
    let cfg = TtsConfig::load(&app);
    Ok(CosyvoiceConfig {
        api_key_configured: cfg
            .cosyvoice_api_key
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false),
        models: cfg.cosyvoice_models,
    })
}

#[tauri::command]
pub async fn cosyvoice_save_api_key(app: AppHandle, api_key: String) -> Result<(), String> {
    let store = crate::config::settings_store(&app).map_err(|e| e.to_string())?;
    store.set(keys::COSYVOICE_API_KEY, serde_json::json!(api_key));
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cosyvoice_create_voice(
    app: AppHandle,
    name: String,
    model: String,
    file_path: String,
    language: String,
    channel: tauri::ipc::Channel<CosyvoiceProgress>,
) -> Result<CosyVoiceRecord, String> {
    // Android 上 dialog 返回 content:// URI,需先经 SAF bridge 复制到本地 cache
    //（桌面端原样返回路径;staged 文件用完必须删除）
    let source =
        crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &file_path)
            .await
            .map_err(|e| format!("读取语音样本失败: {e}"))?;
    let path = source.path;

    let result = async {
        // 上传大小限制 20MB（与参考实现一致）
        const MAX_SAMPLE_BYTES: u64 = 20 * 1024 * 1024;
        let meta = std::fs::metadata(&path).map_err(|e| format!("读取语音样本失败: {e}"))?;
        if meta.len() > MAX_SAMPLE_BYTES {
            return Err(format!(
                "语音样本超过 20MB 限制（当前 {:.1}MB）",
                meta.len() as f64 / (1024.0 * 1024.0)
            ));
        }
        let language = if language.trim().is_empty() {
            "zh"
        } else {
            language.trim()
        };
        let svc = service(&app).map_err(|e| e.to_string())?;
        let record = svc
            .submit_from_file(&model, &name, &path, language, &|phase: &str| {
                let _ = channel.send(CosyvoiceProgress {
                    phase: phase.to_string(),
                });
            })
            .await
            .map_err(|e| e.to_string())?;
        upsert_voice_record(&app, &record).await.map_err(|e| e.to_string())?;
        Ok(record)
    }
    .await;

    // SAF staged 文件无论成功失败都要清理
    if source.cleanup_after_import {
        let _ = tokio::fs::remove_file(&path).await;
    }
    result
}

/// 查询单音色审核状态（小写），结果写回本地缓存；未注册过该音色也照常查询。
#[tauri::command]
pub async fn cosyvoice_voice_status(app: AppHandle, voice_id: String) -> Result<String, String> {
    let svc = service(&app).map_err(|e| e.to_string())?;
    let status = svc.status(&voice_id).await.map_err(|e| e.to_string())?;
    // 锁内完成读-改-写，避免多个音色并发轮询时互相覆盖状态
    let _guard = voices_lock().lock().await;
    let mut records = read_voice_records(&app);
    if let Some(record) = records.iter_mut().find(|r| r.voice_id == voice_id) {
        if record.status.as_deref() != Some(status.as_str()) {
            tracing::info!("CosyVoice 音色状态更新: {voice_id} -> {status}");
        }
        record.status = Some(status.clone());
        write_voice_records(&app, records).map_err(|e| e.to_string())?;
    }
    Ok(status)
}

#[tauri::command]
pub async fn cosyvoice_list_voices(app: AppHandle) -> Result<Vec<CosyVoiceView>, String> {
    // 本地记录为权威列表（参考已验证实现）：注册时 add、删除时 remove、轮询更新状态，
    // 不依赖云端 list_voice 接口
    let records = read_voice_records(&app);
    Ok(records
        .iter()
        .map(|r| CosyVoiceView {
            voice_id: r.voice_id.clone(),
            name: r.name.clone(),
            model: r.model.clone(),
            status: r.status.clone(),
        })
        .collect())
}

#[tauri::command]
pub async fn cosyvoice_delete_voice(app: AppHandle, voice_id: String) -> Result<(), String> {
    // 云端删除失败不阻断本地移除
    if let Ok(svc) = service(&app) {
        let _ = svc.delete(&voice_id).await;
    }
    let _guard = voices_lock().lock().await;
    let mut records = read_voice_records(&app);
    records.retain(|r| r.voice_id != voice_id);
    write_voice_records(&app, records).map_err(|e| e.to_string())
}

/// 试听前自愈检查：缓存状态非 "ok" 时实时查一次云端，通过才放行。
/// 防止「页面关着时审核已通过，缓存仍是 deploying」导致误拒。
fn needs_live_status_check(status: Option<&str>) -> bool {
    !matches!(status, Some("ok"))
}

#[tauri::command]
pub async fn cosyvoice_synthesize_preview(
    app: AppHandle,
    model: String,
    voice_id: String,
    text: String,
) -> Result<Vec<u8>, String> {
    tracing::info!(
        "CosyVoice 试听请求: model={} voice={} text_len={}",
        model,
        voice_id,
        text.chars().count()
    );
    let svc = service(&app).map_err(|e| e.to_string())?;

    // 自愈：本地缓存非 ok → 实时查一次云端；本地没有该音色 → 拒绝（参考实现 404 语义）
    let records = read_voice_records(&app);
    let cached = records
        .iter()
        .find(|r| r.voice_id == voice_id)
        .ok_or_else(|| format!("音色不在本地列表中: {voice_id}"))?;
    // 合成模型必须与注册音色时的模型一致，防前端传错模型被云端拒绝
    if model != cached.model {
        return Err(format!(
            "音色 {voice_id} 注册模型为 {}，与请求模型 {model} 不一致",
            cached.model
        ));
    }
    if needs_live_status_check(cached.status.as_deref()) {
        tracing::info!(
            "CosyVoice 试听自愈: 缓存状态={:?}，实时查询云端",
            cached.status
        );
        let live = svc
            .status(&voice_id)
            .await
            .map_err(|e| format!("查询音色状态失败: {e}"))?;
        tracing::info!("CosyVoice 试听自愈结果: {live}");
        let _guard = voices_lock().lock().await;
        let mut records = read_voice_records(&app);
        if let Some(r) = records.iter_mut().find(|r| r.voice_id == voice_id) {
            r.status = Some(live.clone());
            write_voice_records(&app, records).map_err(|e| e.to_string())?;
        }
        if live != "ok" {
            return Err(format!("音色尚未可用（status={live}），无法合成"));
        }
    } else {
        tracing::debug!("CosyVoice 试听: 缓存状态 ok，直接合成");
    }

    // 复用 adapter 的合成逻辑（与对话链路同构）
    let adapter = crate::ai_service::tts::adapters::cosyvoice::CosyvoiceAdapter::new(
        svc.api_key().to_string(),
        model,
        voice_id,
    );
    let result = adapter.generate_voice(&text, "").await;
    match &result {
        Ok(bytes) => tracing::info!("CosyVoice 试听合成成功: {} bytes", bytes.len()),
        Err(e) => tracing::warn!("CosyVoice 试听合成失败: {e}"),
    }
    result.map_err(|e| e.to_string())
}

/// 新增或更新一条音色记录（按 voice_id 去重）。
async fn upsert_voice_record(app: &AppHandle, record: &CosyVoiceRecord) -> Result<()> {
    let _guard = voices_lock().lock().await;
    let mut records = read_voice_records(app);
    records.retain(|r| r.voice_id != record.voice_id);
    records.push(record.clone());
    write_voice_records(app, records)
}
