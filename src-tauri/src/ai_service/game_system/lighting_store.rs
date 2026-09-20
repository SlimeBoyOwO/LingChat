//! 光影预设库：整个应用唯一的一份预设数据源。
//!
//! 三处消费方共用它，避免各说各话：
//! - 前端设置面板 / 渲染层：`lighting_list_presets` 命令拿到完整 `LightingParams`
//! - 剧本编辑器：`schema.rs` 用 [`preset_ids`] 生成下拉选项，`validate.rs` 校验预设名
//! - 插件与 LLM：`lighting_list_presets` 工具拿到 id / 名称 / 心情关键词
//!
//! 新增预设 = 写一个 `fn` + 在 [`ENTRIES`] 加一行，其余全自动。
//!
//! 除了编译期的 [`ENTRIES`]，还有一张运行期可写的「用户自建预设」表（见
//! [`load_user_presets`]）。所有查询函数都必须走那张表而不是只翻 `ENTRIES`，
//! 否则会出现「面板里看得到、AI 一切换就报未知预设」这种半拉子状态。

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use super::scene_store::LightingParams;

/// 预设元信息 + 参数。`mood` 是空格分隔的关键词，供 LLM / 搜索匹配用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LightingPreset {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub mood: Vec<String>,
    #[serde(default)]
    pub params: LightingParams,
    /// true = 用户在设置面板里自己调出来的；内置预设恒为 false
    #[serde(default)]
    pub custom: bool,
}

/// 用户自建预设表。
///
/// 用全局表而不是把 `Arc<Store>` 传来传去：`LightingOverride::resolve` 是纯函数、
/// 剧本事件和工具都直接调它，拿不到应用状态。预设数量是个位数，锁开销可忽略。
static USER_PRESETS: LazyLock<RwLock<Vec<LightingPreset>>> = LazyLock::new(Default::default);
static USER_PRESET_FILE: OnceLock<PathBuf> = OnceLock::new();

/// 自建预设的条数上限：够个人用，又能挡住「脚本一键生成一万条」把列表刷爆。
const MAX_USER_PRESETS: usize = 100;

/// 启动时加载 `<data_dir>/game_data/lighting_presets.json`。
///
/// 文件不存在算正常状态（还没建过预设）；解析失败只警告不报错——不能让一个坏掉的
/// 自建预设把整个软件启不起来，内置灯光必须照常能用。
pub fn load_user_presets(data_dir: &Path) {
    let path = data_dir.join("game_data").join("lighting_presets.json");
    let _ = USER_PRESET_FILE.set(path.clone());
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return,
    };
    match serde_json::from_str::<Vec<LightingPreset>>(&content) {
        Ok(list) => {
            let count = list.len();
            *USER_PRESETS.write().expect("自建光影预设锁被污染") = list;
            tracing::info!("已加载 {count} 个自建光影预设");
        },
        Err(e) => tracing::warn!("自建光影预设解析失败，已忽略该文件: {e}"),
    }
}

/// 全部自建预设（按 id 稳定排序，避免面板顺序每次启动跳一下）。
pub fn user_presets() -> Vec<LightingPreset> {
    let mut list = USER_PRESETS.read().expect("自建光影预设锁被污染").clone();
    list.sort_by(|a, b| a.id.cmp(&b.id));
    list
}

/// 新增或覆盖一个自建预设，返回落库后的正式 id。
///
/// id 由这里生成而不是让前端传：预设 id 会存进设置面板的 localStorage，一旦和
/// 内置预设撞名，`resolve` 就会在两张表之间摇摆，灯光切换出现解释不了的行为。
pub fn save_user_preset(
    name: &str,
    description: &str,
    mood: Vec<String>,
    params: LightingParams,
    replace_id: Option<&str>,
) -> Result<LightingPreset, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("预设名不能为空".to_string());
    }
    if name.chars().count() > 24 {
        return Err("预设名请控制在 24 个字以内".to_string());
    }
    // 重名会让「换成暖窗光」这类指令说不清指哪一个，内置和自建都不许撞名。
    if ENTRIES.iter().any(|e| e.1 == name) {
        return Err(format!("「{name}」是内置预设的名字，请换一个"));
    }
    if user_presets()
        .iter()
        .any(|p| p.name == name && Some(p.id.as_str()) != replace_id)
    {
        return Err(format!("已经有一个叫「{name}」的自建预设，请换一个"));
    }

    let preset = LightingPreset {
        id: match replace_id {
            Some(id) if is_user_preset(id) => id.to_string(),
            Some(id) => return Err(format!("要覆盖的预设不存在：{id}")),
            None => next_user_id(),
        },
        name: name.to_string(),
        description: description.trim().to_string(),
        mood,
        params,
        custom: true,
    };

    let mut list = USER_PRESETS.write().expect("自建光影预设锁被污染");
    let existed = list.iter().position(|p| p.id == preset.id);
    match existed {
        Some(idx) => list[idx] = preset.clone(),
        None => {
            if list.len() >= MAX_USER_PRESETS {
                return Err(format!("自建预设最多 {MAX_USER_PRESETS} 个，请先删掉一些"));
            }
            list.push(preset.clone());
        },
    }
    drop(list);

    write_user_presets()?;
    Ok(preset)
}

