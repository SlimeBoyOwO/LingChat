//! GPU 性能检测模块
//!
//! CPU 性能检测 (cpu_perf) 的姊妹功能：读取本机 GPU 并划分性能等级。
//! - Windows：DXGI（跳过软件渲染器并去重同一物理 GPU）
//! - Linux / macOS(x86_64)：WebGPU (wgpu)——Linux 走 Vulkan，macOS 走 Metal
//! - Android / ARM macOS：分级不适用，仅使用 CPU 分级（返回友好提示）
//!
//! 前端负责将检测结果缓存到 localStorage，后续启动直接读取缓存，
//! 不再重复调用后端。后端仅维持会话级内存缓存。

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use super::cpu_perf::PerfTier;

// ────────────────────────────────────────
// 公共类型
// ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// 最高性能 GPU 的名称，例如 "NVIDIA GeForce RTX 4060 Laptop GPU"
    pub name: String,
    /// 该 GPU 的性能等级
    pub tier: PerfTier,
    /// 当前平台是否支持 GPU 分级（Android / ARM macOS 不支持）
    pub is_applicable: bool,
    /// 不适用 / 未检测到可用 GPU 时的友好提示（仅在 message 有值时显示）
    pub message: Option<String>,
}

/// 缓存到状态中的 GPU 检测结果
pub struct GpuDetectionCache {
    pub info: Mutex<Option<GpuInfo>>,
}

impl GpuDetectionCache {
    pub fn new() -> Self {
        Self {
            info: Mutex::new(None),
        }
    }
}

// ────────────────────────────────────────
// 设备枚举
// ────────────────────────────────────────

/// 枚举系统 GPU（Windows：DXGI）
#[cfg(target_os = "windows")]
fn list_devices() -> Vec<(u32, u32, String)> {
    use std::collections::HashSet;
    use windows::Win32::Graphics::Dxgi::*;

    let mut devices = Vec::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    unsafe {
        if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
            for i in 0u32.. {
                if let Ok(adapter) = factory.EnumAdapters1(i) {
                    if let Ok(desc) = adapter.GetDesc1() {
                        let name = String::from_utf16_lossy(&desc.Description)
                            .trim_end_matches('\0')
                            .to_string();
                        // 跳过软件渲染器（Microsoft Basic Render Driver, 0x1414）
                        if desc.VendorId != 0x1414 && seen.insert((desc.VendorId, desc.DeviceId)) {
                            devices.push((desc.VendorId, desc.DeviceId, name));
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }
    devices
}

/// 枚举系统 GPU（Linux / macOS x86_64：WebGPU / wgpu）。
/// Linux 走 Vulkan，macOS 走 Metal；无可用后端时返回空列表。
#[cfg(any(target_os = "linux", all(target_os = "macos", target_arch = "x86_64")))]
fn list_devices() -> Vec<(u32, u32, String)> {
    use std::collections::HashSet;

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let mut devices = Vec::new();
    // 去重同一物理 GPU：多后端（如 Linux 的 Vulkan + GLES）会对同一 GPU 重复枚举，
    // 且 GLES 路径的 vendor/device 可能为 0 —— 有真实 ID 按 (vendor, device) 去重，
    // 无真实 ID 时按归一化名字兜底，避免同卡重复。
    let mut seen_id: HashSet<(u32, u32)> = HashSet::new();
    let mut seen_name: HashSet<String> = HashSet::new();
    for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
        let info = adapter.get_info();
        let name_key = normalize(&info.name);
        let is_dup = if info.vendor != 0 && info.device != 0 {
            seen_id.contains(&(info.vendor, info.device))
        } else {
            seen_name.contains(&name_key)
        };
        if is_dup {
            continue;
        }
        devices.push((info.vendor, info.device, info.name.clone()));
        seen_name.insert(name_key);
        if info.vendor != 0 && info.device != 0 {
            seen_id.insert((info.vendor, info.device));
        }
    }
    devices
}

/// 其余平台（Android / iOS / ARM macOS 等）：不适用，返回空列表。
/// （detect_gpu 会在调用前对这些平台提前返回，此处仅为编译兜底）
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    all(target_os = "macos", target_arch = "x86_64"),
)))]
fn list_devices() -> Vec<(u32, u32, String)> {
    Vec::new()
}

// ────────────────────────────────────────
// 名称解析辅助
// ────────────────────────────────────────

