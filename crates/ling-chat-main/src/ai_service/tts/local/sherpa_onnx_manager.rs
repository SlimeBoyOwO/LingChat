//! Sherpa-ONNX 模型目录扫描、完整性校验与类型解析。

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SherpaOnnxModelInfo {
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_type: String,
    pub language: String,
    pub voice: String,
    pub size_bytes: u64,
}

/// 下载成功后写入的完整性清单；扫描时据此判断模型是否完整。
pub const MODEL_MANIFEST_FILE: &str = ".lingchat-model.json";
const MODEL_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct ModelManifest {
    version: u32,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ManifestFile {
    path: String,
    size: u64,
}

/// 清单里的路径必须是模型目录内的相对路径，挡掉绝对路径与 `..` 穿越。
fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.contains('\\') {
        return false;
    }
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// 递归查找 `.part` 临时文件；存在说明下载被中断，模型不完整。
fn has_part_file(dir: &Path) -> Result<bool, String> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|e| format!("读取目录 {} 失败: {e}", current.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
            let file_type = entry
                .file_type()
                .map_err(|e| format!("读取文件类型失败: {e}"))?;
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if entry.file_name().to_string_lossy().ends_with(".part") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 校验模型目录完整性。
///
/// 有清单：逐个核对文件存在且大小一致；没有清单（旧版本下载/用户导入）：
/// 只检查是否有残留 `.part`。前者能发现截断与缺文件，后者兜住中断的旧下载。
pub fn verify_model_dir(dir: &Path) -> Result<(), String> {
    let manifest_path = dir.join(MODEL_MANIFEST_FILE);
    let manifest = match std::fs::read(&manifest_path) {
        Ok(bytes) => {
            let manifest: ModelManifest =
                serde_json::from_slice(&bytes).map_err(|e| format!("模型清单损坏: {e}"))?;
            Some(manifest)
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("读取模型清单失败: {error}")),
    };

    let Some(manifest) = manifest else {
        return if has_part_file(dir)? {
            Err("存在未完成的 .part 文件".to_string())
        } else {
            Ok(())
        };
    };

    if manifest.version != MODEL_MANIFEST_VERSION {
        return Err(format!("模型清单版本不支持: {}", manifest.version));
    }
    if manifest.files.is_empty() {
        return Err("模型清单为空".to_string());
    }

    for file in &manifest.files {
        if !is_safe_relative_path(&file.path) {
            return Err(format!("模型清单包含非法路径: {}", file.path));
        }
        let path = dir.join(&file.path);
        let metadata =
            std::fs::metadata(&path).map_err(|e| format!("缺少文件 {}: {e}", file.path))?;
        if !metadata.is_file() {
            return Err(format!("文件不是普通文件: {}", file.path));
        }
        if file.size > 0 && metadata.len() != file.size {
            return Err(format!(
                "文件 {} 大小不符：期望 {}，实际 {}",
                file.path,
                file.size,
                metadata.len()
            ));
        }
    }
    Ok(())
}

/// 下载收尾：按期望文件表核对落盘结果，再原子写入完整性清单。
///
/// `expected` 为 `(相对路径, 服务端大小)`；大小为 0 表示服务端未知，只校验存在。
pub async fn write_model_manifest(dir: &Path, expected: &[(String, u64)]) -> Result<(), String> {
    let mut files = Vec::with_capacity(expected.len());
    for (rel, expected_size) in expected {
        if !is_safe_relative_path(rel) {
            return Err(format!("模型文件路径非法: {rel}"));
        }
        let path = dir.join(rel);
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|e| format!("下载后缺少文件 {rel}: {e}"))?;
        if !metadata.is_file() {
            return Err(format!("下载结果不是普通文件: {rel}"));
        }
        if *expected_size > 0 && metadata.len() != *expected_size {
            return Err(format!(
                "文件 {rel} 大小不符：期望 {expected_size}，实际 {}",
                metadata.len()
            ));
        }
        files.push(ManifestFile {
            path: rel.clone(),
            size: metadata.len(),
        });
    }

    let manifest = ModelManifest {
        version: MODEL_MANIFEST_VERSION,
        files,
    };
    let bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("序列化模型清单失败: {e}"))?;
    let tmp = dir.join(format!("{MODEL_MANIFEST_FILE}.part"));
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|e| format!("写入模型清单失败: {e}"))?;
    tokio::fs::rename(&tmp, dir.join(MODEL_MANIFEST_FILE))
        .await
        .map_err(|e| format!("提交模型清单失败: {e}"))?;
    Ok(())
}