/// 删除自建预设；内置预设删不掉，直接报错。
pub fn delete_user_preset(id: &str) -> Result<(), String> {
    if normalize_id(id).is_some() && !is_user_preset(id) {
        return Err("内置预设不能删除".to_string());
    }
    let mut list = USER_PRESETS.write().expect("自建光影预设锁被污染");
    let before = list.len();
    list.retain(|p| p.id != id);
    if list.len() == before {
        return Err(format!("找不到自建预设：{id}"));
    }
    drop(list);
    write_user_presets()
}

fn is_user_preset(id: &str) -> bool {
    USER_PRESETS
        .read()
        .expect("自建光影预设锁被污染")
        .iter()
        .any(|p| p.id == id)
}

/// id 用毫秒时间戳：删掉再建也不会把旧 id 复用给另一个样子的灯。
fn next_user_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("custom_{millis}")
}

fn write_user_presets() -> Result<(), String> {
    let Some(path) = USER_PRESET_FILE.get() else {
        return Err("自建光影预设还没完成初始化".to_string());
    };
    let list = USER_PRESETS.read().expect("自建光影预设锁被污染").clone();
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建 {} 失败: {e}", parent.display()))?;
    }
    std::fs::write(path, json).map_err(|e| format!("写入 {} 失败: {e}", path.display()))
}

type Builder = fn() -> LightingParams;

/// 「暖高光 + 冷阴影 + 柔 bloom + 轻暗角」——多数 eden 室内场景的公共底子。
///
/// 叠加层用 `screen`：光应该往画面上「加」，`soft-light` / `overlay` 配深色外圈
/// 会变成整幅压暗，看着像关灯不像打光。外圈颜色写 #000000 即可，screen 对黑色恒等。
fn eden_base() -> LightingParams {
    let mut p = LightingParams::default();
    p.overlay_enabled = true;
    p.blend_mode = "screen".into();
    p.directional_enabled = true;
    p.bloom_enabled = true;
    p.vignette_enabled = true;
    p.grade_enabled = true;
    p.character.rim_enabled = true;
    p
}

fn warm_window() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.95;
    p.background.contrast = 1.15;
    p.background.saturation = 0.98;
    p.character.brightness = 1.02;
    p.character.contrast = 1.06;
    p.light_x = 74;
    p.light_y = 26;
    p.overlay_color1 = "#ffd9a0".into();
    p.overlay_color2 = "#000000".into();
    p.overlay_radius = 34;
    p.overlay_opacity = 0.24;
    p.light_angle = 60.6;
    p.light_warm_color = "#ffd7a1".into();
    p.shadow_cool_color = "#6d82a6".into();
    p.light_softness = 0.34;
    p.light_strength = 0.2;
    p.bloom_radius = 12;
    p.bloom_intensity = 0.14;
    p.vignette_strength = 0.32;
    p.vignette_size = 60;
    p.grade_warm_color = "#ff8a00".into();
    p.grade_cool_color = "#5277ad".into();
    p.grade_strength = 0.42;
    p.character.rim_color = "rgba(255,227,184,0.85)".into();
    p.character.rim_dx = 6;
    p.character.rim_dy = -5;
    p.character.rim_blur = 8;
    p.breathing_enabled = true;
    p.breathing_period = 9.0;
    p.breathing_amount = 0.1;
    p
}

fn backlight_silhouette() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.8;
    p.background.contrast = 1.24;
    p.background.saturation = 0.82;
    p.character.brightness = 0.72;
    p.character.contrast = 1.2;
    p.character.saturation = 0.7;
    p.light_x = 50;
    p.light_y = 34;
    p.overlay_color1 = "#fff0cf".into();
    p.overlay_color2 = "#101a2c".into();
    p.overlay_radius = 52;
    p.overlay_opacity = 0.31;
    p.light_angle = 0.0;
    p.light_warm_color = "#ffeec2".into();
    p.shadow_cool_color = "#0c1524".into();
    p.light_softness = 0.4;
    p.light_strength = 0.30;
    p.bloom_radius = 30;
    p.bloom_intensity = 0.23;
    p.vignette_strength = 0.6;
    p.vignette_size = 40;
    p.grade_warm_color = "#ff9b00".into();
    p.grade_cool_color = "#4f72b0".into();
    p.grade_strength = 0.36;
    p.character.rim_color = "#fff3d6".into();
    p.character.rim_dx = 0;
    p.character.rim_dy = -16;
    p.character.rim_blur = 20;
    p
}

fn moonlit_night() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.66;
    p.background.contrast = 1.16;
    p.background.saturation = 0.66;
    p.character.brightness = 0.82;
    p.character.saturation = 0.78;
    p.light_x = 78;
    p.light_y = 18;
    p.overlay_color1 = "#cfe3ff".into();
    p.overlay_color2 = "#070d1a".into();
    p.overlay_radius = 70;
    p.overlay_opacity = 0.25;
    p.light_angle = 57.3;
    p.light_warm_color = "#d7e8ff".into();
    p.shadow_cool_color = "#050b16".into();
    p.light_softness = 0.68;
    p.light_strength = 0.23;
    p.bloom_radius = 26;
    p.bloom_intensity = 0.13;
    p.vignette_strength = 0.62;
    p.vignette_size = 42;
    p.grade_warm_color = "#3178ce".into();
    p.grade_cool_color = "#446cbb".into();
    p.grade_strength = 0.34;
    p.character.rim_color = "#d9e9ff".into();
    p.character.rim_dx = 12;
    p.character.rim_dy = -14;
    p.character.rim_blur = 16;
    p.breathing_enabled = true;
    p.breathing_period = 12.0;
    p.breathing_amount = 0.08;
    p
}

