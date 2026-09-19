use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de};
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
fn default_rim_color() -> String {
    "#ffd9a0".into()
}
fn default_rim_dx() -> i32 {
    16
}
fn default_rim_dy() -> i32 {
    -12
}
fn default_rim_blur() -> i32 {
    14
}
fn default_light_angle() -> f64 {
    315.0
}
fn default_light_warm_color() -> String {
    "#ffd9a0".into()
}
fn default_shadow_cool_color() -> String {
    "#16233b".into()
}
fn default_light_softness() -> f64 {
    0.55
}
fn default_light_strength() -> f64 {
    0.5
}
fn default_bloom_radius() -> i32 {
    18
}
fn default_bloom_intensity() -> f64 {
    0.35
}
fn default_vignette_strength() -> f64 {
    0.45
}
fn default_vignette_size() -> i32 {
    55
}
fn default_grade_warm_color() -> String {
    "#ffb45e".into()
}
fn default_grade_cool_color() -> String {
    "#2a3f63".into()
}
fn default_grade_strength() -> f64 {
    0.35
}
fn default_breathing_period() -> f64 {
    7.0
}
fn default_breathing_amount() -> f64 {
    0.18
}

// ===== FilterParams: CSS 滤镜组（分别作用于角色 / 背景） =====

/// 一组 CSS 滤镜参数，可分别用于角色和背景图片。
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// 轮廓光（rim light）开关：沿图片 alpha 剪影描一圈受光边
    #[serde(default)]
    pub rim_enabled: bool,
    /// 轮廓光颜色 (hex)
    #[serde(default = "default_rim_color")]
    pub rim_color: String,
    /// 轮廓光水平偏移 (px，正值向右)——决定受光侧
    #[serde(default = "default_rim_dx")]
    pub rim_dx: i32,
    /// 轮廓光垂直偏移 (px，负值向上)
    #[serde(default = "default_rim_dy")]
    pub rim_dy: i32,
    /// 轮廓光模糊半径 (px)
    #[serde(default = "default_rim_blur")]
    pub rim_blur: i32,
}

/// 必须手写：派生 `Default` 会把 brightness/contrast/saturation 置 0，
/// 渲染出来就是一张全黑的图。
impl Default for FilterParams {
    fn default() -> Self {
        Self {
            brightness: default_one(),
            contrast: default_one(),
            saturation: default_one(),
            sepia: 0.0,
            glow_radius: 0,
            glow_color: default_glow_color(),
            rim_enabled: false,
            rim_color: default_rim_color(),
            rim_dx: default_rim_dx(),
            rim_dy: default_rim_dy(),
            rim_blur: default_rim_blur(),
        }
    }
}

// ===== LightingParams: 场景光影参数 =====

