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
fn default_light_type() -> String {
    "point".into()
}
fn default_light_color() -> String {
    "#ffffff".into()
}
fn default_true() -> bool {
    true
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

/// 单盏光源（2.5D 多光源扩展）。
///
/// 统一扁平结构：所有几何字段都用 f64 存放，不适用的字段由前端填默认值。
/// 这样 JSON 简单、前端无分支，避免 serde 标签枚举带来的歧义。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LightSource {
    /// 编辑器内的稳定标识（前端生成）
    #[serde(default)]
    pub id: String,
    /// 显示名称（编辑器里给灯起的名字，缺省由前端按类型生成）
    #[serde(default)]
    pub name: String,
    /// 是否被标为主光（纯 UI 标记，不影响渲染计算）
    #[serde(default)]
    pub is_key: bool,
    /// "point" | "directional" | "spot" | "ambient"
    #[serde(rename = "type", default = "default_light_type")]
    pub kind: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 光源颜色 (hex)
    #[serde(default = "default_light_color")]
    pub color: String,
    /// 强度
    #[serde(default)]
    pub intensity: f64,
    /// 舞台归一化横坐标 (0–1)
    #[serde(default)]
    pub x: f64,
    /// 舞台归一化纵坐标 (0–1)
    #[serde(default)]
    pub y: f64,
    /// 高度，以短边为基准
    #[serde(default)]
    pub z: f64,
    /// 照射半径
    #[serde(default)]
    pub radius: f64,
    /// 边缘柔和度 (0–1)
    #[serde(default)]
    pub softness: f64,
    /// 方位角（度），平行光 / 锥光使用
    #[serde(default)]
    pub azimuth: f64,
    /// 仰角（度），平行光 / 锥光使用
    #[serde(default)]
    pub elevation: f64,
    /// 锥角（度），仅锥光使用
    #[serde(default)]
    pub cone: f64,
    /// 锥边半影 (0–1)，仅锥光使用
    #[serde(default)]
    pub penumbra: f64,
}

/// 全局光影强度参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct GlobalParams {
    pub master: f64,
    /// 环境光参与混合的比例
    pub ambient_blend: f64,
    /// 反照率保护强度（0–1）
    pub albedo_protect: f64,
    /// 软膝色调映射强度
    pub soft_knee: f64,
    /// 暗部抬升
    pub lift: f64,
    /// 去色程度（黑白电影等）
    pub mono: f64,
    /// 饱和度（0–2，1 = 原色，0 = 灰度，>1 增强）
    pub saturation: f64,
    /// 对比度（0–2，1 = 原样）
    pub contrast: f64,
    /// 整体色调偏移角度（度，−180–180），配合 tint_amount 使用
    pub tint_hue: f64,
    /// 色调偏移强度（0–1）
    pub tint_amount: f64,
    /// 立绘单独曝光增益（0–2），与 master 相乘
    pub char_exposure: f64,
    /// 背景单独曝光增益（0–2），与 master 相乘
    pub bg_exposure: f64,
}

impl Default for GlobalParams {
    fn default() -> Self {
        Self {
            master: 1.0,
            ambient_blend: 0.10,
            albedo_protect: 0.24,
            soft_knee: 0.70,
            lift: 0.10,
            mono: 0.0,
            saturation: 1.0,
            contrast: 1.0,
            tint_hue: 0.0,
            tint_amount: 0.0,
            char_exposure: 1.0,
            bg_exposure: 1.0,
        }
    }
}

/// 合成层参数（光包裹 / 边缘光 / 泛光）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct CompositingParams {
    /// 光透过轮廓包裹到暗侧的程度
    pub light_wrap: f64,
    /// 边缘光宽度（像素）
    pub rim_width: f64,
    /// 泛光强度
    pub bloom_int: f64,
    /// 泛光阈值
    pub bloom_thresh: f64,
    /// 泛光染色（hex），白色等于不染色
    pub glow_color: String,
}

impl Default for CompositingParams {
    fn default() -> Self {
        Self {
            light_wrap: 0.22,
            rim_width: 4.5,
            bloom_int: 0.14,
            bloom_thresh: 0.72,
            glow_color: "#ffffff".to_string(),
        }
    }
}