fn dusk_sunset() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.96;
    p.background.contrast = 1.14;
    p.background.saturation = 1.08;
    p.character.brightness = 1.02;
    p.light_x = 22;
    p.light_y = 62;
    p.overlay_color1 = "#ffb26b".into();
    p.overlay_color2 = "#2b1a3a".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.28;
    p.light_angle = 256.4;
    p.light_warm_color = "#ffb877".into();
    p.shadow_cool_color = "#2a1c3e".into();
    p.light_softness = 0.6;
    p.light_strength = 0.27;
    p.bloom_radius = 24;
    p.bloom_intensity = 0.19;
    p.vignette_strength = 0.4;
    p.vignette_size = 56;
    p.grade_warm_color = "#ff6900".into();
    p.grade_cool_color = "#6c53ac".into();
    p.grade_strength = 0.4;
    p.character.rim_color = "#ffcf9a".into();
    p.character.rim_dx = -16;
    p.character.rim_dy = -6;
    p.character.rim_blur = 15;
    p
}

fn candlelight() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.74;
    p.background.contrast = 1.2;
    p.background.saturation = 0.88;
    p.character.brightness = 1.0;
    p.character.contrast = 1.08;
    p.light_x = 50;
    p.light_y = 62;
    p.overlay_color1 = "#ffc072".into();
    p.overlay_color2 = "#0d0a12".into();
    p.overlay_radius = 46;
    p.overlay_opacity = 0.33;
    p.light_angle = 180.0;
    p.light_warm_color = "#ffc27a".into();
    p.shadow_cool_color = "#0a0710".into();
    p.light_softness = 0.34;
    p.light_strength = 0.28;
    p.bloom_radius = 22;
    p.bloom_intensity = 0.17;
    p.vignette_strength = 0.7;
    p.vignette_size = 34;
    p.grade_warm_color = "#ff8000".into();
    p.grade_cool_color = "#8053ac".into();
    p.grade_strength = 0.32;
    p.character.rim_color = "#ffcf94".into();
    p.character.rim_dx = 0;
    p.character.rim_dy = -8;
    p.character.rim_blur = 18;
    p.breathing_enabled = true;
    p.breathing_period = 2.6;
    p.breathing_amount = 0.3;
    p
}

fn neon_night() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.8;
    p.background.contrast = 1.26;
    p.background.saturation = 1.22;
    p.character.brightness = 0.96;
    p.character.saturation = 1.05;
    p.light_x = 30;
    p.light_y = 40;
    p.overlay_color1 = "#ff5fa8".into();
    p.overlay_color2 = "#08131f".into();
    p.overlay_radius = 66;
    p.overlay_opacity = 0.28;
    p.light_angle = 285.7;
    p.light_warm_color = "#4de1ff".into();
    p.shadow_cool_color = "#0a0f1c".into();
    p.light_softness = 0.42;
    p.light_strength = 0.26;
    p.bloom_radius = 28;
    p.bloom_intensity = 0.22;
    p.vignette_strength = 0.56;
    p.vignette_size = 46;
    p.grade_warm_color = "#ff0064".into();
    p.grade_cool_color = "#00c7ff".into();
    p.grade_strength = 0.44;
    p.character.rim_color = "#66e6ff".into();
    p.character.rim_dx = -14;
    p.character.rim_dy = -8;
    p.character.rim_blur = 16;
    p.breathing_enabled = true;
    p.breathing_period = 4.5;
    p.breathing_amount = 0.16;
    p
}

fn morning_soft() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 1.05;
    p.background.contrast = 1.02;
    p.background.saturation = 0.96;
    p.character.brightness = 1.06;
    p.light_x = 26;
    p.light_y = 22;
    p.overlay_color1 = "#fff4dd".into();
    p.overlay_color2 = "#3d4a5e".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.21;
    p.light_angle = 303.3;
    p.light_warm_color = "#fff6e4".into();
    p.shadow_cool_color = "#425066".into();
    p.light_softness = 0.74;
    p.light_strength = 0.20;
    p.bloom_radius = 22;
    p.bloom_intensity = 0.16;
    p.vignette_strength = 0.26;
    p.vignette_size = 64;
    p.grade_warm_color = "#ffa800".into();
    p.grade_cool_color = "#5379ac".into();
    p.grade_strength = 0.24;
    p.character.rim_color = "#fff6e0".into();
    p.character.rim_dx = -12;
    p.character.rim_dy = -10;
    p.character.rim_blur = 14;
    p.breathing_enabled = true;
    p.breathing_period = 11.0;
    p.breathing_amount = 0.08;
    p
}

