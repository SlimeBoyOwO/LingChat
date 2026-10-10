//! 设置页「嵌入模型」子页的本地模型管理命令：查看状态 / 从本地导入 / 删除。
//!
//! 本地量化模型 **bge-small-zh-v1.5**（`model_int4.onnx` + `embedding_int2.bin` +
//! `tokenizer.json.gz`，约 11MB）不再随仓库 / 安装包分发。用户可在这里从**本地文件夹**
//! 或 **zip / 7z 压缩包**导入，或删除已安装的副本。
//!
//! 模型文件的存在性由 [`crate::ai_service::memory::local_embedding::is_model_available`]
//! **实时**检查，`MemoryService::is_configured` 每次调用都重新判定，因此导入 / 删除
//! 后下一次记忆操作立即在「向量 + 词法混合检索」与「纯词法检索」之间切换，无需重启。
//!
//! 压缩包解压复用 [`crate::utils::archive`]（zip 炸弹防护、路径清洗、条目数上限）。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::memory::local_embedding as local;
use crate::ai_service::tts::local::saf_bridge;

/// 单个模型部件的状态。
#[derive(Clone, Serialize)]
pub struct ModelFileInfo {
    /// 文件名（如 `model_int4.onnx`）。
    pub name: String,
    /// 是否必需（缺失即判定未安装）。
    pub required: bool,
    /// 是否已存在于模型目录。
    pub present: bool,
    /// 文件字节数（不存在时 0）。
    pub size_bytes: u64,
}

/// 本地嵌入模型的状态快照（设置页「嵌入模型」子页展示）。
#[derive(Clone, Serialize)]
pub struct EmbeddingLocalStatus {
    /// 三个必需文件是否齐备。
    pub installed: bool,
    /// 模型存放目录（`data_dir/third_party/bge_small_zh_v1_5`）。
    pub dir: String,
    /// 模型名标识。
    pub model_name: String,
    /// 当前检索模式：`hybrid`（向量 + 词法）或 `lexical`（纯词法）。
    pub retrieval_mode: String,
    /// 是否配置了远程 embeddings 接口（配置了则本地模型是可选的第二选择）。
    pub remote_configured: bool,
    /// 各部件状态。
    pub files: Vec<ModelFileInfo>,
}

/// 读取本地嵌入模型状态。
#[tauri::command]
pub async fn embedding_local_status(app: AppHandle) -> Result<EmbeddingLocalStatus, String> {
    let state = app.state::<AppState>();
    Ok(build_status(&state))
}

/// 从本地文件夹或 zip / 7z 压缩包导入本地嵌入模型。
#[tauri::command]
pub async fn embedding_local_import(
    app: AppHandle,
    source_path: String,
) -> Result<EmbeddingLocalStatus, String> {
    // Android 的 content:// 会被安全地暂存到缓存再返回真实路径。
    let src = saf_bridge::prepare_file_import_source(&app, &source_path).await?;
    let result = import_from_path(&app, &src.path).await;
    if src.cleanup_after_import {
        let _ = tokio::fs::remove_file(&src.path).await;
    }
    result?;
    let state = app.state::<AppState>();
    Ok(build_status(&state))
}

/// 删除已安装的本地嵌入模型（只删模型部件，不删目录里的 README 等其它文件）。
#[tauri::command]
pub async fn embedding_local_delete(app: AppHandle) -> Result<EmbeddingLocalStatus, String> {
    let dir = local::model_dir();
    for name in local::REQUIRED_FILES
        .iter()
        .chain(local::OPTIONAL_FILES.iter())
    {
        let path = dir.join(name);
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| format!("删除 {name} 失败: {e}"))?;
        }
    }
    tracing::info!("本地嵌入模型已删除（检索回退词法）: {}", dir.display());
    let state = app.state::<AppState>();
    Ok(build_status(&state))
}

// ─── 内部实现 ────────────────────────────────────────────────

fn build_status(state: &AppState) -> EmbeddingLocalStatus {
    let dir = local::model_dir();
    let mut files: Vec<ModelFileInfo> = local::REQUIRED_FILES
        .iter()
        .map(|name| file_info(&dir, name, true))
        .collect();
    files.extend(
        local::OPTIONAL_FILES
            .iter()
            .map(|name| file_info(&dir, name, false)),
    );
    EmbeddingLocalStatus {
        installed: local::is_model_available(),
        dir: dir.to_string_lossy().into_owned(),
        model_name: local::LOCAL_MODEL_NAME.to_string(),
        retrieval_mode: state.memory.retrieval_mode().to_string(),
        remote_configured: state.memory.remote_configured(),
        files,
    }
}

