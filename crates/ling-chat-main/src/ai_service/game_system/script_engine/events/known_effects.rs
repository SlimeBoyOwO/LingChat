//! 内置背景特效清单 —— 本文件由脚本生成，请勿手改。
//!
//! 生成命令: pnpm gen:effects
//! 唯一真相是前端注册表 src/components/game/standard/particles/index.ts。
//!
//! 特效由前端渲染，Rust 不参与，这份清单只用来让剧本编辑器给作者报
//! 「这不是内置特效」，以及让后端判断某个特效属于哪一层。正因为如此它
//! 必须是派生品 —— 手抄一份的话，抄漏的症状只是判断失灵，不报错，很难查。

/// 全部内置特效的 key，顺序与注册表一致。
///
/// 大小写敏感：前端按 === 比较，starfield 和 Starfield 都不渲染。
/// 不在这个列表里的值（含约定的 None）会清空当前特效。
pub const KNOWN_EFFECTS: &[&str] = &[
    "StarField",
    "Sakura",
    "Fireworks",
    "BA",
    "Fireflies",
    "MeteorShower",
    "Drizzle",
    "Rain",
    "Thunderstorm",
    "Snow",
    "Blizzard",
    "Fog",
];

/// 天气层特效的 key。氛围层与天气层各自单选、互不覆盖。
///
/// 前端按这个集合决定特效落在哪一层；后端据此把天气名的特效写进
/// `weather_effect`，从而能检测天气变化并生成旁白。
pub const WEATHER_EFFECTS: &[&str] =
    &["Drizzle", "Rain", "Thunderstorm", "Snow", "Blizzard", "Fog"];

/// 是否天气层特效（大小写敏感，与前端按 === 比较一致）。
pub fn is_weather_effect(effect: &str) -> bool {
    WEATHER_EFFECTS.contains(&effect)
}