/// 场景光影参数：角色 / 背景各自的 CSS 滤镜，加上若干叠加层
/// （径向光、方向双层光、bloom、暗角、冷暖分离、呼吸动画）。
///
/// 向后兼容：旧版 JSON（滤镜字段直接在顶层）由
/// [`deserialize_lighting_compat`] 自动迁移到 character/background。
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    // ---------- 进阶光影层 ----------
    /// 方向双层光：受光侧暖、背光侧冷，用线性渐变表达光的入射方向
    #[serde(default)]
    pub directional_enabled: bool,
    /// 光的入射角 (0–360 度，0 = 正上方来光，顺时针)
    #[serde(default = "default_light_angle")]
    pub light_angle: f64,
    /// 受光侧颜色 (hex)
    #[serde(default = "default_light_warm_color")]
    pub light_warm_color: String,
    /// 背光侧颜色 (hex)
    #[serde(default = "default_shadow_cool_color")]
    pub shadow_cool_color: String,
    /// 明暗过渡柔和度 (0–1)，越大过渡越慢
    #[serde(default = "default_light_softness")]
    pub light_softness: f64,
    /// 方向光强度 (0–1)
    #[serde(default = "default_light_strength")]
    pub light_strength: f64,
    /// bloom 泛光：复制一层背景做模糊 + screen 混合
    #[serde(default)]
    pub bloom_enabled: bool,
    /// bloom 模糊半径 (px)
    #[serde(default = "default_bloom_radius")]
    pub bloom_radius: i32,
    /// bloom 强度 (0–1)
    #[serde(default = "default_bloom_intensity")]
    pub bloom_intensity: f64,
    /// 四角暗角
    #[serde(default)]
    pub vignette_enabled: bool,
    /// 暗角浓度 (0–1)
    #[serde(default = "default_vignette_strength")]
    pub vignette_strength: f64,
    /// 暗角中心清晰区半径 (%)
    #[serde(default = "default_vignette_size")]
    pub vignette_size: i32,
    /// 冷暖分离染色（高光偏暖、阴影偏冷）
    #[serde(default)]
    pub grade_enabled: bool,
    /// 高光染色 (hex)
    #[serde(default = "default_grade_warm_color")]
    pub grade_warm_color: String,
    /// 阴影染色 (hex)
    #[serde(default = "default_grade_cool_color")]
    pub grade_cool_color: String,
    /// 染色强度 (0–1)
    #[serde(default = "default_grade_strength")]
    pub grade_strength: f64,
    /// 呼吸动画：光强与光位随时间缓慢起伏
    #[serde(default)]
    pub breathing_enabled: bool,
    /// 呼吸周期 (秒)
    #[serde(default = "default_breathing_period")]
    pub breathing_period: f64,
    /// 呼吸幅度 (0–1)
    #[serde(default = "default_breathing_amount")]
    pub breathing_amount: f64,
}

impl Default for LightingParams {
    fn default() -> Self {
        Self {
            character: FilterParams::default(),
            background: FilterParams::default(),
            overlay_enabled: false,
            blend_mode: default_blend_mode(),
            light_x: default_fifty(),
            light_y: default_fifty(),
            overlay_color1: default_overlay_color1(),
            overlay_color2: default_overlay_color2(),
            overlay_radius: default_overlay_radius(),
            overlay_opacity: default_overlay_opacity(),
            overlay_target: default_overlay_target(),
            directional_enabled: false,
            light_angle: default_light_angle(),
            light_warm_color: default_light_warm_color(),
            shadow_cool_color: default_shadow_cool_color(),
            light_softness: default_light_softness(),
            light_strength: default_light_strength(),
            bloom_enabled: false,
            bloom_radius: default_bloom_radius(),
            bloom_intensity: default_bloom_intensity(),
            vignette_enabled: false,
            vignette_strength: default_vignette_strength(),
            vignette_size: default_vignette_size(),
            grade_enabled: false,
            grade_warm_color: default_grade_warm_color(),
            grade_cool_color: default_grade_cool_color(),
            grade_strength: default_grade_strength(),
            breathing_enabled: false,
            breathing_period: default_breathing_period(),
            breathing_amount: default_breathing_amount(),
        }
    }
}

/// 旧版 `scenes.json` 里的滤镜字段是平铺的（`brightness` 在顶层）。
/// 读入时把它们复制进 `character` / `background`，再按新结构解析。
///
/// `LightingParams` 本身只认嵌套结构，所以所有外部来源（存档、剧本内联
/// `params`）都要先过一遍这个函数，否则平铺写法会被静默忽略成全默认值。
pub(crate) fn normalize_legacy_lighting(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if obj.contains_key("character") || obj.contains_key("background") {
        return;
    }
    let mut filters = serde_json::Map::new();
    for key in [
        "brightness",
        "contrast",
        "saturation",
        "sepia",
        "glow_radius",
        "glow_color",
    ] {
        if let Some(v) = obj.remove(key) {
            filters.insert(key.to_string(), v);
        }
    }
    if filters.is_empty() {
        return;
    }
    // 旧版没有 target 概念，叠加层本就同时压住人物与背景。
    obj.entry("overlay_target")
        .or_insert_with(|| Value::String(default_overlay_target()));
    obj.insert("character".into(), Value::Object(filters.clone()));
    obj.insert("background".into(), Value::Object(filters));
}

