//! TtsConfig 的 CosyVoice 配置序列化测试。

use crate::config::tts::{default_cosyvoice_models, CosyVoiceRecord, TtsConfig};

#[test]
fn cosyvoice_default_models() {
    assert_eq!(
        default_cosyvoice_models(),
        vec!["cosyvoice-v3.5-flash".to_string()]
    );
}

#[test]
fn cosyvoice_config_serde_roundtrip() {
    let mut cfg = TtsConfig::default();
    cfg.cosyvoice_api_key = Some("sk-test".into());
    cfg.cosyvoice_models = vec![
        "cosyvoice-v3.5-flash".into(),
        "cosyvoice-v3.5-plus".into(),
    ];
    cfg.cosyvoice_voices = vec![CosyVoiceRecord {
        voice_id: "cosyvoice-v3.5-flash-myvoice-abc".into(),
        name: "诺一".into(),
        model: "cosyvoice-v3.5-flash".into(),
        created_at: Some("2026-08-29".into()),
        status: Some("ok".into()),
    }];
    let json = serde_json::to_string(&cfg).unwrap();
    let back: TtsConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.cosyvoice_api_key, Some("sk-test".into()));
    assert_eq!(back.cosyvoice_models.len(), 2);
    assert_eq!(back.cosyvoice_voices[0].name, "诺一");
    assert_eq!(
        back.cosyvoice_voices[0].voice_id,
        "cosyvoice-v3.5-flash-myvoice-abc"
    );
}

#[test]
fn cosyvoice_config_missing_fields_fall_back_to_defaults() {
    let json = r#"{}"#;
    let cfg: TtsConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.cosyvoice_api_key, None);
    assert_eq!(cfg.cosyvoice_models, default_cosyvoice_models());
    assert!(cfg.cosyvoice_voices.is_empty());
}