/// 提取 token 之后第一个数字，如 "gtx 1650" -> Some(1650)
fn model_number(name_lower: &str, token: &str) -> Option<u32> {
    let idx = name_lower.find(token)?;
    let rest = &name_lower[idx + token.len()..];
    let mut digits = String::new();
    for c in rest.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if !digits.is_empty() {
            break;
        }
    }
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// 归一化显卡名：去掉 "(r)" "(tm)" 等商标标记和空白，如
/// "Intel(R) Iris(R) Xe Graphics" -> "intelirisxegraphics"
fn normalize(name: &str) -> String {
    let mut n = name.to_ascii_lowercase();
    for tag in ["(r)", "(tm)", "(c)"] {
        n = n.replace(tag, "");
    }
    n.chars().filter(|c| !c.is_whitespace()).collect()
}

// ────────────────────────────────────────
// NVIDIA 分级
// ────────────────────────────────────────

/// GTX 型号不能跨代直接比较数字。这里以 GTX 750 Ti 为基准，按各代桌面卡
/// 的实际性能设置下限，并单独处理较弱的 OEM / M / MX 移动型号。
fn grade_nvidia_gtx(name: &str) -> PerfTier {
    let Some(gtx_pos) = name.find("gtx") else {
        return PerfTier::Low;
    };
    let rest = &name[gtx_pos + "gtx".len()..];
    let digit_count = rest.chars().take_while(char::is_ascii_digit).count();
    let Ok(model) = rest[..digit_count].parse::<u32>() else {
        return PerfTier::Low;
    };
    let suffix = &rest[digit_count..];

    // 同编号移动版和桌面版性能可能相差很大，不能共用桌面阈值。
    let mobile = suffix.starts_with('m')
        || suffix.contains("mobile")
        || suffix.contains("max-q")
        || suffix.contains("maxq");
    if mobile {
        let high = matches!(model, 680 if suffix.starts_with("mx"))
            || matches!(model, 780 | 870 | 880 | 970 | 980)
            || model >= 1050;
        return if high { PerfTier::High } else { PerfTier::Low };
    }

    // OEM/AIO 特供型号也不默认继承同编号桌面卡等级。
    if suffix.starts_with('a') || (model == 660 && suffix.contains("oem")) {
        return PerfTier::Low;
    }

    let high = match model {
        400..=499 => model >= 480,
        500..=599 => model >= 570,
        600..=699 => model >= 660,
        700..=799 => model > 750 || (model == 750 && suffix.starts_with("ti")),
        900..=999 => model >= 950,
        1000.. => true,
        _ => false,
    };
    if high {
        PerfTier::High
    } else {
        PerfTier::Low
    }
}

/// NVIDIA 专业卡 device_id → 等级 硬编码表。
/// 规则：同一型号的 PCI ID 变体 ≤ 2 个 → 进表精确映射；
/// PCI device_id 改不了，表映射优先于名字规则。
fn grade_nvidia_by_device_id(device_id: u32) -> Option<PerfTier> {
    match device_id {
        // --- Quadro（单 ID 全进表） ---
        0x1430 => Some(PerfTier::High), // M2000
        0x13F1 => Some(PerfTier::High), // M4000
        0x13F0 => Some(PerfTier::High), // M5000
        0x1C30 => Some(PerfTier::High), // P2000
        0x1C31 => Some(PerfTier::High), // P2200
        0x1BB1 => Some(PerfTier::High), // P4000
        0x1BB0 => Some(PerfTier::High), // P5000
        0x1B30 => Some(PerfTier::High), // P6000
        0x1DBA => Some(PerfTier::High), // GV100

        _ => None,
    }
}

/// AMD 专业卡 device_id → 等级 硬编码表（表优先，防魔改驱动骗名字）。
/// 与 NVIDIA 表同一策略：同一型号的 PCI ID 变体 ≤ 2 个 → 进表精确映射；
/// PCI device_id 改不了，表映射优先于名字规则。
/// 只收录名字规则覆盖不精确 / 易误判的型号：
/// - 名字不含型号（驱动只显示 "FirePro Series Graphics Adapter" 等）→ 靠 ID 定级
/// - 远古弱卡名字前缀区分不出代际的 M / RG 系列 → LOW（名字规则会误判为现代卡）
/// 数据来源：PCI ID Repository (pci-ids.ucw.cz, vendor 1002)。
fn grade_amd_by_device_id(device_id: u32) -> Option<PerfTier> {
    match device_id {
        // --- 名字不含型号，只能靠 ID 定级 ---
        0x6784 | 0x6788 | 0x678A => Some(PerfTier::High), // Tahiti "FirePro Series" ≈ HD 7970
        0x68E9 => Some(PerfTier::Low), // Cedar "ATI FirePro (FireGL) Graphics Adapter"（2010 低端，名字规则会误判 HIGH）

        // --- 远古移动 / 嵌入式弱卡：名字前缀是 M / RG，名字规则会误判为现代卡 ---
        0x946A => Some(PerfTier::Low),          // FirePro M7750 (RV770)
        0x94A3 => Some(PerfTier::Low),          // FirePro M7740 (RV740)
        0x9555 | 0x9557 => Some(PerfTier::Low), // FirePro RG220 (RV711)
        _ => None,
    }
}

