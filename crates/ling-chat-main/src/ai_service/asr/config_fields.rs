//! 各 provider 在设置页展示的配置字段。
//!
//! 前端 SettingsAsr.vue 据此动态渲染表单；新增字段只需改这里。

use crate::ai_service::asr::provider_meta::{
    AsrConfigField, AsrConfigFieldOption, ConfigFieldKind,
};
use crate::ai_service::asr::providers::llama::LlamaAsrProvider;
use crate::ai_service::asr::region::DashScopeRegion;

/// 地域下拉的选项（由 [`super::region::DashScopeRegion::ALL`] 投影而来）。
fn region_field_options() -> Vec<AsrConfigFieldOption> {
    DashScopeRegion::ALL
        .iter()
        .map(|r| AsrConfigFieldOption {
            value: r.id(),
            label: r.label(),
        })
        .collect()
}

pub(crate) fn qwen_asr_config_fields() -> Vec<AsrConfigField> {
    vec![
        AsrConfigField {
            key: "region",
            label: "地域",
            kind: ConfigFieldKind::Select,
            required: true,
            default_value: Some(DashScopeRegion::DEFAULT.id()),
            placeholder: None,
            hint: Some(
                "华北2（北京）与新加坡的域名、API Key、模型列表互相独立，不能混用。\
                 下方 API Key 按地域分开保存，切换后显示的是该地域自己的那一份",
            ),
            options: region_field_options(),
        },
        AsrConfigField {
            // 不是 `api_key` 而是按地域分开存的 `api_keys` 映射：北京与新加坡的
            // Key 互相独立，切换地域不能把另一个地域的 Key 冲掉。UI 仍只显示
            // 当前地域的一个框（见 ConfigFieldKind::PasswordMap）。
            key: "api_keys",
            label: "DashScope API Key",
            kind: ConfigFieldKind::PasswordMap,
            required: true,
            default_value: None,
            placeholder: Some("sk-..."),
            // hint 由前端以纯文本渲染（`{{ field.hint }}`），不能写 Markdown 记号
            hint: Some(
                "阿里云百炼（Model Studio）平台 Key，须与所选地域一致。\
                 每个地域单独保存：切换上方地域后，这里显示的是该地域自己的 Key，\
                 不会覆盖另一个地域已填的",
            ),
            options: Vec::new(),
        },
        AsrConfigField {
            key: "endpoint",
            label: "非实时端点",
            kind: ConfigFieldKind::Text,
            required: false,
            // 默认值随地域变化，是派生值而非静态串 —— 由 ProviderInfo.regions
            // 下发、前端在切地域时填入（见 SettingsAsr.vue 的地域 watch）。
            default_value: None,
            placeholder: Some("留空使用所选地域的默认地址"),
            hint: Some(
                "multimodal-generation 端点。填业务空间专属域名可覆盖，形如 \
                 https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation",
            ),
            options: Vec::new(),
        },
        AsrConfigField {
            key: "ws_endpoint",
            label: "实时（流式）端点",
            kind: ConfigFieldKind::Text,
            required: false,
            default_value: None,
            placeholder: Some("留空使用所选地域的默认地址"),
            hint: Some(
                "WebSocket 端点，仅流式模型使用。形如 \
                 wss://{WorkspaceId}.ap-southeast-1.maas.aliyuncs.com/api-ws/v1/inference",
            ),
            options: Vec::new(),
        },
    ]
}

pub(crate) fn llama_asr_config_fields() -> Vec<AsrConfigField> {
    vec![
        AsrConfigField {
            key: "endpoint",
            label: "服务地址",
            kind: ConfigFieldKind::Text,
            required: false,
            default_value: Some(LlamaAsrProvider::DEFAULT_ENDPOINT),
            placeholder: Some("http://127.0.0.1:8080"),
            hint: Some("llama-server 地址（Qwen3-ASR 本地部署）；局域网部署改 http://<IP>:8080"),
            options: Vec::new(),
        },
        AsrConfigField {
            key: "model",
            label: "模型",
            kind: ConfigFieldKind::Text,
            required: false,
            default_value: Some(LlamaAsrProvider::DEFAULT_MODEL),
            placeholder: Some("models/Qwen3-ASR-1.7B-Q8_0.gguf"),
            hint: Some("从上方模型列表选择，或用 /v1/models 查询服务端全名"),
            options: Vec::new(),
        },
        AsrConfigField {
            key: "api_key",
            label: "API Key（可选）",
            kind: ConfigFieldKind::Password,
            required: false,
            default_value: None,
            placeholder: Some("本地服务无需填写"),
            hint: Some("llama-server 带 --api-key 部署时填写，否则留空"),
            options: Vec::new(),
        },
    ]
}
