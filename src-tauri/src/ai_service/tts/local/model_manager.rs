// CRUD for installed local TTS voices and assets.

use serde::Serialize;

use super::paths::LocalTtsPaths;

#[derive(Debug, Clone, Serialize)]
pub struct VoiceRecord {
    pub voice_id: String,
    pub kind: String,        // "sbv2" | "onnx"
    pub size_bytes: u64,
    pub path: String,
    pub language: Option<String>,
    pub display_name: Option<String>,
    pub source: Option<String>,
    pub has_style_vectors: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssetRecord {
    pub asset_id: String,
    pub kind: String,        // "bert"
    pub size_bytes: u64,
    pub path: String,
    pub language: Option<String>,
    pub display_name: Option<String>,
    pub source: Option<String>,
}

pub fn list_voices(
    paths: &LocalTtsPaths,
) -> std::result::Result<Vec<VoiceRecord>, String> {
    let mut out = vec![];
    if !paths.voices.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&paths.voices)
        .map_err(|e| format!("read_dir: {e}"))?
    {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let voice_id = entry.file_name().to_string_lossy().into_owned();
        let sbv2 = path.join("model.sbv2");
        let onnx = path.join("model.onnx");
        let (primary, kind) = if sbv2.exists() {
            (sbv2, "sbv2")
        } else if onnx.exists() {
            (onnx, "onnx")
        } else {
            continue;
        };
        let style_path = path.join("style_vectors.json");
        let has_style_vectors = kind == "sbv2" || style_path.exists();
        let size = std::fs::metadata(&primary).map(|m| m.len()).unwrap_or(0);
        let catalog_match = super::registry::find(&voice_id);
        out.push(VoiceRecord {
            voice_id: voice_id.clone(),
            kind: kind.into(),
            size_bytes: size,
            path: primary.to_string_lossy().into_owned(),
            language: catalog_match.as_ref().map(|a| a.language.clone()),
            display_name: catalog_match.as_ref().map(|a| a.display_name.clone()),
            source: catalog_match.as_ref().map(|a| a.source.clone()),
            has_style_vectors,
        });
    }
    Ok(out)
}

pub fn list_assets(
    paths: &LocalTtsPaths,
) -> std::result::Result<Vec<AssetRecord>, String> {
    let mut out = vec![];
    let dir = paths.deberta_dir();
    if dir.exists() {
        let onnx = dir.join("deberta.onnx");
        let tok = dir.join("tokenizer.json");
        if onnx.exists() && tok.exists() {
            let size = std::fs::metadata(&onnx).map(|m| m.len()).unwrap_or(0)
                + std::fs::metadata(&tok).map(|m| m.len()).unwrap_or(0);
            out.push(AssetRecord {
                asset_id: "deberta".into(),
                kind: "bert".into(),
                size_bytes: size,
                path: onnx.to_string_lossy().into_owned(),
                language: Some("ja".into()),
                display_name: Some("DeBERTa-v3-base (Japanese BERT)".into()),
                source: Some("ku-nlp/deberta-v3-base-japanese".into()),
            });
        }
    }
    Ok(out)
}

pub fn delete_voice(
    paths: &LocalTtsPaths,
    voice_id: &str,
) -> std::result::Result<(), String> {
    validate_voice_id(voice_id)?;
    let p = paths.voice_dir(voice_id);
    if !p.exists() {
        return Ok(());
    }
    crate::api::validate_path_in_base(&p, &paths.voices)?;
    std::fs::remove_dir_all(&p).map_err(|e| format!("remove_dir_all: {e}"))
}

fn validate_voice_id(id: &str) -> std::result::Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        return Err("voice id length out of range".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("voice id must be kebab-case ASCII".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_voice_id("../etc").is_err());
        assert!(validate_voice_id("").is_err());
        assert!(validate_voice_id("good-voice_1").is_ok());
    }
}