/// 材质参数（伪法线 / 卡通着色 / 高光 / 自阴影）。
///
/// use_normal / use_depth 关闭时，整条伪法线 / 伪深度管线跳过，
/// 回落到只有多光源 RGB 增益的旧表现。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct MaterialParams {
    /// 是否启用伪法线立体受光
    pub use_normal: bool,
    /// 法线强度
    pub normal_strength: f64,
    /// 卡通着色分界阈值
    pub cel_thresh: f64,
    /// 卡通着色过渡宽度
    pub cel_softness: f64,
    /// 高光强度
    pub spec_int: f64,
    /// 高光锐度
    pub spec_gloss: f64,
    /// 自阴影强度
    pub self_shadow_int: f64,
    /// 是否启用伪深度
    pub use_depth: bool,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            use_normal: true,
            normal_strength: 1.0,
            cel_thresh: 0.50,
            cel_softness: 0.06,
            spec_int: 0.45,
            spec_gloss: 24.0,
            self_shadow_int: 0.35,
            use_depth: true,
        }
    }
}

/// 相机 / 景别参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct CameraParams {
    /// "fs"(远景) | "ws"(全景) | "ms"(中景) | "cu"(近景) | "ecu"(特写)
    pub shot: String,
    pub zoom: f64,
    /// 垂直构图焦点
    pub focus_y: f64,
    /// 背景景深
    pub dof: f64,
}

impl Default for CameraParams {
    fn default() -> Self {
        Self {
            shot: "ms".into(),
            zoom: 1.25,
            focus_y: 0.16,
            dof: 2.2,
        }
    }
}

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
    /// 多光源数组（2.5D 多光源扩展）。
    /// None 表示未启用——沿用上面的 CSS 滤镜 + 叠加层老逻辑。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lights: Option<Vec<LightSource>>,
    /// 全局强度参数；None 表示未启用 2.5D 管线。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<GlobalParams>,
    /// 合成层参数（光包裹 / 边缘光 / 泛光）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compositing: Option<CompositingParams>,
    /// 材质参数（伪法线 / 卡通着色 / 高光 / 自阴影）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<MaterialParams>,
    /// 相机 / 景别参数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraParams>,
    /// 角色专属光影，键是 roleId 的字符串形式。
    ///
    /// 后端只负责原样存取：每套的形状由前端 `LightingSet` 定义，这里用 Value 透传，
    /// 免得前端加字段时旧版本后端把它吃掉（存一次就永久丢数据）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_sets: Option<std::collections::HashMap<String, Value>>,
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
                #[serde(default)]
                lights: Option<Vec<LightSource>>,
                #[serde(default)]
                global: Option<GlobalParams>,
                #[serde(default)]
                compositing: Option<CompositingParams>,
                #[serde(default)]
                material: Option<MaterialParams>,
                #[serde(default)]
                camera: Option<CameraParams>,
                #[serde(default)]
                role_sets: Option<std::collections::HashMap<String, Value>>,
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
                lights: nf.lights,
                global: nf.global,
                compositing: nf.compositing,
                material: nf.material,
                camera: nf.camera,
                role_sets: nf.role_sets,
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
            lights: val
                .get("lights")
                .and_then(|v| serde_json::from_value::<Vec<LightSource>>(v.clone()).ok()),
            global: val
                .get("global")
                .and_then(|v| serde_json::from_value::<GlobalParams>(v.clone()).ok()),
            compositing: val
                .get("compositing")
                .and_then(|v| serde_json::from_value::<CompositingParams>(v.clone()).ok()),
            material: val
                .get("material")
                .and_then(|v| serde_json::from_value::<MaterialParams>(v.clone()).ok()),
            camera: val
                .get("camera")
                .and_then(|v| serde_json::from_value::<CameraParams>(v.clone()).ok()),
            role_sets: val.get("role_sets").and_then(|v| {
                serde_json::from_value::<std::collections::HashMap<String, Value>>(v.clone()).ok()
            }),
        })
    }
}

/// 场景室内外标注；`Unknown` 表示无法判定（默认按室外处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationType {
    Indoor,
    Outdoor,
    Unknown,
}