fn overcast_gray() -> LightingParams {
    let mut p = LightingParams::default();
    p.background.brightness = 0.95;
    p.background.contrast = 0.92;
    p.background.saturation = 0.62;
    p.character.brightness = 0.98;
    p.character.saturation = 0.72;
    p.directional_enabled = true;
    p.light_angle = 0.0;
    p.light_warm_color = "#e8eef4".into();
    p.shadow_cool_color = "#3b444f".into();
    p.light_softness = 0.86;
    p.light_strength = 0.14;
    p.grade_enabled = true;
    p.grade_warm_color = "#537dac".into();
    p.grade_cool_color = "#5380ac".into();
    p.grade_strength = 0.3;
    p.vignette_enabled = true;
    p.vignette_strength = 0.24;
    p.vignette_size = 68;
    p
}

fn rainy_gloom() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.78;
    p.background.contrast = 1.1;
    p.background.saturation = 0.68;
    p.character.brightness = 0.92;
    p.character.saturation = 0.82;
    p.light_x = 50;
    p.light_y = 14;
    p.overlay_color1 = "#b9cede".into();
    p.overlay_color2 = "#0e1620".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.22;
    p.light_angle = 0.0;
    p.light_warm_color = "#c6d8e6".into();
    p.shadow_cool_color = "#0b131c".into();
    p.light_softness = 0.78;
    p.light_strength = 0.18;
    p.bloom_radius = 26;
    p.bloom_intensity = 0.11;
    p.vignette_strength = 0.52;
    p.vignette_size = 44;
    p.grade_warm_color = "#5385ac".into();
    p.grade_cool_color = "#537eac".into();
    p.grade_strength = 0.36;
    p.character.rim_color = "#cfe0ec".into();
    p.character.rim_dx = 0;
    p.character.rim_dy = -12;
    p.character.rim_blur = 15;
    p.breathing_enabled = true;
    p.breathing_period = 8.0;
    p.breathing_amount = 0.12;
    p
}

fn snow_bright() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 1.05;
    p.background.contrast = 1.04;
    p.background.saturation = 0.8;
    p.character.brightness = 1.08;
    p.light_x = 62;
    p.light_y = 20;
    p.overlay_color1 = "#ffffff".into();
    p.overlay_color2 = "#5a6b80".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.18;
    p.light_angle = 35.4;
    p.light_warm_color = "#ffffff".into();
    p.shadow_cool_color = "#63758c".into();
    p.light_softness = 0.8;
    p.light_strength = 0.18;
    p.bloom_radius = 30;
    p.bloom_intensity = 0.20;
    p.vignette_strength = 0.22;
    p.vignette_size = 66;
    p.grade_warm_color = "#0062ff".into();
    p.grade_cool_color = "#537aac".into();
    p.grade_strength = 0.22;
    p.character.rim_color = "#ffffff".into();
    p.character.rim_dx = 10;
    p.character.rim_dy = -10;
    p.character.rim_blur = 16;
    p
}

fn forest_dapple() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.9;
    p.background.contrast = 1.14;
    p.background.saturation = 1.02;
    p.character.brightness = 1.04;
    p.light_x = 38;
    p.light_y = 12;
    p.overlay_color1 = "#e8ff9f".into();
    p.overlay_color2 = "#10240f".into();
    p.overlay_radius = 58;
    p.overlay_opacity = 0.25;
    p.light_angle = 330.7;
    p.light_warm_color = "#e9ffa8".into();
    p.shadow_cool_color = "#0e2110".into();
    p.light_softness = 0.46;
    p.light_strength = 0.26;
    p.bloom_radius = 24;
    p.bloom_intensity = 0.18;
    p.vignette_strength = 0.46;
    p.vignette_size = 50;
    p.grade_warm_color = "#9dff00".into();
    p.grade_cool_color = "#53ac68".into();
    p.grade_strength = 0.34;
    p.character.rim_color = "#eaffb0".into();
    p.character.rim_dx = -10;
    p.character.rim_dy = -12;
    p.character.rim_blur = 14;
    p.breathing_enabled = true;
    p.breathing_period = 6.5;
    p.breathing_amount = 0.2;
    p
}

fn classroom_noon() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 1.04;
    p.background.contrast = 1.08;
    p.background.saturation = 0.98;
    p.character.brightness = 1.02;
    p.light_x = 82;
    p.light_y = 32;
    p.overlay_color1 = "#fff1cf".into();
    p.overlay_color2 = "#2f3b52".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.20;
    p.light_angle = 72.4;
    p.light_warm_color = "#fff2d2".into();
    p.shadow_cool_color = "#33405a".into();
    p.light_softness = 0.66;
    p.light_strength = 0.21;
    p.bloom_radius = 18;
    p.bloom_intensity = 0.13;
    p.vignette_strength = 0.3;
    p.vignette_size = 62;
    p.grade_warm_color = "#ffb300".into();
    p.grade_cool_color = "#5377ac".into();
    p.grade_strength = 0.24;
    p.character.rim_color = "#fff0cc".into();
    p.character.rim_dx = 16;
    p.character.rim_dy = -8;
    p.character.rim_blur = 12;
    p
}

