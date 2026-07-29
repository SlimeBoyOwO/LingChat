// Curated asset catalog. URLs are reference only - user triggers downloads
// explicitly from the UI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: String,
    pub kind: AssetKind,
    pub display_name: String,
    pub language: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub source: String,
    /// Required when `kind == StyleVectors`; identifies the parent voice
    /// directory under `<paths.voices>/<voice_id>/` that the downloaded
    /// `style_vectors.json` should land in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Bert,
    Voice,
    StyleVectors,
}

// Hardcoded catalog. Real project should seed from JSON; kept inline to
// avoid an extra bundled file.
pub fn catalog() -> Vec<AssetEntry> {
    vec![
        AssetEntry {
            id: "deberta".into(),
            kind: AssetKind::Bert,
            display_name: "DeBERTa-v3-base (Japanese BERT)".into(),
            language: "ja".into(),
            size_bytes: 278_000_000,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/deberta.onnx".into(),
            source: "lingchat-research-studio/DeBERTa.onnx".into(),
            voice_id: None,
        },
        AssetEntry {
            id: "deberta-tokenizer".into(),
            kind: AssetKind::Bert,
            display_name: "DeBERTa-v3-base Tokenizer".into(),
            language: "ja".into(),
            size_bytes: 2_100_000,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/DeBERTa.onnx/resolve/master/tokenizer.json".into(),
            source: "lingchat-research-studio/DeBERTa.onnx".into(),
            voice_id: None,
        },
        AssetEntry {
            id: "ling-v2".into(),
            kind: AssetKind::Voice,
            display_name: "Ling-v2 (Japanese)".into(),
            language: "ja".into(),
            size_bytes: 249_000_000,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/sbv2api-model-Ling-v2-onnx/resolve/master/sbv2api-model-Ling-v2-onnx.onnx".into(),
            source: "lingchat-research-studio/sbv2api-model-Ling-v2-onnx".into(),
            voice_id: None,
        },
        AssetEntry {
            id: "ling-v2-style".into(),
            kind: AssetKind::StyleVectors,
            display_name: "Ling-v2 Style Vectors".into(),
            language: "ja".into(),
            size_bytes: 7_400,
            download_url: "https://www.modelscope.cn/models/lingchat-research-studio/sbv2api-model-Ling-v2-onnx/resolve/master/style_vectors.json".into(),
            source: "lingchat-research-studio/sbv2api-model-Ling-v2-onnx".into(),
            voice_id: Some("ling-v2".into()),
        },
    ]
}

pub fn find(id: &str) -> Option<AssetEntry> {
    catalog().into_iter().find(|a| a.id == id)
}

pub fn all_assets() -> Vec<AssetEntry> {
    catalog()
}

pub fn expected_extension(entry: &AssetEntry) -> &'static str {
    if entry.download_url.ends_with(".zip") {
        "zip"
    } else if entry.download_url.ends_with(".json") {
        "json"
    } else if entry.download_url.ends_with(".onnx") {
        "onnx"
    } else if entry.download_url.ends_with(".7z") {
        "7z"
    } else {
        "bin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_uses_modelscope_for_deberta_and_ling_v2() {
        let c = catalog();
        let deberta = c
            .iter()
            .find(|a| a.id == "deberta")
            .expect("deberta entry present");
        assert_eq!(deberta.kind, AssetKind::Bert);
        assert!(deberta.download_url.starts_with("https://www.modelscope.cn/"));

        let ling_v2 = c
            .iter()
            .find(|a| a.id == "ling-v2")
            .expect("ling-v2 entry present");
        assert_eq!(ling_v2.kind, AssetKind::Voice);

        let style = c
            .iter()
            .find(|a| a.id == "ling-v2-style")
            .expect("ling-v2-style entry present");
        assert!(matches!(style.kind, AssetKind::StyleVectors));
        assert_eq!(style.voice_id.as_deref(), Some("ling-v2"));
    }

    #[test]
    fn find_returns_some_for_modelscope_ids_only() {
        assert!(find("ling-v2").is_some());
        assert!(find("ling-v2-style").is_some());
        assert!(find("tsukuyomi").is_none());
        assert!(find("amitaro").is_none());
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn expected_extension_handles_onnx_json_and_zip() {
        assert_eq!(expected_extension(&entry_with_url("a.onnx")), "onnx");
        assert_eq!(expected_extension(&entry_with_url("b.json")), "json");
        assert_eq!(expected_extension(&entry_with_url("c.zip")), "zip");
    }

    fn entry_with_url(url_suffix: &str) -> AssetEntry {
        AssetEntry {
            id: "tmp".into(),
            kind: AssetKind::Bert,
            display_name: "tmp".into(),
            language: "ja".into(),
            size_bytes: 0,
            download_url: format!("https://example.com/{url_suffix}"),
            source: "test".into(),
            voice_id: None,
        }
    }
}
