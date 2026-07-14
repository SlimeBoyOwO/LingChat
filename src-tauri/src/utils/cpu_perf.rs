//! CPU 性能检测模块
//!
//! 初次启动时检测 CPU 型号并划分性能等级。
//! 前端负责将检测结果缓存到 localStorage，后续启动直接读取缓存，
//! 不再重复调用后端。后端仅维持会话级内存缓存。

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

// ────────────────────────────────────────
// 公共类型
// ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerfTier {
    Internet,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    /// CPU 品牌字符串，例如 "Intel(R) Core(TM) i7-8550U CPU @ 1.80GHz"
    pub brand: String,
    /// 性能等级
    pub tier: PerfTier,
    /// 是否为 ARM 等非 x86 无法识别的 CPU
    pub is_unknown: bool,
    /// 未知 CPU 时的友好提示（仅在 is_unknown 为 true 时有值）
    pub unknown_message: Option<String>,
}

/// 缓存到状态中的 CPU 检测结果
pub struct CpuDetectionCache {
    pub info: Mutex<Option<CpuInfo>>,
}

impl CpuDetectionCache {
    pub fn new() -> Self {
        Self {
            info: Mutex::new(None),
        }
    }
}

// ────────────────────────────────────────
// x86 / x86_64 CPUID 实现
// ────────────────────────────────────────

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86_impl {
    use super::*;

    /// 执行 CPUID 指令
    #[inline]
    fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
        let eax: u32;
        let ebx: u32;
        let ecx: u32;
        let edx: u32;
        unsafe {
            core::arch::asm!(
                "mov {tmp}, rbx",
                "cpuid",
                "mov {ebx:e}, ebx",
                "mov rbx, {tmp}",
                tmp = out(reg) _,
                ebx = out(reg) ebx,
                inout("eax") leaf => eax,
                inout("ecx") subleaf => ecx,
                out("edx") edx,
                options(nostack, preserves_flags)
            );
        }
        (eax, ebx, ecx, edx)
    }

    /// 获取 CPU 品牌字符串
    fn get_brand_string() -> Option<String> {
        let (max_ext, _, _, _) = cpuid(0x80000000, 0);
        if max_ext < 0x80000004 {
            return None;
        }

        let mut buf = [0u8; 48];
        for i in 0usize..3 {
            let leaf = 0x80000002 + i as u32;
            let (eax, ebx, ecx, edx) = cpuid(leaf, 0);
            let offset = i * 16;
            buf[offset..offset + 4].copy_from_slice(&eax.to_le_bytes());
            buf[offset + 4..offset + 8].copy_from_slice(&ebx.to_le_bytes());
            buf[offset + 8..offset + 12].copy_from_slice(&ecx.to_le_bytes());
            buf[offset + 12..offset + 16].copy_from_slice(&edx.to_le_bytes());
        }

        // 去除尾部空白和空字符（CPUID 字符串以空字符填充）
        let s = String::from_utf8_lossy(&buf)
            .trim_end_matches(|c: char| c.is_ascii_whitespace() || c == '\0')
            .to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    /// 检查是否为 Intel CPU
    fn is_intel() -> bool {
        let (_, ebx, ecx, edx) = cpuid(0, 0);
        ebx == 0x756e6547 && edx == 0x49656e69 && ecx == 0x6c65746e
    }

    /// 检查是否为 AMD CPU
    fn is_amd() -> bool {
        let (_, ebx, ecx, edx) = cpuid(0, 0);
        ebx == 0x68747541 && edx == 0x69746e65 && ecx == 0x444d4163
    }

    /// 从品牌字符串提取 Core 代数
    fn extract_core_generation(brand: &str) -> Option<i32> {
        let p = brand.find("Core")?;
        let after_core = &brand[p..];

        let patterns = ["i3-", "i5-", "i7-", "i9-", "m3-", "m5-", "m7-"];
        let marker = patterns.iter().find_map(|pat| {
            let pos = after_core.find(pat)?;
            Some(pos + 3) // 跳过 "iX-" 或 "mX-"
        })?;

        let num_str = &after_core[marker..];
        let model_num: i64 = num_str
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .ok()?;

        if model_num >= 1000 {
            Some((model_num / 1000) as i32)
        } else {
            None // 初代 Core
        }
    }

    /// 判断是否为低电压后缀（U / Y）
    fn is_low_power_suffix(brand: &str) -> bool {
        let cpu_at = match brand.find("CPU @") {
            Some(pos) => pos,
            None => return false,
        };

        let before = &brand[..cpu_at].trim();
        let last_part = before.split(' ').last().unwrap_or("");
        last_part.contains('U') || last_part.contains('Y')
    }

    /// 核心分级逻辑 —— 与原始 C 版保持语义一致
    fn classify_brand(brand: &str) -> PerfTier {
        if brand.contains("Atom") {
            return PerfTier::Internet;
        }

        if brand.contains("Celeron") || brand.contains("Pentium") {
            return if is_low_power_suffix(brand) {
                PerfTier::Internet
            } else {
                PerfTier::Low
            };
        }

        if brand.contains("Core") {
            let gen = extract_core_generation(brand);
            let low_power = is_low_power_suffix(brand);

            if gen >= Some(8) {
                return PerfTier::High;
            }

            // 特判 12/13 代
            if brand.contains("12th Gen") || brand.contains("13th Gen") {
                return PerfTier::High;
            }

            // Core Ultra
            if brand.contains("Ultra") {
                return PerfTier::High;
            }

            if let Some(gen) = gen {
                if gen < 8 {
                    let is_i3 = brand.contains("i3-");

                    if is_i3 && gen < 5 {
                        return PerfTier::Low;
                    }
                    if low_power && gen <= 7 {
                        return PerfTier::Low;
                    }
                    // 6代及以上 i7 → High
                    if gen >= 6 && brand.contains("i7-") {
                        return PerfTier::High;
                    }
                    return PerfTier::Medium;
                }
            }

            // Core 2 系列
            if brand.contains("Duo") || brand.contains("Quad") || brand.contains("Extreme") {
                return PerfTier::Low;
            }

            if low_power {
                return PerfTier::Low;
            }
        }

        // Xeon
        if brand.contains("Xeon") {
            if brand.contains("E5") || brand.contains("E7") {
                return PerfTier::High;
            }
            return PerfTier::Medium;
        }

        PerfTier::Low
    }

    pub fn detect_cpu() -> CpuInfo {
        let brand = get_brand_string().unwrap_or_default();

        if !is_intel() {
            if is_amd() {
                return CpuInfo {
                    brand,
                    tier: PerfTier::Low,
                    is_unknown: false,
                    unknown_message: None,
                };
            }
            // 非 Intel/AMD（如兆芯、海光等）—— 无法准确识别
            return CpuInfo {
                brand,
                tier: PerfTier::Low,
                is_unknown: true,
                unknown_message: Some("还有我不认识的设备，哈！".to_string()),
            };
        }

        let tier = classify_brand(&brand);
        CpuInfo {
            brand,
            tier,
            is_unknown: false,
            unknown_message: None,
        }
    }
}