fn stage_spotlight() -> LightingParams {
    let mut p = LightingParams::default();
    p.background.brightness = 0.52;
    p.background.contrast = 1.3;
    p.background.saturation = 0.6;
    p.character.brightness = 1.1;
    p.character.contrast = 1.1;
    p.overlay_enabled = true;
    p.blend_mode = "soft-light".into();
    p.light_x = 50;
    p.light_y = 24;
    p.overlay_color1 = "#fff6e0".into();
    p.overlay_color2 = "#04060a".into();
    p.overlay_radius = 30;
    p.overlay_opacity = 0.41;
    p.directional_enabled = true;
    p.light_angle = 0.0;
    p.light_warm_color = "#fff4dd".into();
    p.shadow_cool_color = "#04070c".into();
    p.light_softness = 0.26;
    p.light_strength = 0.32;
    p.bloom_enabled = true;
    p.bloom_radius = 26;
    p.bloom_intensity = 0.17;
    p.vignette_enabled = true;
    p.vignette_strength = 0.85;
    p.vignette_size = 26;
    p.character.rim_enabled = true;
    p.character.rim_color = "#fff5e2".into();
    p.character.rim_dx = 0;
    p.character.rim_dy = -14;
    p.character.rim_blur = 22;
    p.breathing_enabled = true;
    p.breathing_period = 5.0;
    p.breathing_amount = 0.12;
    p
}

fn screen_glow() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.6;
    p.background.contrast = 1.14;
    p.background.saturation = 0.74;
    p.character.brightness = 0.94;
    p.character.saturation = 0.86;
    p.light_x = 50;
    p.light_y = 58;
    p.overlay_color1 = "#9fd8ff".into();
    p.overlay_color2 = "#050810".into();
    p.overlay_radius = 44;
    p.overlay_opacity = 0.30;
    p.light_angle = 180.0;
    p.light_warm_color = "#a8dcff".into();
    p.shadow_cool_color = "#04070e".into();
    p.light_softness = 0.4;
    p.light_strength = 0.24;
    p.bloom_radius = 24;
    p.bloom_intensity = 0.15;
    p.vignette_strength = 0.66;
    p.vignette_size = 38;
    p.grade_warm_color = "#189ce7".into();
    p.grade_cool_color = "#4c6db3".into();
    p.grade_strength = 0.38;
    p.character.rim_color = "#b6e2ff".into();
    p.character.rim_dx = 0;
    p.character.rim_dy = 10;
    p.character.rim_blur = 16;
    p.breathing_enabled = true;
    p.breathing_period = 3.4;
    p.breathing_amount = 0.18;
    p
}

fn thriller_red() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.68;
    p.background.contrast = 1.32;
    p.background.saturation = 0.9;
    p.character.brightness = 0.9;
    p.character.contrast = 1.16;
    p.light_x = 18;
    p.light_y = 46;
    p.overlay_color1 = "#ff4d4d".into();
    p.overlay_color2 = "#0b0206".into();
    p.overlay_radius = 62;
    p.overlay_opacity = 0.30;
    p.light_angle = 274.0;
    p.light_warm_color = "#ff5b52".into();
    p.shadow_cool_color = "#0a0206".into();
    p.light_softness = 0.4;
    p.light_strength = 0.29;
    p.bloom_radius = 22;
    p.bloom_intensity = 0.14;
    p.vignette_strength = 0.74;
    p.vignette_size = 34;
    p.grade_warm_color = "#ff1900".into();
    p.grade_cool_color = "#c33c82".into();
    p.grade_strength = 0.42;
    p.character.rim_color = "#ff7a68".into();
    p.character.rim_dx = -14;
    p.character.rim_dy = -6;
    p.character.rim_blur = 16;
    p.breathing_enabled = true;
    p.breathing_period = 2.2;
    p.breathing_amount = 0.26;
    p
}

fn dream_pastel() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 1.05;
    p.background.contrast = 0.94;
    p.background.saturation = 1.06;
    p.character.brightness = 1.08;
    p.light_x = 50;
    p.light_y = 30;
    p.overlay_color1 = "#ffe1f4".into();
    p.overlay_color2 = "#b9c8f5".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.20;
    p.light_angle = 0.0;
    p.light_warm_color = "#ffe6f6".into();
    p.shadow_cool_color = "#c3d0f7".into();
    p.light_softness = 0.88;
    p.light_strength = 0.16;
    p.bloom_radius = 34;
    p.bloom_intensity = 0.24;
    p.vignette_strength = 0.18;
    p.vignette_size = 72;
    p.grade_warm_color = "#ff0094".into();
    p.grade_cool_color = "#004cff".into();
    p.grade_strength = 0.3;
    p.character.rim_color = "#fff0fa".into();
    p.character.rim_dx = 0;
    p.character.rim_dy = -14;
    p.character.rim_blur = 22;
    p.breathing_enabled = true;
    p.breathing_period = 10.0;
    p.breathing_amount = 0.14;
    p
}