fn file_info(dir: &Path, name: &str, required: bool) -> ModelFileInfo {
    let path = dir.join(name);
    let present = path.is_file();
    let size_bytes = if present {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    ModelFileInfo {
        name: name.to_string(),
        required,
        present,
        size_bytes,
    }
}

/// 把 `path`（文件夹或压缩包）里的模型部件安装到模型目录。
async fn import_from_path(app: &AppHandle, path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("路径不存在: {}", path.display()));
    }

    // 文件夹直接用；压缩包先解到缓存里的临时目录。
    let (source_root, staged_dir): (PathBuf, Option<PathBuf>) = if path.is_dir() {
        (path.to_path_buf(), None)
    } else {
        let format = crate::utils::archive::detect_format(path)
            .map_err(|_| "请选择包含模型文件的文件夹，或 zip / 7z 压缩包".to_string())?;
        let staging = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("获取缓存目录失败: {e}"))?
            .join("embedding-local-import")
            .join(format!("staging-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&staging).map_err(|e| format!("创建临时目录失败: {e}"))?;

        let src = path.to_path_buf();
        let dst = staging.clone();
        let token = tokio_util::sync::CancellationToken::new();
        let extracted = tokio::task::spawn_blocking(move || match format {
            crate::utils::archive::ArchiveFormat::Zip => {
                crate::utils::archive::extract_zip(&src, &dst, &token, &|_| {})
            },
            crate::utils::archive::ArchiveFormat::SevenZ => {
                crate::utils::archive::extract_sevenz(&src, &dst, &token, &|_| {})
            },
        })
        .await
        .map_err(|e| format!("解压任务失败: {e}"))?;
        if let Err(e) = extracted {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("解压失败: {e}"));
        }
        (staging.clone(), Some(staging))
    };

    // 在 source_root 下（含子目录，深度 ≤ 3）按文件名定位部件。
    let found = find_model_files(&source_root);
    let missing: Vec<&str> = local::REQUIRED_FILES
        .iter()
        .copied()
        .filter(|name| !found.contains_key(*name))
        .collect();
    if !missing.is_empty() {
        if let Some(dir) = &staged_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
        return Err(format!("导入失败：缺少必需文件 {}", missing.join("、")));
    }

    let dest_dir = local::model_dir();
    if let Err(e) = std::fs::create_dir_all(&dest_dir) {
        if let Some(dir) = &staged_dir {
            let _ = std::fs::remove_dir_all(dir);
        }
        return Err(format!("创建模型目录失败: {e}"));
    }

    let mut copy_error: Option<String> = None;
    for name in local::REQUIRED_FILES
        .iter()
        .chain(local::OPTIONAL_FILES.iter())
    {
        let Some(src_file) = found.get(*name) else {
            continue;
        };
        let dest_file = dest_dir.join(name);
        if src_file == &dest_file {
            continue;
        }
        if let Err(e) = copy_atomic(src_file, &dest_file) {
            copy_error = Some(format!("写入 {name} 失败: {e}"));
            break;
        }
    }

    if let Some(dir) = &staged_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
    if let Some(e) = copy_error {
        return Err(e);
    }
    if !local::is_model_available() {
        return Err("导入后仍缺少必需文件，请确认所选内容包含完整的模型部件".into());
    }
    tracing::info!("本地嵌入模型已导入: {}", dest_dir.display());
    Ok(())
}

/// 在 `root` 下（含子目录，深度 ≤ 3，BFS 优先靠上层的同名文件）按文件名收集模型部件。
fn find_model_files(root: &Path) -> HashMap<String, PathBuf> {
    let wanted: HashSet<&str> = local::REQUIRED_FILES
        .iter()
        .chain(local::OPTIONAL_FILES.iter())
        .copied()
        .collect();
    let mut found: HashMap<String, PathBuf> = HashMap::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if depth < 3 {
                    queue.push_back((path, depth + 1));
                }
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if wanted.contains(name) {
                    found.entry(name.to_string()).or_insert(path);
                }
            }
        }
    }
    found
}

/// 复制到同目录临时名再改名，避免中途失败留下半个文件。
fn copy_atomic(src: &Path, dest: &Path) -> Result<(), String> {
    let Some(name) = dest.file_name().and_then(|n| n.to_str()) else {
        return Err("目标文件名非法".into());
    };
    let tmp = dest.with_file_name(format!("{name}.part"));
    std::fs::copy(src, &tmp).map_err(|e| e.to_string())?;
    // Windows 上 rename 不能覆盖已存在文件，先删旧。
    let _ = std::fs::remove_file(dest);
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lingchat_embedding_local_test_{}_{}",
            tag,
            std::process::id()
        ))
    }

    /// 部件散落在子目录里也能被找到（BFS，深度 ≤ 3），并忽略无关文件。
    #[test]
    fn find_model_files_walks_subdirs() {
        let root = unique_temp_dir("find");
        let _ = std::fs::remove_dir_all(&root);
        let nested = root.join("bge").join("onnx");
        std::fs::create_dir_all(&nested).expect("create nested dir");
        for name in local::REQUIRED_FILES
            .iter()
            .chain(local::OPTIONAL_FILES.iter())
        {
            std::fs::write(nested.join(name), b"x").expect("write part");
        }
        std::fs::write(root.join("readme.txt"), b"decoy").expect("write decoy");

        let found = find_model_files(&root);
        for name in local::REQUIRED_FILES
            .iter()
            .chain(local::OPTIONAL_FILES.iter())
        {
            assert!(found.contains_key(*name), "未找到 {name}");
        }
        assert!(!found.contains_key("readme.txt"));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// 同名文件优先取靠上层的那份（BFS 顺序）。
    #[test]
    fn find_model_files_prefers_shallower() {
        let root = unique_temp_dir("shallow");
        let _ = std::fs::remove_dir_all(&root);
        let deep = root.join("deep");
        std::fs::create_dir_all(&deep).expect("create deep dir");
        let top_file = root.join(local::MODEL_FILE);
        std::fs::write(&top_file, b"top").expect("write top");
        std::fs::write(deep.join(local::MODEL_FILE), b"deep").expect("write deep");

        let found = find_model_files(&root);
        assert_eq!(found.get(local::MODEL_FILE), Some(&top_file));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// copy_atomic 覆盖已有文件且不残留 `.part`。
    #[test]
    fn copy_atomic_replaces_target() {
        let root = unique_temp_dir("copy");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create dir");
        let src = root.join("src.bin");
        let dest = root.join("dest.bin");
        std::fs::write(&src, b"new").expect("write src");
        std::fs::write(&dest, b"old-content").expect("write dest");

        copy_atomic(&src, &dest).expect("copy");
        assert_eq!(std::fs::read(&dest).expect("read dest"), b"new");
        assert!(!root.join("dest.bin.part").exists());

        let _ = std::fs::remove_dir_all(&root);
    }
}
