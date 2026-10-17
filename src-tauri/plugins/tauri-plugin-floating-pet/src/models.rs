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
    /// 窗口宽度（dp）。`<= 0` 表示「只改高度，宽度由原生独占」。
    pub width: f64,
    /// 窗口高度（dp）。仅在 `logical_height <= 0` 时生效。
    pub height: f64,
    /// 逻辑画布高度（dp，**未缩放**）。`> 0` 时原生忽略 `height`，
    /// 自己按 `逻辑高度 × 当前缩放系数` 算实际高度。
    ///
    /// 页面应当优先用这个口径回报高度：按「实际 dp」回报时，那个数字
    /// 是「逻辑高度 × 页面手里的缩放系数」，一旦页面手里的系数过期，
    /// 就会把与窗口宽度不匹配的高度写进窗口（宽度已展开、高度还是收起态），
    /// 表现就是「展开后一大片空白」。改报逻辑高度后，这个不变式由原生保证。
    #[serde(default)]
    pub logical_height: f64,
}

/// `set_touchable` 命令的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchableArgs {
    pub touchable: bool,
}

/// `set_ime_focus` 命令的参数。
///
/// 悬浮窗展开时**不能**无条件摘掉 `FLAG_NOT_FOCUSABLE` —— 那会让它在整个展开
/// 期间持有输入焦点，其他 App 的键盘弹不出来、返回键也先被它吃掉。
/// 现在改成由页面的输入框 `focus` / `blur` 驱动：
///
/// - `focused = true`  → 摘掉 `FLAG_NOT_FOCUSABLE` 并弹出输入法；
/// - `focused = false` → 收起输入法并把标志加回去，让出焦点。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImeFocusArgs {
    pub focused: bool,
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

/// `set_scale` 命令的参数 —— 设置页里的「桌宠大小」。
///
/// ## 为什么需要一个独立命令
///
/// 悬浮窗的**宽度**由原生独占（`screenBasis × 比例 × petScale`，见
/// `FloatingPetPlugin.collapsedSize/expandedSize`），前端只负责高度。
/// 所以用户拖动「桌宠大小」滑块时，前端除了写 store 之外**必须**把新系数
/// 送回原生重算窗口宽度 —— 否则原生手里还是旧系数，窗口尺寸不变，
/// 表现就是「滑块拖了等于没拖」。
///
/// 曾经想复用 `show`（它也接收 `scale`），但 `show` 是「把 WebView 搬进
/// 悬浮窗」的完整流程（先还给 Activity 再搬出来），用来改个尺寸代价太大，
/// 而且会闪一下。这里只做「重算尺寸 + 重排」，不动 WebView 的宿主。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleArgs {
    /// 用户缩放系数（设置页滑块，0.7–1.3）。原生会再夹到 0.5–2.0。
    pub scale: f64,
}

/// 权限 / 可见性 / 窗口几何查询结果。
///
/// 需要 `Deserialize`：Android 侧这些字段由 Kotlin 插件返回
/// （见 `FloatingPetPlugin.status`），Rust 只做转发。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetStatus {
    /// 当前平台是否支持系统级悬浮窗。
    pub supported: bool,
    /// 是否已获得「显示在其他应用上层」权限。
    pub granted: bool,
    /// 悬浮窗当前是否可见。
    pub visible: bool,
    /// WebView 当前是否已被搬进悬浮窗。
    ///
    /// 与 `visible` 的区别：`visible` 是「原生侧认为桌宠在运行」，
    /// `detached` 是「WebView 此刻真的不在 Activity 里」。页面用它在
    /// **不依赖任何原生事件**的情况下判断自己是否已被搬回 Activity。
    #[serde(default)]
    pub detached: bool,
    /// 逻辑画布 → 窗口的缩放系数（`窗口宽度 / FLOATING_LOGICAL_WIDTH`）。
    ///
    /// 前端用它做整体 `transform: scale()`。**必须由原生给**：页面从
    /// `window.innerWidth` 自算会踩到「原生刚改完尺寸、WebView 视口还没
    /// 跟上」的滞后窗口，算出的系数偏小 → 内容只占窗口一角、大片空白。
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// 悬浮窗宽度（dp）。
    #[serde(default)]
    pub width: f64,
    /// 悬浮窗高度（dp）。
    #[serde(default)]
    pub height: f64,
}

/// 「屏幕视奸」（主动窥屏）的取景结果。
///
/// 需要 `Deserialize`：字段由 Kotlin 插件的 `captureScreen` 返回，
/// Rust 只做转发（见 `mobile.rs::capture_screen`）。
///
/// ⚠️ 与其它命令不同，**失败不抛错**：`ok = false` 表示「功能未启用」
/// （用户还没在系统设置里开启无障碍服务）这种**正常状态**，调用方应当据此
/// 降级，而不是当成异常。只有「写文件失败」这类真异常才会 reject。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCapture {
    /// 是否真的截到了图。
    pub ok: bool,
    /// 成功时 JPEG 的绝对路径（位于 App 的 `cacheDir`，每次覆盖）。
    #[serde(default)]
    pub path: String,
    /// 最近观察到的前台包名，用作窥屏的语境；拿不到时为空串。
    #[serde(default)]
    pub foreground_package: String,
    /// 失败原因，便于调用方区分「没开服务」和「截屏本身失败」：
    /// `accessibility_service_disabled` / `capture_failed` / 空串（成功）。
    #[serde(default)]
    pub reason: String,
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