fn golden_hour() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 1.02;
    p.background.contrast = 1.16;
    p.background.saturation = 1.12;
    p.character.brightness = 1.06;
    p.character.saturation = 1.06;
    p.light_x = 16;
    p.light_y = 54;
    p.overlay_color1 = "#ffcf7a".into();
    p.overlay_color2 = "#3a2415".into();
    p.overlay_radius = 74;
    p.overlay_opacity = 0.29;
    p.light_angle = 266.2;
    p.light_warm_color = "#ffd488".into();
    p.shadow_cool_color = "#33210f".into();
    p.light_softness = 0.58;
    p.light_strength = 0.28;
    p.bloom_radius = 26;
    p.bloom_intensity = 0.21;
    p.vignette_strength = 0.36;
    p.vignette_size = 58;
    p.grade_warm_color = "#ff9600".into();
    p.grade_cool_color = "#b77f48".into();
    p.grade_strength = 0.38;
    p.character.rim_color = "#ffdca0".into();
    p.character.rim_dx = -18;
    p.character.rim_dy = -4;
    p.character.rim_blur = 16;
    p
}

fn night_ambient() -> LightingParams {
    let mut p = eden_base();
    p.background.brightness = 0.58;
    p.background.contrast = 1.12;
    p.background.saturation = 0.6;
    p.character.brightness = 0.86;
    p.character.saturation = 0.8;
    p.light_x = 40;
    p.light_y = 30;
    p.overlay_color1 = "#8fa9d8".into();
    p.overlay_color2 = "#04060e".into();
    p.overlay_radius = 60;
    p.overlay_opacity = 0.22;
    p.light_angle = 318.4;
    p.light_warm_color = "#9db6e0".into();
    p.shadow_cool_color = "#03050c".into();
    p.light_softness = 0.72;
    p.light_strength = 0.19;
    p.bloom_radius = 20;
    p.bloom_intensity = 0.10;
    p.vignette_strength = 0.68;
    p.vignette_size = 40;
    p.grade_warm_color = "#426ebd".into();
    p.grade_cool_color = "#446fbb".into();
    p.grade_strength = 0.32;
    p.character.rim_color = "#b9ccf0".into();
    p.character.rim_dx = 8;
    p.character.rim_dy = -12;
    p.character.rim_blur = 15;
    p.breathing_enabled = true;
    p.breathing_period = 13.0;
    p.breathing_amount = 0.07;
    p
}

fn sepia_memory() -> LightingParams {
    let mut p = LightingParams::default();
    p.background.brightness = 0.98;
    p.background.contrast = 1.14;
    p.background.saturation = 0.7;
    p.background.sepia = 0.55;
    p.character.brightness = 1.0;
    p.character.sepia = 0.35;
    p.character.glow_radius = 14;
    p.character.glow_color = "#d8b078".into();
    p.overlay_enabled = true;
    p.blend_mode = "soft-light".into();
    p.light_x = 50;
    p.light_y = 40;
    p.overlay_color1 = "#f0d9a8".into();
    p.overlay_color2 = "#3a2c1a".into();
    p.overlay_radius = 70;
    p.overlay_opacity = 0.25;
    p.light_angle = 0.0;
    p.vignette_enabled = true;
    p.vignette_strength = 0.62;
    p.vignette_size = 36;
    p.grade_enabled = true;
    p.grade_warm_color = "#d49a2b".into();
    p.grade_cool_color = "#ac8a53".into();
    p.grade_strength = 0.4;
    p.bloom_enabled = true;
    p.bloom_radius = 20;
    p.bloom_intensity = 0.11;
    p
}