/// 字符串家族兜底（对应 NVIDIA 的矿卡 / 计算卡字符串家族）：
/// ID 变体多的家族逐个进表维护成本高，按品牌字符串整体判定等级。
/// Radeon Pro / Instinct 全系独显变体几十个且整体高性能，统一 HIGH。
/// 注意：Ryzen Pro APU 的核显也会伪装成 "Radeon Pro Graphics"，名字带
/// "graphics" 的是核显，不在此列，交核显规则（Vega → MEDIUM）处理。
/// 命中返回对应等级；未命中返回 None。传入的是 normalize 后的名字。
fn grade_amd_by_string_family(n: &str) -> Option<PerfTier> {
    if n.contains("instinct") || (n.contains("radeonpro") && !n.contains("graphics")) {
        return Some(PerfTier::High);
    }
    None
}

/// 按名字识别 AMD 专业卡（FirePro / FireGL / FireStream / FireMV）。
/// Radeon Pro / Instinct 已由字符串家族层 grade_amd_by_string_family 先行判定，
/// 这里只细分名字能区分代际的 FirePro 系列与远古 FireGL / FireMV / FireStream。
/// 命中专业关键字返回对应等级；未命中返回 None，交由 RX / R9 / HD 等消费卡规则处理。
/// 传入的是 normalize 后的名字（小写、去空白、去商标标记）。
fn grade_amd_by_name(n: &str) -> Option<PerfTier> {
    // 远古 TeraScale / 多屏卡
    if n.contains("firegl") || n.contains("firemv") {
        return Some(PerfTier::Low);
    }
    // FirePro 系列：按子系列与代际细分
    if let Some(pos) = n.find("firepro") {
        let rest = &n[pos + "firepro".len()..];
        // FirePro V 系列（TeraScale 2/3）：编号 >= 6000 才算能打
        if let Some(v) = rest.strip_prefix('v') {
            let digits: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = digits.parse::<u32>() {
                return Some(if num >= 6000 { PerfTier::High } else { PerfTier::Low });
            }
            return Some(PerfTier::Low);
        }
        // FirePro W / S / A 系列（GCN，含低端 W 不降级）
        if rest.starts_with('w') || rest.starts_with('s') || rest.starts_with('a') {
            return Some(PerfTier::High);
        }
        // FirePro 2xxx 老低端
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Some(PerfTier::Low);
        }
        // M / RG 等其他前缀：现代移动 / 定制专业卡按现代处理；
        // 远古 M7750 / M7740 / RG220 由 device_id 表兜底为 LOW
        return Some(PerfTier::High);
    }
    // FireStream 计算卡：编号 >= 9300（9370 / 9350）才算能打，9270 / 9250 / 9170 较弱
    if let Some(pos) = n.find("firestream") {
        let rest = &n[pos + "firestream".len()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(num) = digits.parse::<u32>() {
            return Some(if num >= 9300 { PerfTier::High } else { PerfTier::Low });
        }
        return Some(PerfTier::High);
    }
    None
}

// ────────────────────────────────────────
// 性能分级
// ────────────────────────────────────────

