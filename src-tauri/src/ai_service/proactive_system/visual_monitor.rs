#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

pub struct VisualMonitor {
    last_hash: Option<u64>,
    pending_change: bool,
}

impl VisualMonitor {
    pub fn new() -> Self {
        Self {
            last_hash: None,
            pending_change: false,
        }
    }

    /// 执行快速的 GDI 像素网格采样并检测画面是否变化。
    /// 成功检测到变化时返回 true，否则返回 false。
    pub fn check_visual_change(&mut self) -> bool {
        #[cfg(target_os = "windows")]
        {
            unsafe {
                let screen_width = GetSystemMetrics(SM_CXSCREEN);
                let screen_height = GetSystemMetrics(SM_CYSCREEN);
                if screen_width <= 0 || screen_height <= 0 {
                    return false;
                }

                let hdc = GetDC(None);
                if hdc.is_invalid() {
                    return false;
                }

                // 采样 9x8 像素网格
                let mut grays = [0.0; 72];
                for j in 0..8 {
                    for i in 0..9 {
                        // 在屏幕上均匀取点，避开极边缘
                        let x = (i as f32 + 0.5) * (screen_width as f32 / 9.0);
                        let y = (j as f32 + 0.5) * (screen_height as f32 / 8.0);

                        let color = GetPixel(hdc, x as i32, y as i32).0;

                        // Extract RGB from COLORREF (0x00BBGGRR)
                        let r = (color & 0x000000FF) as f64;
                        let g = ((color & 0x0000FF00) >> 8) as f64;
                        let b = ((color & 0x00FF0000) >> 16) as f64;

                        // Grayscale
                        grays[j * 9 + i] = 0.299 * r + 0.587 * g + 0.114 * b;
                    }
                }

                ReleaseDC(None, hdc);

                // 计算 64-bit dhash
                let mut hash: u64 = 0;
                let mut bit_idx = 0;
                for j in 0..8 {
                    for i in 0..8 {
                        let gray1 = grays[j * 9 + i];
                        let gray2 = grays[j * 9 + i + 1];
                        if gray1 > gray2 {
                            hash |= 1 << bit_idx;
                        }
                        bit_idx += 1;
                    }
                }

                if let Some(prev) = self.last_hash {
                    let diff = (prev ^ hash).count_ones();
                    self.last_hash = Some(hash);

                    // Hamming distance >= 6 means change detected (threshold matching Python dhash difference)
                    let change_detected = diff >= 6;
                    if change_detected {
                        self.pending_change = true;
                        tracing::info!(
                            "[VisualMonitor] Screen change detected! Hamming distance: {}",
                            diff
                        );
                    }
                    self.pending_change
                } else {
                    self.last_hash = Some(hash);
                    // 首次采样也需要分析一次，否则用户在启动后画面不变化时永远不会进入视觉路径。
                    self.pending_change = true;
                    tracing::debug!("[VisualMonitor] Initial screen sample marked for analysis");
                    true
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    /// 视觉模型已经消费了当前变化，清除待分析标记。
    pub fn mark_analyzed(&mut self) {
        self.pending_change = false;
    }
}
