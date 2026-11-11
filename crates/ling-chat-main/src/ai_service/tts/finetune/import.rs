//! 数据集导入：解包压缩包、配对音频与同名 `.txt`、归一化并生成清单。
//!
//! 缺少文本的音频保留在清单中，训练阶段会跳过。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::normalize::normalize_to_wav;

/// 可接受的音频扩展名。
pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "m4a", "aac", "ogg", "opus"];

/// 训练集最低总时长（秒）。
pub const MIN_DATASET_SECONDS: f64 = 300.0;

/// 单条音频时长上限（秒），过长既对齐不准也吃内存。
pub const MAX_CLIP_SECONDS: f64 = 30.0;

/// 数据集里的一条样本。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetItem {
    pub id: String,
    pub wav: String,
    pub text: Option<String>,
    pub duration_secs: f64,
    /// 风格簇；None 表示未聚类或特征提取失败，旧清单缺省按 None 处理。
    #[serde(default)]
    pub style_cluster: Option<u32>,
}

/// 被跳过的素材及原因。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedItem {
    pub name: String,
    pub reason: String,
}

/// 数据集统计。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatasetStats {
    pub total_items: usize,
    pub items_with_text: usize,
    pub total_secs: f64,
    pub min_required_secs: f64,
    pub meets_minimum: bool,
}

/// 数据集清单，落盘为 dataset.json。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub version: u32,
    pub dataset_id: String,
    pub sample_rate: u32,
    pub created_at_unix: u64,
    pub items: Vec<DatasetItem>,
    pub skipped: Vec<SkippedItem>,
    pub stats: DatasetStats,
}

/// 导入结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub dataset_id: String,
    pub root: String,
    pub stats: DatasetStats,
    pub skipped: Vec<SkippedItem>,
    /// metadata.csv 的绝对路径。
    pub metadata_csv: String,
}

/// 微调数据集的根目录。
pub fn finetune_root() -> PathBuf {
    crate::data_dir::get_data_dir().join("sherpa_onnx_finetune")
}

