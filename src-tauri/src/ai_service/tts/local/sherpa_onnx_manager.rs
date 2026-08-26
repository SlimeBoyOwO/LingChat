//! Sherpa-ONNX 模型管理器
//!
//! 负责管理 Sherpa-ONNX 模型的下载、验证、缓存和加载状态。

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::RwLock;

/// Sherpa-ONNX 模型信息
#[derive(Debug, Clone)]
pub struct SherpaOnnxModelInfo {
    pub model_name: String,
    pub model_path: PathBuf,
    pub model_type: String,
    pub language: String,
    pub voice: String,
    pub size_bytes: u64,
    pub is_valid: bool,
}

/// Sherpa-ONNX 模型管理器
#[derive(Debug)]
pub struct SherpaOnnxManager {
    models: Arc<RwLock<HashMap<String, SherpaOnnxModelInfo>>>,
    base_path: PathBuf,
}

impl SherpaOnnxManager {
    /// 创建新的模型管理器
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            base_path,
        }
    }

    /// 获取模型目录
    pub fn model_dir(&self) -> PathBuf {
        self.base_path.join("sherpa_onnx_models")
    }

    /// 初始化模型管理器
    pub async fn initialize(&self) -> Result<()> {
        // 创建模型目录
        tokio::fs::create_dir_all(self.model_dir()).await?;
        
        // 扫描现有模型
        self.scan_existing_models().await?;
        
        Ok(())
    }

    /// 扫描现有模型文件
    async fn scan_existing_models(&self) -> Result<()> {
        let model_dir = self.model_dir();
        if !model_dir.exists() {
            return Ok(());
        }

        let mut models = self.models.write().await;
        let mut entries = tokio::fs::read_dir(&model_dir).await?;
        
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(model_info) = self.parse_model_directory(&path).await {
                    models.insert(model_info.model_name.clone(), model_info);
                }
            }
        }
        
        Ok(())
    }

    /// 解析模型目录
    async fn parse_model_directory(&self, model_dir: &Path) -> Result<SherpaOnnxModelInfo> {
        let model_name = model_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Invalid model directory name"))?
            .to_string();

        // 查找模型文件
        let model_files = ["model.onnx", "tts-model.onnx", "sherpa-onnx-tts.onnx"];
        let mut model_path = None;
        
        for file in &model_files {
            let candidate = model_dir.join(file);
            if candidate.exists() {
                model_path = Some(candidate);
                break;
            }
        }

        let model_path = model_path.ok_or_else(|| anyhow!("No model file found in directory"))?;
        
        // 获取模型文件大小
        let metadata = tokio::fs::metadata(&model_path).await?;
        let size_bytes = metadata.len();

        // 解析配置文件（如果存在）
        let config_path = model_dir.join("config.json");
        let (model_type, language, voice) = if config_path.exists() {
            self.load_model_config(&config_path).await?
        } else {
            // 默认值
            ("vits".to_string(), "zh".to_string(), "female".to_string())
        };

        Ok(SherpaOnnxModelInfo {
            model_name,
            model_path,
            model_type,
            language,
            voice,
            size_bytes,
            is_valid: true, // TODO: 实现模型验证
        })
    }

    /// 加载模型配置
    async fn load_model_config(&self, config_path: &Path) -> Result<(String, String, String)> {
        let content = tokio::fs::read_to_string(config_path).await?;
        let config: serde_json::Value = serde_json::from_str(&content)?;
        
        let model_type = config.get("model_type")
            .and_then(|v| v.as_str())
            .unwrap_or("vits");
        
        let language = config.get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("zh");
        
        let voice = config.get("voice")
            .and_then(|v| v.as_str())
            .unwrap_or("female");

        Ok((model_type.to_string(), language.to_string(), voice.to_string()))
    }

    /// 获取所有模型
    pub async fn get_models(&self) -> Vec<SherpaOnnxModelInfo> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    /// 获取指定模型
    pub async fn get_model(&self, model_name: &str) -> Option<SherpaOnnxModelInfo> {
        let models = self.models.read().await;
        models.get(model_name).cloned()
    }

    /// 检查模型是否存在
    pub async fn model_exists(&self, model_name: &str) -> bool {
        let models = self.models.read().await;
        models.contains_key(model_name)
    }

    /// 添加模型
    pub async fn add_model(&self, model_info: SherpaOnnxModelInfo) -> Result<()> {
        let mut models = self.models.write().await;
        models.insert(model_info.model_name.clone(), model_info);
        Ok(())
    }

    /// 删除模型
    pub async fn remove_model(&self, model_name: &str) -> Result<()> {
        let mut models = self.models.write().await;
        
        if let Some(model_info) = models.remove(model_name) {
            // 删除模型文件
            if model_info.model_path.exists() {
                tokio::fs::remove_file(&model_info.model_path).await?;
            }
            
            // 删除模型目录（如果为空）
            if let Some(parent) = model_info.model_path.parent() {
                if parent.exists() {
                    let mut entries = tokio::fs::read_dir(parent).await?;
                    if entries.next_entry().await?.is_none() {
                        tokio::fs::remove_dir(parent).await?;
                    }
                }
            }
        }
        
        Ok(())
    }

    /// 获取模型路径
    pub async fn get_model_path(&self, model_name: &str) -> Option<PathBuf> {
        let models = self.models.read().await;
        models.get(model_name).map(|info| info.model_path.clone())
    }

    /// 获取模型类型
    pub async fn get_model_type(&self, model_name: &str) -> Option<String> {
        let models = self.models.read().await;
        models.get(model_name).map(|info| info.model_type.clone())
    }

    /// 获取模型语言
    pub async fn get_model_language(&self, model_name: &str) -> Option<String> {
        let models = self.models.read().await;
        models.get(model_name).map(|info| info.language.clone())
    }

    /// 获取模型音色
    pub async fn get_model_voice(&self, model_name: &str) -> Option<String> {
        let models = self.models.read().await;
        models.get(model_name).map(|info| info.voice.clone())
    }

    /// 获取模型大小
    pub async fn get_model_size(&self, model_name: &str) -> Option<u64> {
        let models = self.models.read().await;
        models.get(model_name).map(|info| info.size_bytes)
    }

    /// 验证模型完整性
    pub async fn validate_model(&self, model_name: &str) -> Result<bool> {
        let models = self.models.read().await;
        if let Some(model_info) = models.get(model_name) {
            if !model_info.model_path.exists() {
                return Ok(false);
            }
            
            // TODO: 实现更详细的模型验证
            // 检查文件大小、模型格式等
            
            Ok(true)
        } else {
            Err(anyhow!("Model not found: {}", model_name))
        }
    }

    /// 获取模型存储统计
    pub async fn get_storage_stats(&self) -> Result<(u64, usize)> {
        let models = self.models.read().await;
        let total_size = models.values().map(|info| info.size_bytes).sum();
        let model_count = models.len();
        
        Ok((total_size, model_count))
    }

    /// 清理无效模型
    pub async fn cleanup_invalid_models(&self) -> Result<usize> {
        let mut models = self.models.write().await;
        let mut removed_count = 0;
        
        let invalid_models: Vec<String> = models
            .iter()
            .filter(|(_, info)| !info.is_valid)
            .map(|(name, _)| name.clone())
            .collect();
        
        for model_name in invalid_models {
            if models.remove(&model_name).is_some() {
                removed_count += 1;
            }
        }
        
        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_model_manager() {
        let temp_dir = tempdir().unwrap();
        let manager = SherpaOnnxManager::new(temp_dir.path().to_path_buf());
        
        // 初始化
        assert!(manager.initialize().await.is_ok());
        
        // 测试空模型列表
        let models = manager.get_models().await;
        assert!(models.is_empty());
        
        // 测试存储统计
        let (size, count) = manager.get_storage_stats().await.unwrap();
        assert_eq!(size, 0);
        assert_eq!(count, 0);
    }
}