/// 模型根目录（`<name>/` 直接位于其下）。
/// 所有平台统一用应用数据目录下的 `sherpa_onnx_models`，无需存储权限。
pub fn sherpa_onnx_models_root() -> PathBuf {
    crate::data_dir::get_data_dir().join("sherpa_onnx_models")
}

/// 依据模型目录本身解析实际 `model_type`：内置清单 → `config.json` → 文件特征 → 传入值。
pub fn resolve_model_type(model_dir: &Path, configured: &str) -> String {
    if let Some(mtype) = model_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(crate::api::tts_sherpa::catalog_model_type)
    {
        return mtype.to_string();
    }

    if let Ok(content) = std::fs::read_to_string(model_dir.join("config.json")) {
        if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(mtype) = config
                .get("model_type")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                return mtype.to_string();
            }
        }
    }

    if let Some(mtype) = infer_model_type(model_dir) {
        return mtype.to_string();
    }

    let configured = configured.trim();
    if configured.is_empty() {
        "vits".to_string()
    } else {
        configured.to_string()
    }
}

/// 按目录内文件特征推断 `model_type`；kokoro/kitten 无法区分时按 kokoro。
fn infer_model_type(model_dir: &Path) -> Option<&'static str> {
    let has = |file: &str| model_dir.join(file).exists();

    if has("fm_decoder.onnx") || has("fm_decoder_int8.onnx") {
        return Some("zipvoice");
    }
    if has("lm_main.int8.onnx") || has("lm_main.onnx") || has("lm_flow.onnx") {
        return Some("pocket");
    }
    if has("duration_predictor.onnx") || has("vector_estimator.onnx") {
        return Some("supertonic");
    }
    if has("model-steps-3.onnx") || has("model-steps-6.onnx") {
        return Some("matcha");
    }
    if has("text_encoder.onnx") {
        return Some("zipvoice");
    }
    if has("voices.bin") && (has("model.int8.onnx") || has("model.onnx")) {
        return Some("kokoro");
    }
    if has("model.onnx") || has("tts-model.onnx") || has("sherpa-onnx-tts.onnx") {
        return Some("vits");
    }
    None
}

/// 扫描模型根目录下的所有可识别模型。
///
/// `in_flight` 是正在下载的模型目录名：下载过程中目录里会有 `.part`，
/// 不能当成不完整模型删掉。扫描发现清单/文件不完整时删除整个模型目录。
pub async fn scan_models(
    root: &Path,
    in_flight: &HashSet<String>,
) -> Result<Vec<SherpaOnnxModelInfo>> {
    tokio::fs::create_dir_all(root).await?;
    let mut models = Vec::new();
    let mut entries = tokio::fs::read_dir(root).await?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        // `_cache` 等内部目录不是模型；隐藏目录同样跳过。
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if in_flight.contains(&name) {
            continue;
        }

        // 校验要遍历目录并读大小，放到阻塞池，避免占住 async worker。
        let verify_path = path.clone();
        match tokio::task::spawn_blocking(move || verify_model_dir(&verify_path)).await {
            Ok(Ok(())) => {},
            Ok(Err(reason)) => {
                tracing::warn!("删除不完整的 Sherpa 模型 {}: {reason}", path.display());
                if let Err(error) = tokio::fs::remove_dir_all(&path).await {
                    tracing::warn!("删除不完整模型 {} 失败: {error}", path.display());
                }
                continue;
            },
            Err(error) => {
                // 校验任务异常时保守跳过，不误删。
                tracing::warn!("校验 Sherpa 模型目录 {} 失败: {error}", path.display());
                continue;
            },
        }

        if let Ok(model_info) = parse_model_directory(&path).await {
            models.push(model_info);
        }
    }

    Ok(models)
}