/// 预设表：`(id, 显示名, 说明, 心情关键词, 构造函数)`。
const ENTRIES: &[(&str, &str, &str, &str, Builder)] = &[
    (
        "warm_window",
        "暖窗光",
        "午后窗边斜射的暖光，高光偏琥珀、阴影偏青蓝，接近 eden 室内演出",
        "午后 室内 窗边 日常 治愈 温暖 回忆",
        warm_window,
    ),
    (
        "backlight_silhouette",
        "逆光剪影",
        "强背光、人物压暗、边缘发白，适合登场与告白镜头",
        "逆光 剪影 登场 告白 震撼 强光",
        backlight_silhouette,
    ),
    (
        "moonlit_night",
        "冷月夜",
        "月光冷蓝、暗部压黑、轻微 bloom，适合夜晚谈心",
        "夜晚 月夜 室外 安静 忧伤 冷色",
        moonlit_night,
    ),
    (
        "dusk_sunset",
        "黄昏",
        "低角度橙紫渐变天光，暖冷交界明显",
        "黄昏 傍晚 天台 离别 感伤 晚霞",
        dusk_sunset,
    ),
    (
        "candlelight",
        "烛光",
        "小范围暖光源 + 快速呼吸，适合停电 / 祭典 / 深夜对坐",
        "烛光 夜晚 室内 停电 祭典 温暖 摇曳",
        candlelight,
    ),
    (
        "neon_night",
        "霓虹夜",
        "粉蓝霓虹撞色、强 bloom、彩色分离",
        "霓虹 城市 夜晚 赛博 便利店 狂欢",
        neon_night,
    ),
    (
        "morning_soft",
        "清晨柔光",
        "高亮低对比的柔和晨光，适合新的一天开场",
        "清晨 早晨 日出 清新 希望 日常",
        morning_soft,
    ),
    (
        "overcast_gray",
        "阴天平光",
        "去饱和、几乎无方向，情绪低落的留白",
        "阴天 灰暗 压抑 平淡 忧郁 无光",
        overcast_gray,
    ),
    (
        "rainy_gloom",
        "雨雾",
        "冷灰蓝压暗 + 大范围柔光，配合 Rain 特效",
        "雨天 下雨 雨夜 忧郁 潮湿 冷",
        rainy_gloom,
    ),
    (
        "snow_bright",
        "雪地强光",
        "高亮冷白 + 强 bloom，配合 Snow 特效",
        "雪 雪地 冬天 洁白 强光 寒冷",
        snow_bright,
    ),
    (
        "forest_dapple",
        "林间光斑",
        "绿叶滤下的黄绿光斑，呼吸幅度较大",
        "森林 树林 夏日 斑驳 自然 生机",
        forest_dapple,
    ),
    (
        "classroom_noon",
        "正午教室",
        "明亮中性偏暖，最安全的日常底光",
        "教室 学校 正午 白天 日常 明亮",
        classroom_noon,
    ),
    (
        "stage_spotlight",
        "舞台聚光",
        "小范围顶光 + 重暗角，背景几乎全黑",
        "舞台 聚光灯 演出 独白 焦点 黑暗",
        stage_spotlight,
    ),
    (
        "screen_glow",
        "屏幕冷光",
        "自下而上的青蓝冷光 + 快呼吸，深夜电脑前",
        "深夜 电脑 屏幕 蓝光 加班 孤独",
        screen_glow,
    ),
    (
        "thriller_red",
        "危险红光",
        "单侧血红 + 心跳式呼吸，冲突与危机",
        "危险 血腥 紧张 冲突 警报 恐怖",
        thriller_red,
    ),
    (
        "dream_pastel",
        "梦幻粉彩",
        "粉紫低对比 + 强 bloom，回忆与梦境",
        "梦幻 回忆 梦境 温柔 粉彩 童话",
        dream_pastel,
    ),
    (
        "golden_hour",
        "黄金时刻",
        "极低角度浓橙侧光，轮廓光最强",
        "夕阳 黄金 侧光 田野 海边 浓暖",
        golden_hour,
    ),
    (
        "night_ambient",
        "夜室内",
        "极暗环境 + 微弱冷光源，深夜房间常态",
        "深夜 室内 关灯 睡觉 安静 昏暗",
        night_ambient,
    ),
    (
        "sepia_memory",
        "旧照片",
        "棕褐去色 + 重暗角，闪回与老照片段落",
        "回忆 闪回 旧照片 过去 复古 褪色",
        sepia_memory,
    ),
];

/// 内置预设（不含用户自建的那份）。
fn builtin_presets() -> Vec<LightingPreset> {
    ENTRIES
        .iter()
        .map(|(id, name, desc, mood, build)| LightingPreset {
            id: (*id).to_string(),
            name: (*name).to_string(),
            description: (*desc).to_string(),
            mood: mood.split_whitespace().map(str::to_string).collect(),
            params: build(),
            custom: false,
        })
        .collect()
}

/// 全部预设（含完整参数）：内置在前、自建在后，面板和 LLM 看到的是同一张表。
pub fn presets() -> Vec<LightingPreset> {
    let mut out = builtin_presets();
    out.extend(user_presets());
    out
}

/// 按 id 找预设，内置优先，再做大小写纠正。所有查询都走这里，避免各处规则不一致。
fn find_preset(id: &str) -> Option<LightingPreset> {
    let all = presets();
    all.iter()
        .find(|p| p.id == id)
        .cloned()
        .or_else(|| all.into_iter().find(|p| p.id.eq_ignore_ascii_case(id)))
}

/// 预设 id 列表（剧本编辑器下拉与校验共用）。
pub fn preset_ids() -> Vec<String> {
    presets().into_iter().map(|p| p.id).collect()
}

/// 按 id 取预设参数；未知 id 返回 `None`。大小写不匹配时给出规范写法。
pub fn resolve(id: &str) -> Option<LightingParams> {
    find_preset(id).map(|p| p.params)
}

/// 校验用：返回规范化后的预设 id（大小写纠正），完全未知则 `None`。
pub fn normalize_id(id: &str) -> Option<String> {
    find_preset(id).map(|p| p.id)
}

/// 按 id 取展示名（如 `warm_window` → 「暖窗光」）。
///
/// 回给 LLM 的状态要带名字：它判断「用户要的暖窗光是否已经在打」时用的是中文，
/// 只给 id 就得再调一次 `lighting_list_presets` 才能对上。
pub fn preset_name(id: &str) -> Option<String> {
    find_preset(id).map(|p| p.name)
}

/// 供 LLM / 插件读的轻量清单（不含参数，省 token）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub mood: Vec<String>,
    /// 自建预设要标出来：用户说「我那个黄昏」时，LLM 知道这不是内置名
    pub custom: bool,
}