/// 性能分级：
/// - NVIDIA 独显：先查 device_id 专业卡表（防魔改），再按字符串家族（矿卡/计算卡），
///   最后 RTX/TITAN/GTX 名字规则
/// - AMD 独显：先查 device_id 专业卡表（防魔改 + 兜底名字不可靠），再按字符串家族
///   （Radeon Pro / Instinct 全系 HIGH），最后名字规则（FirePro V/W/S/A 细分、
///   FireGL / FireMV、FireStream 编号）；RX 全系 HIGH；R9 / R7 >= 260 HIGH；
///   HD 7800/7900 HIGH；其余老弱病残 LOW
/// - Intel 核显：Iris Xe / Iris Plus / UHD -> MEDIUM；HD 老核显 -> LOW；Arc 独显 -> HIGH
/// - AMD 核显：Vega / 新 Radeon Graphics -> MEDIUM；HD 系列老 APU -> LOW
/// - 未知 / 未读取到 -> LOW
fn grade(vendor_id: u32, device_id: u32, name: &str) -> PerfTier {
    let n = normalize(name);
    match vendor_id {
        0x10DE => {
            // NVIDIA：硬件 ID 表优先（防魔改驱动骗名字）
            if let Some(t) = grade_nvidia_by_device_id(device_id) {
                return t;
            }
            // ID 变体多的家族走字符串兜底（表维护成本高）：
            // 矿卡 P106 / P104 / P102 / CMP，计算卡 V100 / P100 / 其余 Tesla
            if n.contains("p106")
                || n.contains("p104")
                || n.contains("p102")
                || n.contains("cmp")
                || n.contains("v100")
                || n.contains("p100")
                || n.contains("tesla")
            {
                return PerfTier::High;
            }
            if n.contains("rtx") || n.contains("titan") {
                return PerfTier::High;
            }
            if n.contains("gtx") {
                return grade_nvidia_gtx(&n);
            }
            PerfTier::Low // GeForce GT / 老系列 / 未识别
        }
        0x1002 => {
            // AMD / ATI：硬件 ID 表优先（防魔改驱动骗名字）
            if let Some(t) = grade_amd_by_device_id(device_id) {
                return t;
            }
            // ID 变体多的家族走字符串兜底（表维护成本高）：
            // Radeon Pro / Instinct 全系（变体几十个，整体 HIGH）
            if let Some(t) = grade_amd_by_string_family(&n) {
                return t;
            }
            // 名字规则细分 FirePro / FireGL / FireStream 等专业卡
            if let Some(t) = grade_amd_by_name(&n) {
                return t;
            }
            if n.contains("rx") {
                return PerfTier::High; // RX 全系 >= 750Ti
            }
            if n.contains("r9") || n.contains("r7") {
                let token = if n.contains("r9") { "r9" } else { "r7" };
                if let Some(num) = model_number(&n, token) {
                    if num >= 260 {
                        return PerfTier::High;
                    }
                }
                return PerfTier::Low;
            }
            if n.contains("hd") {
                // HD 7800/7900 独显还算能打，其余（含 HD 8xxxD 老 APU）LOW
                if let Some(num) = model_number(&n, "hd") {
                    if (7800..8000).contains(&num) {
                        return PerfTier::High;
                    }
                }
                return PerfTier::Low;
            }
            // 新 APU 核显（Vega / RDNA）
            if n.contains("vega") || n.contains("radeongraphics") {
                return PerfTier::Medium;
            }
            PerfTier::Low
        }
        0x8086 => {
            // Intel
            if n.contains("arc") {
                return PerfTier::High; // Arc 独显
            }
            if n.contains("irisxe") || n.contains("irisplus") || n.contains("uhd") {
                return PerfTier::Medium;
            }
            PerfTier::Low // HD Graphics / GMA 老核显
        }
        _ => PerfTier::Low,
    }
}

/// PerfTier 排序权重（Internet < Low < Medium < High），用于取最高 / 最低
fn tier_rank(tier: PerfTier) -> u8 {
    match tier {
        PerfTier::Internet => 0,
        PerfTier::Low => 1,
        PerfTier::Medium => 2,
        PerfTier::High => 3,
    }
}

// ────────────────────────────────────────
// 检测入口
// ────────────────────────────────────────

/// 执行 GPU 检测，返回最高性能 GPU 的名称与等级。
///
/// Android / ARM macOS：分级不适用，仅使用 CPU 分级，返回友好提示。
pub fn detect_gpu() -> GpuInfo {
    // Android / ARM macOS：GPU 分级不适用（仅使用 CPU 分级）
    if cfg!(target_os = "android") || (cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")) {
        return GpuInfo {
            name: String::new(),
            tier: PerfTier::Low,
            is_applicable: false,
            message: Some("当前设备不支持 GPU 性能分级，仅使用 CPU 分级".to_string()),
        };
    }

    let devices = list_devices();
    if devices.is_empty() {
        return GpuInfo {
            name: String::new(),
            tier: PerfTier::Low,
            is_applicable: true,
            message: Some("未检测到可用 GPU（可能只有软件渲染器）".to_string()),
        };
    }

    // 取性能等级最高的 GPU（并列时保留第一个）
    let mut best: Option<(PerfTier, String)> = None;
    for (vid, did, name) in devices {
        let t = grade(vid, did, &name);
        let better = match &best {
            None => true,
            Some((bt, _)) => tier_rank(t) > tier_rank(*bt),
        };
        if better {
            best = Some((t, name));
        }
    }

    match best {
        Some((tier, name)) => GpuInfo {
            name,
            tier,
            is_applicable: true,
            message: None,
        },
        None => GpuInfo {
            name: String::new(),
            tier: PerfTier::Low,
            is_applicable: true,
            message: Some("未检测到可用 GPU（可能只有软件渲染器）".to_string()),
        },
    }
}

