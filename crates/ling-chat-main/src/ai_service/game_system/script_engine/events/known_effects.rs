//! 内置背景特效清单 —— 本文件由脚本生成，请勿手改。
//!
//! 生成命令: pnpm gen:effects
//! 唯一真相是前端注册表 src/components/game/standard/particles/index.ts。
//!
//! 特效由前端渲染，Rust 不参与，这份清单只用来让剧本编辑器给作者报
//! 「这不是内置特效」。正因为如此它必须是派生品 —— 手抄一份的话，抄漏的
//! 症状只是警告不再触发，不报错，很难查。
//!
//! 前端把特效分成氛围与天气两层，这里刻意拍平：剧本只写一个特效值，
//! 落在哪一层由前端决定。

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
    "Glitch",
    "Shake",
    "Flash",
    "Blackout",
    "Tear",
    "Static",
    "Invert",
    "BloodDrip",
    "Veins",
    "BSOD",
    "UiCorrupt",
    "BloodUI",
];