async fn parse_model_directory(model_dir: &Path) -> Result<SherpaOnnxModelInfo> {
    let model_name = model_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Invalid model directory name"))?
        .to_string();

    // 覆盖所有支持的模型类型。
    const MODEL_FILES: &[&str] = &[
        // VITS / FastSpeech2
        "model.onnx",
        "tts-model.onnx",
        "sherpa-onnx-tts.onnx",
        // Matcha
        "model-steps-3.onnx",
        "model-steps-6.onnx",
        // ZipVoice
        "fm_decoder.onnx",
        // Kokoro / Kitten
        "model.int8.onnx",
        // Pocket
        "lm_main.int8.onnx",
        "lm_main.onnx",
        "lm_flow.int8.onnx",
        "lm_flow.onnx",
        // Supertonic
        "text_encoder.onnx",
        "duration_predictor.onnx",
    ];
    let model_path = MODEL_FILES
        .iter()
        .map(|file| model_dir.join(file))
        .find(|candidate| candidate.exists())
        .ok_or_else(|| anyhow!("No model file found in directory"))?;

    let size_bytes = tokio::fs::metadata(&model_path).await?.len();

    // 类型以模型目录本身为准，避免角色设置里的错误类型。
    let config_path = model_dir.join("config.json");
    let (config_type, language, voice) = if config_path.exists() {
        load_model_config(&config_path).await?
    } else {
        (String::new(), "zh".to_string(), "female".to_string())
    };
    let model_type = resolve_model_type(model_dir, &config_type);

    Ok(SherpaOnnxModelInfo {
        model_name,
        model_path,
        model_type,
        language,
        voice,
        size_bytes,
    })
}

