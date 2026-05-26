use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 用户创建的场景：名称 + 描述 + 背景图片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub description: String,
    pub background: String,
    pub created_at: String,
    pub updated_at: String,
}

/// JSON 文件存储，路径为 `<data_dir>/game_data/scenes.json`
pub struct SceneStore {
    path: PathBuf,
}

impl SceneStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("game_data").join("scenes.json"),
        }
    }

    fn ensure_dir(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    pub fn load_all(&self) -> Result<Vec<Scene>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.path)
            .with_context(|| format!("读取场景文件失败: {:?}", self.path))?;
        let scenes: Vec<Scene> = serde_json::from_str(&content)
            .with_context(|| format!("解析场景 JSON 失败: {:?}", self.path))?;
        Ok(scenes)
    }

    pub fn save_all(&self, scenes: &[Scene]) -> Result<()> {
        self.ensure_dir()?;
        let content = serde_json::to_string_pretty(scenes)?;
        std::fs::write(&self.path, content)
            .with_context(|| format!("写入场景文件失败: {:?}", self.path))?;
        Ok(())
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Scene>> {
        Ok(self.load_all()?.into_iter().find(|s| s.id == id))
    }
}
