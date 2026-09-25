//! 共享数据模型：桌面端与移动端都使用同一套参数结构。

use serde::{Deserialize, Serialize};

/// `show` 命令的参数。
///
/// 所有尺寸/坐标都是 **逻辑像素（dp）**，由 Kotlin 侧乘以 density 后
/// 再交给 `WindowManager`，避免不同 DPI 设备上桌宠大小不一致。
///
/// 尺寸不再由前端传入：手机端窗口大小按**屏幕宽度的比例**计算
/// （收起态 1/6 屏宽、展开态 2/5 屏宽），这个比例只有原生侧知道
/// 真实屏幕宽度，因此由 Kotlin 自己算。前端只传缩放系数。
///
/// 需要 `Serialize`：移动端要把参数转发给原生插件（`run_mobile_plugin`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowArgs {
    /// 桌宠缩放系数（来自设置），原生侧在此基础上再乘屏幕比例。
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// 初始 X 坐标（dp），屏幕左上角为原点。
    #[serde(default)]
    pub x: f64,
    /// 初始 Y 坐标（dp），屏幕左上角为原点。
    #[serde(default)]
    pub y: f64,
}

fn default_scale() -> f64 {
    1.0
}

/// `move_pet` 命令的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveArgs {
    pub x: f64,
    pub y: f64,
}

/// `set_size` 命令的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeArgs {
    pub width: f64,
    pub height: f64,
}

/// `set_touchable` 命令的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchableArgs {
    pub touchable: bool,
}

/// `set_expanded` 命令的参数。
///
/// 手机端没有鼠标悬停，展开/收起由前端「点头像」触发，
/// 对应桌面端 `mouseenter/mouseleave` 的行为。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpandedArgs {
    /// true = 展开（头像 + 输入框）；false = 收起（仅头像）。
    pub expanded: bool,
}

/// 权限 / 可见性查询结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetStatus {
    /// 当前平台是否支持系统级悬浮窗。
    pub supported: bool,
    /// 是否已获得「显示在其他应用上层」权限。
    pub granted: bool,
    /// 悬浮窗当前是否可见。
    pub visible: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("当前平台不支持系统级悬浮窗")]
    NotSupported,
    #[error("尚未获得悬浮窗权限")]
    PermissionDenied,
    #[error("悬浮窗未显示")]
    NotVisible,
    #[error("原生调用失败: {0}")]
    Native(String),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
