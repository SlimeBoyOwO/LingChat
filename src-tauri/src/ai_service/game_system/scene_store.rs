use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{de, Deserialize, Serialize};
use serde_json::Value;

// ===== Default helpers =====

fn default_one() -> f64 {
    1.0
}
fn default_fifty() -> i32 {
    50
}
fn default_glow_color() -> String {
    "#ffaa33".into()
}
fn default_blend_mode() -> String {
    "normal".into()
}
fn default_overlay_color1() -> String {
    "#ffb44b".into()
}
fn default_overlay_color2() -> String {
    "#18202e".into()
}
fn default_overlay_radius() -> i32 {
    80
}
fn default_overlay_opacity() -> f64 {
    0.5
}
fn default_overlay_target() -> String {
    "both".into()
}

// ===== FilterParams: CSS 滤镜组（分别作用于角色 / 背景） =====

/// 一组 CSS 滤镜参数，可分别用于角色和背景图片。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct FilterParams {
    /// CSS brightness 滤镜 (0.3–2.2)
    #[serde(default = "default_one")]
    pub brightness: f64,
    /// CSS contrast 滤镜 (0.5–2.0)
    #[serde(default = "default_one")]
    pub contrast: f64,
    /// CSS saturate 滤镜 (0.0–2.5)
    #[serde(default = "default_one")]
    pub saturation: f64,
    /// CSS sepia 滤镜 (0.0–1.0)
    #[serde(default)]
    pub sepia: f64,
    /// drop-shadow 模糊半径 (0–50 px)
    #[serde(default)]
    pub glow_radius: i32,
    /// drop-shadow 颜色 (hex)
    #[serde(default = "default_glow_color")]
    pub glow_color: String,
}

// ===== LightingParams: 场景光影参数 =====

/// 场景光影参数，可分别控制角色和背景的滤镜，以及全局光照叠加层。
/// 参考 example/shadow.html 的 CSS filter + blend-mode + light overlay。
///
/// 向后兼容：旧版 JSON（滤镜字段直接在顶层）会自动迁移到 character/background。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct LightingParams {
    /// 角色图片 CSS 滤镜
    #[serde(default)]
    pub character: FilterParams,
    /// 背景图片 CSS 滤镜
    #[serde(default)]
    pub background: FilterParams,
    /// 是否启用光照叠加层
    #[serde(default)]
    pub overlay_enabled: bool,
    /// CSS mix-blend-mode（叠加层混合模式）
    #[serde(default = "default_blend_mode")]
    pub blend_mode: String,
    /// 光源水平位置 (0–100 %)
    #[serde(default = "default_fifty")]
    pub light_x: i32,
    /// 光源垂直位置 (0–100 %)
    #[serde(default = "default_fifty")]
    pub light_y: i32,
    /// radial-gradient 中心颜色 (hex)
    #[serde(default = "default_overlay_color1")]
    pub overlay_color1: String,
    /// radial-gradient 边缘颜色 (hex)
    #[serde(default = "default_overlay_color2")]
    pub overlay_color2: String,
    /// radial-gradient 边缘半径百分比 (0–100)，控制光照范围
    #[serde(default = "default_overlay_radius")]
    pub overlay_radius: i32,
    /// 叠加层不透明度 (0.0–1.0)，控制光照强度
    #[serde(default = "default_overlay_opacity")]
    pub overlay_opacity: f64,
    /// 叠加层作用目标: "both" | "character" | "background"
    #[serde(default = "default_overlay_target")]
    pub overlay_target: String,
}

impl<'de> Deserialize<'de> for LightingParams {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = Value::deserialize(deserializer)?;

        // 新格式：有 character 或 background 嵌套对象
        if val.get("character").is_some() || val.get("background").is_some() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            struct NewFormat {
                #[serde(default)]
                character: FilterParams,
                #[serde(default)]
                background: FilterParams,
                #[serde(default)]
                overlay_enabled: bool,
                #[serde(default = "default_blend_mode")]
                blend_mode: String,
                #[serde(default = "default_fifty")]
                light_x: i32,
                #[serde(default = "default_fifty")]
                light_y: i32,
                #[serde(default = "default_overlay_color1")]
                overlay_color1: String,
                #[serde(default = "default_overlay_color2")]
                overlay_color2: String,
                #[serde(default = "default_overlay_radius")]
                overlay_radius: i32,
                #[serde(default = "default_overlay_opacity")]
                overlay_opacity: f64,
                #[serde(default = "default_overlay_target")]
                overlay_target: String,
            }
            let nf: NewFormat = serde_json::from_value(val).map_err(de::Error::custom)?;
            return Ok(LightingParams {
                character: nf.character,
                background: nf.background,
                overlay_enabled: nf.overlay_enabled,
                blend_mode: nf.blend_mode,
                light_x: nf.light_x,
                light_y: nf.light_y,
                overlay_color1: nf.overlay_color1,
                overlay_color2: nf.overlay_color2,
                overlay_radius: nf.overlay_radius,
                overlay_opacity: nf.overlay_opacity,
                overlay_target: nf.overlay_target,
            });
        }

        // 旧格式迁移：滤镜字段在顶层，同时复制给 character 和 background
        let migrated = FilterParams {
            brightness: val
                .get("brightness")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0),
            contrast: val.get("contrast").and_then(|v| v.as_f64()).unwrap_or(1.0),
            saturation: val
                .get("saturation")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0),
            sepia: val.get("sepia").and_then(|v| v.as_f64()).unwrap_or(0.0),
            glow_radius: val.get("glow_radius").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            glow_color: val
                .get("glow_color")
                .and_then(|v| v.as_str())
                .unwrap_or("#ffaa33")
                .to_string(),
        };

        Ok(LightingParams {
            character: migrated.clone(),
            background: migrated,
            overlay_enabled: val
                .get("overlay_enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            blend_mode: val
                .get("blend_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("normal")
                .to_string(),
            light_x: val.get("light_x").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
            light_y: val.get("light_y").and_then(|v| v.as_i64()).unwrap_or(50) as i32,
            overlay_color1: val
                .get("overlay_color1")
                .and_then(|v| v.as_str())
                .unwrap_or("#ffb44b")
                .to_string(),
            overlay_color2: val
                .get("overlay_color2")
                .and_then(|v| v.as_str())
                .unwrap_or("#18202e")
                .to_string(),
            overlay_radius: val
                .get("overlay_radius")
                .and_then(|v| v.as_i64())
                .unwrap_or(80) as i32,
            overlay_opacity: val
                .get("overlay_opacity")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5),
            overlay_target: val
                .get("overlay_target")
                .and_then(|v| v.as_str())
                .unwrap_or("both")
                .to_string(),
        })
    }
}

/// 用户创建的场景：名称 + 描述 + 背景图片 + 光影参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub description: String,
    pub background: String,
    #[serde(default)]
    pub lighting: Option<LightingParams>,
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
