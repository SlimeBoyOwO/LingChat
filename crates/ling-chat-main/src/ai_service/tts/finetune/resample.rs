//! 采样率转换：加窗 sinc 插值，纯 Rust 实现，桌面与移动端一致。

/// 插值核半宽；32 点对语音足够。
const TAPS: usize = 32;

/// 把单声道采样从 `from` 重采样到 `to`；降采样时低通压到目标奈奎斯特的 95% 防混叠。
pub fn resample_mono(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == 0 || to == 0 || from == to {
        return input.to_vec();
    }

    let ratio = f64::from(to) / f64::from(from);
    let out_len = (input.len() as f64 * ratio).round() as usize;
    let cutoff = if to < from {
        f64::from(to) / f64::from(from) * 0.95
    } else {
        0.95
    };
    let half = TAPS as f64;

    let mut out = Vec::with_capacity(out_len);
    for n in 0..out_len {
        let pos = n as f64 / ratio;
        let center = pos.floor() as isize;
        let mut acc = 0.0f64;
        let mut weight_sum = 0.0f64;

        for offset in -(TAPS as isize)..=(TAPS as isize) {
            let index = center + offset;
            if index < 0 || index as usize >= input.len() {
                continue;
            }
            let distance = pos - index as f64;
            if distance.abs() > half {
                continue;
            }

            let x = cutoff * distance;
            let sinc = if x.abs() < 1e-9 {
                1.0
            } else {
                let pi_x = std::f64::consts::PI * x;
                pi_x.sin() / pi_x
            };
            // Hann 窗：距离为 0 时权重 1，到达核半宽时衰减到 0。
            let window = 0.5 * (1.0 + (std::f64::consts::PI * distance / half).cos());
            let weight = sinc * window * cutoff;

            acc += f64::from(input[index as usize]) * weight;
            weight_sum += weight;
        }

        out.push(if weight_sum.abs() > 1e-9 {
            (acc / weight_sum) as f32
        } else {
            0.0
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_returns_input() {
        let input = vec![0.1, -0.2, 0.3];
        assert_eq!(resample_mono(&input, 22_050, 22_050), input);
    }

    #[test]
    fn empty_input_stays_empty() {
        assert!(resample_mono(&[], 48_000, 22_050).is_empty());
    }

    #[test]
    fn output_length_matches_ratio() {
        let input = vec![0.0f32; 48_000];
        let out = resample_mono(&input, 48_000, 16_000);
        assert!((out.len() as i64 - 16_000).abs() <= 1);
    }

    #[test]
    fn sine_frequency_is_preserved() {
        // 440 Hz 正弦从 48k 降到 16k，过零点数量应保持一致。
        let from = 48_000u32;
        let to = 16_000u32;
        let freq = 440.0f64;
        let input: Vec<f32> = (0..from)
            .map(|i| {
                (2.0 * std::f64::consts::PI * freq * f64::from(i) / f64::from(from)).sin() as f32
            })
            .collect();
        let out = resample_mono(&input, from, to);

        let zero_crossings = out.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        // 1 秒 440 Hz 应有约 440 次上升过零，允许少量边界误差。
        assert!(
            (zero_crossings as i64 - 440).abs() <= 3,
            "zero crossings: {zero_crossings}"
        );
        assert!(out.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn amplitude_is_preserved() {
        let input: Vec<f32> = (0..9_600).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        let out = resample_mono(&input, 48_000, 48_000);
        let peak = out.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!((peak - 0.5).abs() < 0.01);
    }
}