/// 导入压缩包生成数据集；同名数据集存在且 `overwrite=false` 时报错，避免误删。
pub fn import_zip(
    zip: &Path,
    dataset_id: &str,
    sample_rate: u32,
    overwrite: bool,
) -> Result<ImportReport> {
    if !zip.is_file() {
        bail!("压缩包不存在: {}", zip.display());
    }
    if !is_valid_dataset_id(dataset_id) {
        bail!("数据集 id 只能包含字母、数字、下划线和短横线: {dataset_id}");
    }

    let root = finetune_root().join(dataset_id);
    if root.exists() {
        if !overwrite {
            bail!("数据集已存在: {dataset_id}");
        }
        std::fs::remove_dir_all(&root)
            .with_context(|| format!("清理旧数据集失败: {}", root.display()))?;
    }

    let raw = root.join("raw");
    let wavs = root.join("wavs");
    std::fs::create_dir_all(&raw)?;
    std::fs::create_dir_all(&wavs)?;

    let token = tokio_util::sync::CancellationToken::new();
    crate::utils::archive::extract_zip(zip, &raw, &token, &|_| {})
        .map_err(|e| anyhow::anyhow!("解压失败: {e}"))?;

    let mut files = Vec::new();
    collect_files(&raw, &mut files)?;

    // 文本索引：同一目录下同名 .txt。
    let mut texts: HashMap<PathBuf, PathBuf> = HashMap::new();
    for file in &files {
        if has_extension(file, "txt") {
            texts.insert(file.with_extension(""), file.clone());
        }
    }

    let mut audio: Vec<PathBuf> = files
        .into_iter()
        .filter(|file| {
            file.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();
    audio.sort();

    let mut items = Vec::new();
    let mut skipped = Vec::new();
    let mut used_ids: HashMap<String, usize> = HashMap::new();

    for source in audio {
        let name = source
            .strip_prefix(&raw)
            .unwrap_or(&source)
            .to_string_lossy()
            .to_string();

        let base_id = make_id(&source);
        let count = used_ids.entry(base_id.clone()).or_insert(0);
        let id = if *count == 0 {
            base_id.clone()
        } else {
            format!("{base_id}_{}", *count)
        };
        *count += 1;

        let dest = wavs.join(format!("{id}.wav"));
        let report = match normalize_to_wav(&source, &dest, sample_rate) {
            Ok(report) => report,
            Err(error) => {
                skipped.push(SkippedItem {
                    name,
                    reason: format!("归一化失败: {error:#}"),
                });
                continue;
            },
        };

        if report.duration_secs > MAX_CLIP_SECONDS {
            let _ = std::fs::remove_file(&dest);
            skipped.push(SkippedItem {
                name,
                reason: format!(
                    "单条超过 {} 秒（{:.1} 秒），请先切成短句",
                    MAX_CLIP_SECONDS, report.duration_secs
                ),
            });
            continue;
        }

        let text = texts
            .get(&source.with_extension(""))
            .and_then(|path| std::fs::read(path).ok())
            .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
            .filter(|text| !text.is_empty());

        let wav = format!("wavs/{id}.wav");
        items.push(DatasetItem {
            id,
            wav,
            text,
            duration_secs: report.duration_secs,
            style_cluster: None,
        });
    }

    let stats = summarize(&items);
    if items.is_empty() {
        bail!("压缩包里没有可用的音频（支持 {AUDIO_EXTENSIONS:?}）");
    }

    let manifest = DatasetManifest {
        version: 1,
        dataset_id: dataset_id.to_string(),
        sample_rate,
        created_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        items,
        skipped: skipped.clone(),
        stats: stats.clone(),
    };

    let manifest_path = root.join("dataset.json");
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    let metadata_path = root.join("metadata.csv");
    std::fs::write(&metadata_path, build_metadata_csv(&manifest.items))?;

    Ok(ImportReport {
        dataset_id: dataset_id.to_string(),
        root: root.to_string_lossy().to_string(),
        stats,
        skipped,
        metadata_csv: metadata_path.to_string_lossy().to_string(),
    })
}

/// 读取已存在的数据集清单。
pub fn load_manifest(dataset_id: &str) -> Result<DatasetManifest> {
    let path = finetune_root().join(dataset_id).join("dataset.json");
    let bytes =
        std::fs::read(&path).with_context(|| format!("数据集清单不存在: {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// 列出所有已导入的数据集 id。
pub fn list_datasets() -> Result<Vec<String>> {
    let root = finetune_root();
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

/// 删除一个数据集。
pub fn remove_dataset(dataset_id: &str) -> Result<()> {
    if !is_valid_dataset_id(dataset_id) {
        bail!("非法数据集 id: {dataset_id}");
    }
    let root = finetune_root().join(dataset_id);
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    Ok(())
}

/// 文本修改后重算统计并写回 dataset.json 与 metadata.csv，避免统计与文本脱节。
pub fn persist_text_updates(root: &Path, manifest: &mut DatasetManifest) -> Result<DatasetStats> {
    let stats = summarize(&manifest.items);
    manifest.stats = stats.clone();
    std::fs::write(
        root.join("dataset.json"),
        serde_json::to_vec_pretty(manifest)?,
    )?;
    std::fs::write(
        root.join("metadata.csv"),
        build_metadata_csv(&manifest.items),
    )?;
    Ok(stats)
}

pub(crate) fn summarize(items: &[DatasetItem]) -> DatasetStats {
    let total_secs: f64 = items.iter().map(|item| item.duration_secs).sum();
    let items_with_text = items.iter().filter(|item| item.text.is_some()).count();
    DatasetStats {
        total_items: items.len(),
        items_with_text,
        total_secs,
        min_required_secs: MIN_DATASET_SECONDS,
        meets_minimum: total_secs >= MIN_DATASET_SECONDS && items_with_text == items.len(),
    }
}

/// 生成 ljspeech 风格的清单：wavs/<id>.wav|文本|speaker。
fn build_metadata_csv(items: &[DatasetItem]) -> String {
    let mut out = String::new();
    for item in items {
        let Some(text) = item.text.as_deref() else {
            continue;
        };
        // 管道符和换行会破坏分隔格式，训练文本里本来也不该有。
        let text = text.replace(&['\r', '\n', '|'][..], " ");
        out.push_str(&format!("{}|{}|default\n", item.wav, text));
    }
    out
}

fn is_valid_dataset_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

fn make_id(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "clip".to_string());
    let sanitized: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "clip".to_string()
    } else {
        sanitized
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_id_validation() {
        assert!(is_valid_dataset_id("voice_01"));
        assert!(is_valid_dataset_id("a-b"));
        assert!(!is_valid_dataset_id(""));
        assert!(!is_valid_dataset_id("a/b"));
        assert!(!is_valid_dataset_id("../x"));
        assert!(!is_valid_dataset_id("有中文"));
    }

    #[test]
    fn metadata_skips_items_without_text() {
        let items = vec![
            DatasetItem {
                id: "a".into(),
                wav: "wavs/a.wav".into(),
                text: Some("你好".into()),
                duration_secs: 1.0,
                style_cluster: None,
            },
            DatasetItem {
                id: "b".into(),
                wav: "wavs/b.wav".into(),
                text: None,
                duration_secs: 1.0,
                style_cluster: None,
            },
        ];
        let csv = build_metadata_csv(&items);
        assert!(csv.contains("wavs/a.wav|你好|default"));
        assert!(!csv.contains("wavs/b.wav"));
    }

    #[test]
    fn stats_require_every_item_to_have_text() {
        let items = vec![DatasetItem {
            id: "a".into(),
            wav: "wavs/a.wav".into(),
            text: Some("你好".into()),
            duration_secs: MIN_DATASET_SECONDS,
            style_cluster: None,
        }];
        assert!(summarize(&items).meets_minimum);

        let items = vec![DatasetItem {
            id: "a".into(),
            wav: "wavs/a.wav".into(),
            text: None,
            duration_secs: MIN_DATASET_SECONDS,
            style_cluster: None,
        }];
        assert!(!summarize(&items).meets_minimum);
    }

    #[test]
    fn id_sanitizes_unsafe_characters() {
        assert_eq!(make_id(Path::new("a b/c.wav")), "c");
        assert_eq!(make_id(Path::new("你好 世界.wav")), "你好_世界");
    }
}
