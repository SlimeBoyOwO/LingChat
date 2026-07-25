// Curated asset catalog. URLs are reference only - user triggers downloads
// explicitly from the UI.

#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: String,
    pub kind: AssetKind,
    pub display_name: String,
    pub language: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub download_url: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Bert,
    Voice,
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
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            download_url: "https://huggingface.co/ku-nlp/deberta-v3-base-japanese/resolve/main/onnx/model.onnx".into(),
            source: "ku-nlp/deberta-v3-base-japanese".into(),
        },
        AssetEntry {
            id: "deberta-tokenizer".into(),
            kind: AssetKind::Bert,
            display_name: "DeBERTa-v3-base Tokenizer".into(),
            language: "ja".into(),
            size_bytes: 2_100_000,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            download_url: "https://huggingface.co/ku-nlp/deberta-v3-base-japanese/resolve/main/tokenizer.json".into(),
            source: "ku-nlp/deberta-v3-base-japanese".into(),
        },
        AssetEntry {
            id: "tsukuyomi".into(),
            kind: AssetKind::Voice,
            display_name: "Tsukuyomi (Japanese)".into(),
            language: "ja".into(),
            size_bytes: 65_000_000,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            download_url: "https://github.com/Style-Bert-VITS2/Style-Bert-VITS2/releases/download/1.0/tsukuyomi.zip".into(),
            source: "Style-Bert-VITS2".into(),
        },
        AssetEntry {
            id: "amitaro".into(),
            kind: AssetKind::Voice,
            display_name: "Amitaro (Japanese)".into(),
            language: "ja".into(),
            size_bytes: 64_000_000,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            download_url: "https://github.com/Style-Bert-VITS2/Style-Bert-VITS2/releases/download/1.0/amitaro.zip".into(),
            source: "Style-Bert-VITS2".into(),
        },
    ]
}

pub fn find(id: &str) -> Option<AssetEntry> {
    catalog().into_iter().find(|a| a.id == id)
}

pub fn all_voices() -> Vec<AssetEntry> {
    catalog()
        .into_iter()
        .filter(|a| matches!(a.kind, AssetKind::Voice))
        .collect()
}

pub fn all_assets() -> Vec<AssetEntry> {
    catalog()
}

pub fn expected_extension(entry: &AssetEntry) -> &'static str {
    if entry.download_url.ends_with(".zip") {
        "zip"
    } else if entry.download_url.ends_with(".7z") {
        "7z"
    } else {
        "bin"
    }
}

// Validate SHA256 against a downloaded blob. Returns true if the catalog
// entry has the placeholder "all zeros" sha256 (skip check when unset).
pub fn sha256_matches(entry: &AssetEntry, path: &Path) -> std::result::Result<bool, String> {
    if entry.sha256.chars().all(|c| c == '0') {
        return Ok(true);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(&bytes);
    let got = format!("{:x}", h.finalize());
    Ok(got.eq_ignore_ascii_case(&entry.sha256))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_deberta_and_two_voices() {
        let c = catalog();
        assert!(c.iter().any(|a| a.id == "deberta" && a.kind == AssetKind::Bert));
        assert!(all_voices().len() >= 2);
    }

    #[test]
    fn find_returns_some_for_known_id() {
        assert!(find("tsukuyomi").is_some());
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn sha256_matches_returns_true_when_unset() {
        let e = find("deberta").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("blob");
        std::fs::write(&p, b"abc").unwrap();
        assert!(sha256_matches(&e, &p).unwrap());
    }
}