// ────────────────────────────────────────
// Tauri 命令
// ────────────────────────────────────────

use tauri::State;

/// Tauri 命令：获取 GPU 信息（仅维持会话级内存缓存）
///
/// 注意：持久化缓存由前端在 localStorage 中管理，后端不读写磁盘文件。
#[tauri::command]
pub fn get_gpu_info(state: State<'_, GpuDetectionCache>) -> Result<GpuInfo, String> {
    let mut guard = state.info.lock().map_err(|e| e.to_string())?;
    if let Some(ref info) = *guard {
        return Ok(info.clone());
    }

    // 会话内首次调用：执行检测
    let info = detect_gpu();
    *guard = Some(info.clone());
    Ok(info)
}

/// Tauri 命令：重新检测 GPU 性能（清除内存缓存后重测）
#[tauri::command]
pub fn redetect_gpu(state: State<'_, GpuDetectionCache>) -> Result<GpuInfo, String> {
    let info = detect_gpu();

    let mut guard = state.info.lock().map_err(|e| e.to_string())?;
    *guard = Some(info.clone());
    Ok(info)
}

// ────────────────────────────────────────
// 当前实际调用的 GPU（WebGL 渲染器字符串）
// ────────────────────────────────────────

/// 从 WebGL `WEBGL_debug_renderer_info.UNMASKED_RENDERER_WEBGL` 渲染器字符串中，
/// 解析出 GPU 厂商 `vendor_id` 与型号名 `name`。
///
/// 该字符串反映的是 Chromium/WebView2 实际合成（含 WebGL 渲染）所用的 GPU，
/// 因此比「枚举所有硬件取最高」更能代表程序当前真正调用的卡。
///
/// 示例：
/// - `ANGLE (NVIDIA, NVIDIA GeForce RTX 4060 Laptop GPU Direct3D11 vs_5_0 ps_5_0, D3D11)`
/// - `ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)`
/// - `ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) ...))`
///
/// 返回 `(vendor_id, gpu_name)`；识别不出厂商或为软件渲染时 `vendor_id = 0`。
fn parse_renderer(renderer: &str) -> (u32, String) {
    let lower = renderer.to_ascii_lowercase();

    // 厂商 → vendor_id（按关键词判定；AMD 兼容 Radeon/ATI 老命名）
    let vendor_id = if lower.contains("nvidia") {
        0x10DE
    } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("ati") {
        0x1002
    } else if lower.contains("intel") {
        0x8086
    } else {
        0
    };

    // 去掉 "ANGLE (" 前缀与尾部 ")"，取厂商认领之后的 GPU 名称段
    let body = renderer
        .trim()
        .trim_start_matches("ANGLE")
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let name_part = if let Some(comma) = body.find(',') {
        body[comma + 1..].trim()
    } else {
        body
    };

    // 在第一个后端描述关键字处截断（Direct3D / Vulkan / OpenGL / Metal / d3d 等）
    let cut = ["direct3d", "vulkan", "opengl", "metal", "d3d11", "d3d12"]
        .iter()
        .filter_map(|kw| name_part.to_ascii_lowercase().find(kw).map(|i| i))
        .min()
        .unwrap_or(name_part.len());
    let name = name_part[..cut].trim().to_string();

    // SwiftShader 等软件渲染 → 视为不可用（vendor=0）
    let name_lower = name.to_ascii_lowercase();
    if name_lower.is_empty() || name_lower.contains("swiftshader") || name_lower.contains("software")
    {
        (0, name)
    } else {
        (vendor_id, name)
    }
}

/// Tauri 命令：对前端上报的 WebGL 渲染器字符串定级，返回「当前实际调用的 GPU」。
///
/// 前端通过 `WEBGL_debug_renderer_info.UNMASKED_RENDERER_WEBGL` 读到渲染器字符串后传给这里，
/// 复用与硬件枚举一致的 `grade()` 逻辑定级。仅展示用，不参与 `detect_gpu()` 的「取最高」。
#[tauri::command]
pub fn grade_active_gpu(renderer: String) -> Result<GpuInfo, String> {
    let (vendor_id, name) = parse_renderer(&renderer);
    let name = name.trim().to_string();
    if vendor_id == 0 || name.is_empty() {
        return Ok(GpuInfo {
            name: String::new(),
            tier: PerfTier::Low,
            is_applicable: true,
            message: Some("无法识别当前 WebGL 渲染 GPU".to_string()),
        });
    }
    let tier = grade(vendor_id, 0, &name);
    Ok(GpuInfo {
        name,
        tier,
        is_applicable: true,
        message: None,
    })
}
