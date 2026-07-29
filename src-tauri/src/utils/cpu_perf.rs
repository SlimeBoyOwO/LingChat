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

    /// 执行 CPUID 指令 (x86_64，使用 rbx)
    #[cfg(target_arch = "x86_64")]
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

    /// 执行 CPUID 指令 (x86，直接使用 out("ebx") 让编译器处理保存/恢复)
    #[cfg(target_arch = "x86")]
    #[inline]
    fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
        let eax: u32;
        let ebx: u32;
        let ecx: u32;
        let edx: u32;
        unsafe {
            core::arch::asm!(
                "cpuid",
                inout("eax") leaf => eax,
                out("ebx") ebx,
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
    
    /// AMD:提取 Ryzen 等级（3/5/7/9）
    fn extract_ryzen_level(brand: &str) -> Option<u32> {
        if brand.contains("Ryzen 9") {
            Some(9)
        } else if brand.contains("Ryzen 7") {
            Some(7)
        } else if brand.contains("Ryzen 5") {
            Some(5)
        } else if brand.contains("Ryzen 3") {
            Some(3)
        } else {
            None
        }
    }

    /// 找到品牌字符串中的 Ryzen 型号单词（如 "7840U", "4650G", "8845HS"）
    fn find_ryzen_model_word(brand: &str) -> Option<&str> {
        for word in brand.split_whitespace() {
            let digit_count = word
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .count();

            if digit_count == 4 {
                return Some(word);
            }
        }
        None
    }

    /// 提取 Ryzen 型号（如 7840U -> 7840，4650G -> 4650）
    fn extract_ryzen_model(brand: &str) -> Option<u32> {
        let word = find_ryzen_model_word(brand)?;
        let digits: String = word.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<u32>().ok()
    }

    /// 提取后缀（从型号单词中提取字母后缀，如 "7840U" → "U"、"8845HS" → "HS"）
    fn extract_ryzen_suffix(brand: &str) -> &'static str {
        // 从型号单词中提取字母后缀（主要路径）
        if let Some(word) = find_ryzen_model_word(brand) {
            let suffix: String = word
                .chars()
                .skip_while(|c| c.is_ascii_digit())
                .collect();

            return match suffix.as_str() {
                "HX" => "HX",
                "HS" => "HS",
                "GE" => "GE",
                "PRO" => "PRO",
                "U" => "U",
                "H" => "H",
                "X" => "X",
                "G" => "G",
                _ => "",
            };
        }

        // 回退：旧逻辑（处理型号单词不在品牌字符串中的边缘情况）
        if brand.contains("HX") {
            "HX"
        } else if brand.contains("HS") {
            "HS"
        } else if brand.contains("GE") {
            "GE"
        } else if brand.contains("PRO") {
            "PRO"
        } else if brand.contains(" U") || brand.ends_with('U') {
            "U"
        } else if brand.contains(" H") || brand.ends_with('H') {
            "H"
        } else if brand.contains(" X") || brand.ends_with('X') {
            "X"
        } else if brand.contains(" G") || brand.ends_with('G') {
            "G"
        } else {
            ""
        }
    }

    fn classify_amd_brand(brand: &str) -> PerfTier {
        // 服务器
        if brand.contains("EPYC") || brand.contains("Threadripper") {
            return PerfTier::High;
        }

        // Ryzen AI
        if brand.contains("Ryzen AI") {
            return PerfTier::High;
        }

        // 老系列
        if brand.contains("Sempron") {
            return PerfTier::Internet;
        }

        if brand.contains("Athlon II") || brand.contains("Athlon") || brand.contains("Phenom")
        {
            return PerfTier::Low;
        }

        // FX
        if brand.contains("FX-") {
            return PerfTier::Medium;
        }

        // APU
        if brand.contains("A4-") {
            return PerfTier::Internet;
        }

        if brand.contains("A6-") {
            return PerfTier::Low;
        }

        if brand.contains("A8-") || brand.contains("A10-") || brand.contains("A12-")
        {
            return PerfTier::Medium;
        }

        if !brand.contains("Ryzen") {
            return PerfTier::Low;
        }

        let level = extract_ryzen_level(brand).unwrap_or(5);
        let model = extract_ryzen_model(brand).unwrap_or(0);
        let suffix = extract_ryzen_suffix(brand);

        let series = model / 1000;

        match level {
            9 => PerfTier::High,

            7 => {
                if series <= 2 {
                    PerfTier::Medium
                } else {
                    PerfTier::High
                }
            }

            5 => {
                // U 后缀优先处理（含 AMD 官方特殊型号映射，必须在 series >= 6 之前）
                if suffix == "U" {
                    if series <= 2 {
                        return PerfTier::Low;
                    }

                    // AMD 官方 7000 系特殊命名
                    match model {
                        7520 | 7320 => return PerfTier::Medium,
                        7530 | 7730 => return PerfTier::Medium,
                        7535 | 7735 | 7640 | 7840 | 8840 | 8845 => {
                            return PerfTier::High
                        }
                        _ => {}
                    }

                    return PerfTier::Medium;
                }

                if series >= 6 {
                    return PerfTier::High;
                }

                if suffix == "H" || suffix == "HS" || suffix == "HX" || suffix == "X"
                {
                    return PerfTier::High;
                }

                if series <= 2 {
                    PerfTier::Medium
                } else {
                    PerfTier::High
                }
            }

            3 => {
                if series >= 6 {
                    return PerfTier::Medium;
                }

                if suffix == "H" || suffix == "HS" || suffix == "HX" || suffix == "X"
                {
                    return PerfTier::Medium;
                }

                PerfTier::Low
            }

            _ => PerfTier::Medium,
        }
    }
    /// Intel:从品牌字符串提取 Core 代数
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
            let tier = classify_amd_brand(&brand);    
                return CpuInfo {
                    brand,
                    tier,
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
// 非 x86 平台（ARM 等）—— 核心拓扑 + 频率启发式
// ────────────────────────────────────────

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
mod imp {
    use super::*;
    use std::fs;
    use std::path::Path;

    // ────────────────────────────────────────
    // 频率分级阈值常量（经验值，后续可调）
    // ────────────────────────────────────────

    /// High 等级所需的最低最高频率 (MHz)
    const HIGH_FREQ_THRESHOLD_MHZ: u64 = 3000;
    /// High 等级所需的最低大核数量（频率 ≥ BIG_CORE_FREQ_THRESHOLD_MHZ 的核心）
    const HIGH_BIG_CORE_MIN: usize = 2;
    /// 统计 big core 时的频率门槛 (MHz)
    const BIG_CORE_FREQ_THRESHOLD_MHZ: u64 = 2800;
    /// Medium 等级所需的最低最高频率 (MHz)
    const MEDIUM_FREQ_THRESHOLD_MHZ: u64 = 2400;
    /// Medium 等级所需的最低总核心数
    const MEDIUM_TOTAL_CORES_MIN: usize = 8;

    // ────────────────────────────────────────
    // 辅助函数
    // ────────────────────────────────────────

    /// 读取指定 CPU 核心的 `cpuinfo_max_freq`（单位 kHz），失败返回 `None`。
    fn read_core_max_freq(core_index: usize) -> Option<u64> {
        let path = format!(
            "/sys/devices/system/cpu/cpu{}/cpufreq/cpuinfo_max_freq",
            core_index
        );
        let content = fs::read_to_string(&path).ok()?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }
        trimmed.parse::<u64>().ok()
    }

    /// 收集所有逻辑核心的最高频率。
    ///
    /// 遍历 `/sys/devices/system/cpu/cpu[0-9]+/cpufreq/cpuinfo_max_freq`，
    /// 读取失败的核心会被静默跳过。
    /// 返回 `(freqs_mhz, total_cores)`，其中 `freqs_mhz` 为成功读取到的以 MHz 为单位的频率列表。
    fn collect_core_frequencies() -> (Vec<u64>, usize) {
        let cpu_dir = Path::new("/sys/devices/system/cpu");

        // 非 Linux 系统（如 iOS/macOS ARM）上此路径不存在，直接返回空
        if !cpu_dir.is_dir() {
            return (Vec::new(), 0);
        }

        let mut freqs = Vec::new();
        let mut total_cores = 0usize;

        if let Ok(entries) = fs::read_dir(cpu_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if !name_str.starts_with("cpu") {
                    continue;
                }

                // 提取 "cpu" 后面的数字部分
                let num_part = &name_str[3..];
                let index: usize = match num_part.parse() {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                total_cores += 1;

                if let Some(freq_khz) = read_core_max_freq(index) {
                    freqs.push(freq_khz / 1000); // kHz → MHz
                }
            }
        }

        (freqs, total_cores)
    }

    /// 根据核心频率拓扑数据划分性能等级（纯函数，方便单元测试）。
    fn classify_by_topology(freqs_mhz: &[u64], total_cores: usize) -> PerfTier {
        if freqs_mhz.is_empty() || total_cores == 0 {
            return PerfTier::Low;
        }

        let max_freq_mhz = *freqs_mhz.iter().max().unwrap_or(&0);
        let big_core_count = freqs_mhz
            .iter()
            .filter(|&&f| f >= BIG_CORE_FREQ_THRESHOLD_MHZ)
            .count();

        if max_freq_mhz >= HIGH_FREQ_THRESHOLD_MHZ && big_core_count >= HIGH_BIG_CORE_MIN {
            PerfTier::High
        } else if max_freq_mhz >= MEDIUM_FREQ_THRESHOLD_MHZ
            && total_cores >= MEDIUM_TOTAL_CORES_MIN
        {
            PerfTier::Medium
        } else {
            PerfTier::Low
        }
    }

    /// 构建品牌字符串。
    fn build_brand_string(total_cores: usize, max_freq_mhz: Option<u64>) -> String {
        match max_freq_mhz {
            Some(freq) => format!("ARM {}核 (最高{}MHz)", total_cores, freq),
            None => format!("ARM {}核 (频率未知)", total_cores),
        }
    }

    pub fn detect_cpu() -> CpuInfo {
        let (freqs_mhz, total_cores) = collect_core_frequencies();

        let is_unknown = freqs_mhz.is_empty() || total_cores == 0;
        let tier = if is_unknown {
            PerfTier::Low
        } else {
            classify_by_topology(&freqs_mhz, total_cores)
        };

        let max_freq_mhz = freqs_mhz.iter().max().copied();
        let brand = build_brand_string(total_cores, max_freq_mhz);

        let unknown_message = if is_unknown {
            Some("还有我不认识的设备，哈！".to_string())
        } else {
            None
        };

        CpuInfo {brand,tier,is_unknown,unknown_message,}
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