// ────────────────────────────────────────
// 非 x86 平台（ARM 等）—— 直接返回 Low
// ────────────────────────────────────────

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
mod imp {
    use super::*;

    pub fn detect_cpu() -> CpuInfo {
        // ARM 或不支持 CPUID 的平台
        let arch = std::env::consts::ARCH.to_string();
        CpuInfo {
            brand: format!("{arch} 架构处理器"),
            tier: PerfTier::Low,
            is_unknown: true,
            unknown_message: Some("还有我不认识的设备，哈！".to_string()),
        }
    }
}

// ────────────────────────────────────────
// 公开 API（统一入口）
// ────────────────────────────────────────

/// 执行 CPU 检测（仅在 x86/x86_64 上真正执行 CPUID）
pub fn detect_cpu() -> CpuInfo {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        x86_impl::detect_cpu()
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        imp::detect_cpu()
    }
}

// ────────────────────────────────────────
// Tauri 命令
// ────────────────────────────────────────

use tauri::State;

/// Tauri 命令：获取 CPU 信息（仅维持会话级内存缓存）
///
/// 注意：持久化缓存由前端在 localStorage 中管理，
/// 后端不再读写磁盘文件。
#[tauri::command]
pub fn get_cpu_info(state: State<'_, CpuDetectionCache>) -> Result<CpuInfo, String> {
    let mut guard = state.info.lock().map_err(|e| e.to_string())?;
    if let Some(ref info) = *guard {
        return Ok(info.clone());
    }

    // 会话内首次调用：执行检测
    let info = detect_cpu();
    *guard = Some(info.clone());
    Ok(info)
}

/// Tauri 命令：重新检测 CPU 性能（清除内存缓存后重测）
#[tauri::command]
pub fn redetect_cpu(state: State<'_, CpuDetectionCache>) -> Result<CpuInfo, String> {
    let info = detect_cpu();

    let mut guard = state.info.lock().map_err(|e| e.to_string())?;
    *guard = Some(info.clone());
    Ok(info)
}