pub fn summaries() -> Vec<PresetSummary> {
    presets()
        .into_iter()
        .map(|p| PresetSummary {
            id: p.id,
            name: p.name,
            description: p.description,
            mood: p.mood,
            custom: p.custom,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::scene_store::FilterParams;
    use super::*;

    /// 内置表是编译期常量，读它的测试不该被别的测试临时塞进来的自建预设干扰。
    #[test]
    fn ids_are_unique() {
        let builtins = builtin_presets();
        let mut ids: Vec<&str> = builtins.iter().map(|p| p.id.as_str()).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "预设 id 重复");
    }

    #[test]
    fn defaults_keep_neutral_filters() {
        let f = FilterParams::default();
        assert_eq!(f.brightness, 1.0);
        assert_eq!(f.contrast, 1.0);
        assert_eq!(f.saturation, 1.0);
    }

    #[test]
    fn every_preset_serializes_with_all_fields() {
        for preset in builtin_presets() {
            let json = serde_json::to_value(&preset.params).unwrap();
            assert!(json.get("light_angle").is_some(), "{} 缺字段", preset.id);
            assert!(json.get("bloom_radius").is_some(), "{} 缺字段", preset.id);
        }
    }

    /// CSS 罗盘角（0 = 光源在正上方，顺时针）。舞台按 16:9 计算，横向偏移要乘 16/9
    /// 才是画面上的真实方向。
    fn angle_toward_light(x: i32, y: i32) -> f64 {
        let a = ((x - 50) as f64 * 16.0)
            .atan2((50 - y) as f64 * 9.0)
            .to_degrees();
        if a < 0.0 { a + 360.0 } else { a }
    }

    /// 方向光和冷暖分离按 `light_angle` 铺渐变，径向光斑按 `light_x/light_y` 画。两处
    /// 写反就会出现「窗在右上、阴影也压在右上」的裂开效果，所以钉死在一起。
    #[test]
    fn directional_angle_points_at_the_light() {
        for preset in builtin_presets() {
            let p = &preset.params;
            let delta = (p.light_angle - angle_toward_light(p.light_x, p.light_y)).abs();
            let delta = delta.min(360.0 - delta);
            assert!(
                delta < 0.6,
                "{}: light_angle={} 与灯光位置 ({},{}) 相差 {delta:.1} 度",
                preset.id,
                p.light_angle,
                p.light_x,
                p.light_y
            );
        }
    }

    /// 自建预设表是进程级全局，几个测试共用同一份，必须串行。
    static USER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_user_presets() -> std::sync::MutexGuard<'static, ()> {
        USER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 把落盘指向临时目录；`USER_PRESET_FILE` 是 OnceLock，首个测试设定后大家共用。
    fn init_temp_store() {
        let dir = std::env::temp_dir().join("lingchat_lighting_preset_test");
        let _ = std::fs::create_dir_all(&dir);
        load_user_presets(&dir);
    }

    fn sample_params() -> LightingParams {
        let mut p = LightingParams::default();
        p.overlay_enabled = true;
        p.light_x = 80;
        p.light_y = 20;
        p.bloom_radius = 14;
        p
    }

    #[test]
    fn user_preset_shows_up_in_every_lookup() {
        let _guard = lock_user_presets();
        init_temp_store();
        let saved = save_user_preset("测试黄昏", "", vec![], sample_params(), None)
            .expect("自建预设应能保存");
        assert!(saved.custom);
        assert!(resolve(&saved.id).is_some(), "resolve 要认自建预设");
        assert_eq!(preset_name(&saved.id).as_deref(), Some("测试黄昏"));
        assert!(
            summaries().iter().any(|s| s.id == saved.id && s.custom),
            "LLM 拿到的清单里要带上自建预设，并标出不是内置的"
        );
        assert!(delete_user_preset(&saved.id).is_ok());
        assert!(resolve(&saved.id).is_none(), "删掉后不该还能解析");
    }

    #[test]
    fn user_preset_cannot_shadow_a_builtin_name() {
        let _guard = lock_user_presets();
        init_temp_store();
        let name = builtin_presets()[0].name.clone();
        let err = save_user_preset(&name, "", vec![], sample_params(), None)
            .expect_err("和内置预设重名必须被拒绝");
        assert!(err.contains("内置预设"), "报错要说明原因，实际: {err}");
    }

    #[test]
    fn builtin_presets_are_read_only() {
        let _guard = lock_user_presets();
        init_temp_store();
        assert!(
            delete_user_preset("warm_window").is_err(),
            "内置预设不能被删除"
        );
        assert!(
            save_user_preset("覆盖测试", "", vec![], sample_params(), Some("warm_window")).is_err(),
            "不能借 replace_id 改写内置预设"
        );
    }

    #[test]
    fn saving_with_replace_id_updates_in_place() {
        let _guard = lock_user_presets();
        init_temp_store();
        let first = save_user_preset("改名前", "", vec![], sample_params(), None).unwrap();
        let second = save_user_preset("改名后", "", vec![], sample_params(), Some(&first.id))
            .expect("编辑自己的预设应该原地覆盖");
        assert_eq!(first.id, second.id, "编辑不该换 id，否则面板选中的会跳掉");
        assert_eq!(preset_name(&first.id).as_deref(), Some("改名后"));
        assert_eq!(
            user_presets().iter().filter(|p| p.id == first.id).count(),
            1,
            "覆盖后不能留两份"
        );
        delete_user_preset(&first.id).unwrap();
    }
}