fn deserialize_lighting_compat<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<LightingParams>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<Value>::deserialize(deserializer)?;
    let Some(mut value) = raw else {
        return Ok(None);
    };

    normalize_legacy_lighting(&mut value);

    serde_json::from_value(value)
        .map(Some)
        .map_err(de::Error::custom)
}

/// 运行时光影覆盖：由剧本 `lighting` 事件、`lighting_apply` 工具或设置面板写入。
///
/// 优先级高于场景自带的 `lighting`，但**不进存档快照**——重开软件回到
/// 「跟随场景」，避免上一次剧情的灯光漏到下一次对话里。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LightingOverride {
    /// 预设 id；与 `params` 同时给出时以 `params` 为准
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// 直接给出的完整参数（供自定义/微调场景使用）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<LightingParams>,
}

impl LightingOverride {
    pub fn from_preset(id: &str) -> Self {
        Self {
            preset: Some(id.to_string()),
            params: None,
        }
    }

    /// 解析成实际渲染用的参数：显式 params 优先，其次查预设表。
    pub fn resolve(&self) -> Option<LightingParams> {
        if let Some(params) = &self.params {
            return Some(params.clone());
        }
        self.preset
            .as_deref()
            .and_then(|id| super::lighting_store::resolve(id))
    }
}

/// 用户创建的场景：名称 + 描述 + 背景图片 + 光影参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub description: String,
    pub background: String,
    #[serde(default, deserialize_with = "deserialize_lighting_compat")]
    pub lighting: Option<LightingParams>,
    pub created_at: String,
    pub updated_at: String,
    /// 该场景来自哪个插件（None = 游戏自有 / 用户上传场景）。
    /// 插件背景图自动注册为场景时写入，用于启停 / 隐藏时同步增删。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Scene {
        serde_json::from_str(input).expect("场景 JSON 解析失败")
    }

    /// 老存档：滤镜字段平铺在 lighting 顶层，必须读成 character/background 两份。
    #[test]
    fn legacy_flat_lighting_still_loads() {
        let scene = parse(
            r#"{
                "id": "s1",
                "name": "旧场景", "description": "",
                "background": "bg.png",
                "lighting": {"brightness": 1.4, "glow_radius": 18, "overlay_enabled": true},
                "created_at": "0",
                "updated_at": "0"
            }"#,
        );
        let lighting = scene.lighting.expect("平铺 lighting 应被迁移而非丢弃");
        assert_eq!(lighting.character.brightness, 1.4);
        assert_eq!(lighting.background.glow_radius, 18);
        assert!(lighting.overlay_enabled);
        // 新增字段缺省即关闭，老场景不会因为升级而莫名变亮或变暗。
        assert_eq!(lighting.character.rim_enabled, false);
        assert_eq!(lighting.bloom_enabled, false);
        assert_eq!(lighting.vignette_enabled, false);
    }

    #[test]
    fn nested_lighting_is_left_untouched() {
        let scene = parse(
            r#"{
                "id": "s2",
                "name": "新场景", "description": "",
                "background": "bg.png",
                "lighting": {"character": {"brightness": 0.8}, "background": {"brightness": 1.2}},
                "created_at": "0",
                "updated_at": "0"
            }"#,
        );
        let lighting = scene.lighting.expect("嵌套 lighting");
        assert_eq!(lighting.character.brightness, 0.8);
        assert_eq!(lighting.background.brightness, 1.2);
    }

    #[test]
    fn missing_lighting_is_none() {
        let scene = parse(
            r#"{
                "id": "s3",
                "name": "无灯光", "description": "",
                "background": "bg.png",
                "created_at": "0",
                "updated_at": "0"
            }"#,
        );
        assert!(scene.lighting.is_none());
    }
}