impl LocationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LocationType::Indoor => "indoor",
            LocationType::Outdoor => "outdoor",
            LocationType::Unknown => "unknown",
        }
    }

    /// 解析标注字符串（前端回传 / LLM 输出归一化后）。
    pub fn parse(value: &str) -> Option<LocationType> {
        match value.trim().to_lowercase().as_str() {
            "indoor" | "室内" => Some(LocationType::Indoor),
            "outdoor" | "室外" => Some(LocationType::Outdoor),
            "unknown" | "不确定" => Some(LocationType::Unknown),
            _ => None,
        }
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
    /// 场景所属子分类（背景子文件夹名；None 表示背景位于根目录或未设置分类）。
    /// 显式存储，避免依赖「背景文件名 → 递归查找路径 → 与背景列表 url 匹配」的推断链失效。
    #[serde(default)]
    pub category: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// 该场景来自哪个插件（None = 游戏自有 / 用户上传场景）。
    /// 插件背景图自动注册为场景时写入，用于启停 / 隐藏时同步增删。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    /// 室内外标注（None = 未标注，旧场景缺省）；LLM 标注一次，可在场景卡上改写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_type: Option<LocationType>,
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

    const NEW_FILTER: &str = r##""character":{"brightness":1.0,"contrast":1.0,"saturation":1.0,"sepia":0.0,"glow_radius":0,"glow_color":"#ffaa33"},"background":{"brightness":1.0,"contrast":1.0,"saturation":1.0,"sepia":0.0,"glow_radius":0,"glow_color":"#ffaa33"}"##;

    /// 旧存档（滤镜字段在顶层）必须仍然能读，且不会被凭空塞进 lights。
    #[test]
    fn lighting_legacy_top_level_filter_still_parses() {
        let json = r##"{
            "brightness": 1.2, "contrast": 1.1, "saturation": 0.9,
            "sepia": 0.1, "glow_radius": 8, "glow_color": "#ffaa33",
            "overlay_enabled": true, "light_x": 40, "light_y": 60
        }"##;
        let p: LightingParams = serde_json::from_str(json).unwrap();
        assert_eq!(p.character.brightness, 1.2);
        assert_eq!(p.background.brightness, 1.2, "旧格式应迁移到两边");
        assert_eq!(p.light_x, 40);
        assert!(p.lights.is_none(), "旧存档不应产生 lights");
    }

    /// 新格式但没写 lights —— 保持 None，行为与改动前一致。
    #[test]
    fn lighting_new_format_without_lights_is_none() {
        let json = format!(r##"{{{}}}"##, NEW_FILTER);
        let p: LightingParams = serde_json::from_str(&json).unwrap();
        assert!(p.lights.is_none());
    }

    /// 带 lights 的存档要完整解析，各灯型字段正确。
    #[test]
    fn lighting_lights_are_parsed() {
        let json = format!(
            r##"{{{},"lights":[
                {{"id":"l1","type":"ambient","color":"#1b2a4d","intensity":0.22}},
                {{"id":"l2","type":"directional","color":"#ffe8d0","intensity":0.3,"azimuth":92,"elevation":64}},
                {{"id":"l3","type":"spot","color":"#ffffff","intensity":1.0,"x":0.3,"y":0.2,"z":0.6,"radius":1.2,"softness":0.8,"azimuth":52,"elevation":35,"cone":56,"penumbra":0.5}}
            ]}}"##,
            NEW_FILTER
        );
        let p: LightingParams = serde_json::from_str(&json).unwrap();
        let lights = p.lights.expect("应解析出 lights");
        assert_eq!(lights.len(), 3);
        assert_eq!(lights[0].kind, "ambient");
        assert_eq!(lights[1].kind, "directional");
        assert_eq!(lights[1].azimuth, 92.0);
        assert_eq!(lights[2].kind, "spot");
        assert_eq!(lights[2].cone, 56.0);
        assert_eq!(lights[2].penumbra, 0.5);
        assert!(lights[0].enabled, "enabled 缺省应为 true");
    }

    /// lights 为 None 时不写出该字段 —— 保证旧场景回写后 JSON 形状不变。
    #[test]
    fn lighting_none_lights_is_skipped_on_serialize() {
        let p = LightingParams::default();
        assert!(p.lights.is_none());
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("lights").is_none(), "None 不应写出 lights 字段");
    }

    /// 序列化 → 反序列化，光源字段不丢失。
    #[test]
    fn lighting_lights_roundtrip() {
        let json = format!(
            r##"{{{},"lights":[{{"id":"l1","type":"point","color":"#ffd9b0","intensity":0.5,"x":0.48,"y":0.38,"z":0.7,"radius":0.9,"softness":0.75}}]}}"##,
            NEW_FILTER
        );
        let p1: LightingParams = serde_json::from_str(&json).unwrap();
        let s = serde_json::to_string(&p1).unwrap();
        let p2: LightingParams = serde_json::from_str(&s).unwrap();

        let a = p1.lights.unwrap();
        let b = p2.lights.expect("往返后 lights 应保留");
        assert_eq!(a.len(), b.len());
        assert_eq!(a[0].id, b[0].id);
        assert_eq!(a[0].color, b[0].color);
        assert_eq!(a[0].intensity, b[0].intensity);
        assert_eq!(a[0].z, b[0].z);
    }

    /// 材质 / 合成 / 相机 / 全局四组参数完整解析。
    #[test]
    fn lighting_subparams_are_parsed() {
        let json = format!(
            r##"{{{},"global":{{"master":0.9,"ambient_blend":0.2,"albedo_protect":0.5,"soft_knee":0.6,"lift":0.05,"mono":0.1}},"compositing":{{"light_wrap":0.3,"rim_width":6.0,"bloom_int":0.2,"bloom_thresh":0.8}},"material":{{"use_normal":true,"normal_strength":1.4,"cel_thresh":0.45,"cel_softness":0.1,"spec_int":0.5,"spec_gloss":32.0,"self_shadow_int":0.4,"use_depth":false}},"camera":{{"shot":"cu","zoom":1.55,"focus_y":0.24,"dof":5.5}}}}"##,
            NEW_FILTER
        );
        let p: LightingParams = serde_json::from_str(&json).unwrap();

        let g = p.global.expect("global");
        assert!((g.master - 0.9).abs() < 1e-9);
        assert!((g.ambient_blend - 0.2).abs() < 1e-9);

        let c = p.compositing.expect("compositing");
        assert!((c.rim_width - 6.0).abs() < 1e-9);

        let m = p.material.expect("material");
        assert!(m.use_normal);
        assert!(!m.use_depth);
        assert!((m.spec_gloss - 32.0).abs() < 1e-9);

        let cam = p.camera.expect("camera");
        assert_eq!(cam.shot, "cu");
        assert!((cam.dof - 5.5).abs() < 1e-9);
    }

    /// 子对象只给部分字段时，缺失项必须取 Default，而不是 0/false。
    #[test]
    fn lighting_partial_subparams_fall_back_to_defaults() {
        let json = format!(
            r##"{{{}, "material": {{"normal_strength":2.0}}}}"##,
            NEW_FILTER
        );
        let p: LightingParams = serde_json::from_str(&json).unwrap();
        let m = p.material.expect("material 应存在");
        assert!((m.normal_strength - 2.0).abs() < 1e-9);
        assert!(m.use_normal, "缺失字段应取 default 的 true 而不是 false");
        assert!((m.spec_gloss - 24.0).abs() < 1e-9);
        assert!((m.cel_thresh - 0.50).abs() < 1e-9);
    }

    /// 五组新增字段全为 None 时不写进 JSON —— 旧场景回写形状不变。
    #[test]
    fn lighting_subparams_none_are_skipped_on_serialize() {
        let p = LightingParams::default();
        let v = serde_json::to_value(&p).unwrap();
        for key in [
            "lights",
            "global",
            "compositing",
            "material",
            "camera",
            "role_sets",
        ] {
            assert!(v.get(key).is_none(), "{} 为 None 时不应写出", key);
        }
    }

    /// 角色专属光影原样透传：往返后每套的灯与参数都不掉字段。
    #[test]
    fn lighting_role_sets_roundtrip() {
        let json = format!(
            r##"{{{},"role_sets":{{"3":{{"lights":[{{"id":"r3","type":"point","color":"#8fd0ff","intensity":1.6,"x":0.3,"y":0.4,"z":0.6,"radius":1.0,"softness":0.5}}],"global":{{"master":1.2,"char_exposure":0.8}},"camera":{{"shot":"cu","zoom":1.5,"focus_y":0.2,"dof":1.0}}}}}}}}"##,
            NEW_FILTER
        );
        let p1: LightingParams = serde_json::from_str(&json).unwrap();
        let sets = p1.role_sets.as_ref().expect("应解析出 role_sets");
        assert_eq!(sets.len(), 1);
        assert!(sets.contains_key("3"));

        let s = serde_json::to_string(&p1).unwrap();
        let p2: LightingParams = serde_json::from_str(&s).unwrap();
        let again = p2.role_sets.expect("往返后 role_sets 应保留");
        let set = again.get("3").unwrap();
        assert_eq!(set["lights"][0]["color"], "#8fd0ff");
        assert_eq!(set["global"]["char_exposure"], 0.8);
        assert_eq!(set["camera"]["shot"], "cu");
    }
}