async fn load_model_config(config_path: &Path) -> Result<(String, String, String)> {
    let content = tokio::fs::read_to_string(config_path).await?;
    let config: serde_json::Value = serde_json::from_str(&content)?;
    let field = |key: &str, default: &str| {
        config
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or(default)
            .to_string()
    };
    Ok((
        field("model_type", "vits"),
        field("language", "zh"),
        field("voice", "female"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn scans_installed_models() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().join("models");
        let model_dir = root.join("my-vits");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"1234").unwrap();

        let models = scan_models(&root, &HashSet::new()).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_name, "my-vits");
        assert_eq!(models[0].model_type, "vits");
        assert_eq!(models[0].size_bytes, 4);
    }

    /// 清单齐全的模型保留；缺文件/大小不符的模型整目录删除。
    #[tokio::test]
    async fn removes_incomplete_manifest_models() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().join("models");
        let complete = root.join("complete");
        std::fs::create_dir_all(&complete).unwrap();
        std::fs::write(complete.join("model.onnx"), b"1234").unwrap();
        std::fs::write(complete.join("tokens.txt"), b"ab").unwrap();
        write_model_manifest(
            &complete,
            &[("model.onnx".to_string(), 4), ("tokens.txt".to_string(), 2)],
        )
        .await
        .unwrap();

        let broken = root.join("broken");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("model.onnx"), b"1234").unwrap();
        write_model_manifest(
            &broken,
            &[("model.onnx".to_string(), 4), ("tokens.txt".to_string(), 2)],
        )
        .await
        .unwrap_err();
        // 模拟清单已写入但后续文件丢失：直接补一份只含缺失文件的清单。
        let manifest = serde_json::json!({
            "version": MODEL_MANIFEST_VERSION,
            "files": [
                { "path": "model.onnx", "size": 4 },
                { "path": "tokens.txt", "size": 2 }
            ]
        });
        std::fs::write(
            broken.join(MODEL_MANIFEST_FILE),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let models = scan_models(&root, &HashSet::new()).await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_name, "complete");
        assert!(!broken.exists(), "缺文件的模型目录应被删除");
    }

    /// 旧下载没有清单：残留 `.part` 视为不完整并删除。
    #[tokio::test]
    async fn removes_legacy_model_with_part_files() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().join("models");
        let model_dir = root.join("interrupted");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"1234").unwrap();
        std::fs::write(model_dir.join("tokens.txt.part"), b"ab").unwrap();

        let models = scan_models(&root, &HashSet::new()).await.unwrap();
        assert!(models.is_empty());
        assert!(!model_dir.exists(), "有 .part 的模型目录应被删除");
    }

    /// 正在下载的目录即使有 `.part` 也不能被扫描删掉。
    #[tokio::test]
    async fn keeps_in_flight_download_directory() {
        let temp_dir = tempdir().unwrap();
        let root = temp_dir.path().join("models");
        let model_dir = root.join("downloading");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx.part"), b"1234").unwrap();

        let in_flight = HashSet::from(["downloading".to_string()]);
        let models = scan_models(&root, &in_flight).await.unwrap();
        assert!(models.is_empty());
        assert!(model_dir.exists(), "下载中的目录不能被删除");
    }

    /// 清单路径穿越必须被拒绝。
    #[test]
    fn rejects_manifest_path_traversal() {
        let temp_dir = tempdir().unwrap();
        let model_dir = temp_dir.path().join("model");
        std::fs::create_dir_all(&model_dir).unwrap();
        let manifest = serde_json::json!({
            "version": MODEL_MANIFEST_VERSION,
            "files": [{ "path": "../outside.onnx", "size": 4 }]
        });
        std::fs::write(
            model_dir.join(MODEL_MANIFEST_FILE),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let error = verify_model_dir(&model_dir).unwrap_err();
        assert!(error.contains("非法路径"), "{error}");
    }

    #[test]
    fn resolve_model_type_prefers_catalog_dir() {
        // 内置目录名即权威类型，忽略传入的错误默认值。
        let dir = Path::new("/nonexistent/sherpa_onnx_models/kokoro-int8-multi-lang-v1_1");
        assert_eq!(resolve_model_type(dir, "vits"), "kokoro");

        let dir = Path::new("/nonexistent/sherpa_onnx_models/matcha-icefall-zh-baker");
        assert_eq!(resolve_model_type(dir, "vits"), "matcha");
    }

    #[test]
    fn resolve_model_type_infers_from_files() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        // Kokoro/Kitten：voices.bin + model(.int8).onnx
        let kokoro = root.join("my-kokoro");
        std::fs::create_dir_all(&kokoro).unwrap();
        std::fs::write(kokoro.join("model.int8.onnx"), b"x").unwrap();
        std::fs::write(kokoro.join("voices.bin"), b"x").unwrap();
        assert_eq!(resolve_model_type(&kokoro, "vits"), "kokoro");

        // Matcha：model-steps-*.onnx
        let matcha = root.join("my-matcha");
        std::fs::create_dir_all(&matcha).unwrap();
        std::fs::write(matcha.join("model-steps-3.onnx"), b"x").unwrap();
        assert_eq!(resolve_model_type(&matcha, "vits"), "matcha");

        // config.json 声明优先于文件推断。
        let declared = root.join("my-declared");
        std::fs::create_dir_all(&declared).unwrap();
        std::fs::write(declared.join("model.onnx"), b"x").unwrap();
        std::fs::write(declared.join("config.json"), br#"{"model_type":"kitten"}"#).unwrap();
        assert_eq!(resolve_model_type(&declared, "vits"), "kitten");

        // 无法识别时回退到配置值；为空取 vits。
        let plain = root.join("my-plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(resolve_model_type(&plain, "fastspeech2"), "fastspeech2");
        assert_eq!(resolve_model_type(&plain, ""), "vits");
    }
}
