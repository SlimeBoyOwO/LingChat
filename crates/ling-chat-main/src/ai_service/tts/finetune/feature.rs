//! 声学特征：STFT 幅度谱与 log-mel。
//!
//! 训练声学模型需要和基座一致的特征。这里实现最常用的一套：周期 Hann 窗、
//! 居中反射填充、幅度谱、Slaney 归一化的三角 mel 滤波器组（与 librosa 默认一致），
//! 最后取自然对数。纯 Rust，桌面和移动端结果一致。

/// mel 特征配置。
#[derive(Debug, Clone, Copy)]
pub struct MelConfig {
    /// 采样率。
    pub sample_rate: u32,
    /// FFT 点数，必须是 2 的幂。
    pub n_fft: usize,
    /// 帧移。
    pub hop_length: usize,
    /// 窗长，不超过 n_fft。
    pub win_length: usize,
    /// mel 通道数。
    pub n_mels: usize,
    /// 最低频率。
    pub fmin: f32,
    /// 最高频率。
    pub fmax: f32,
}

impl Default for MelConfig {
    /// 与 matcha-icefall-zh-baker 一类的中文声学模型对齐。
    fn default() -> Self {
        Self {
            sample_rate: 22_050,
            n_fft: 1024,
            hop_length: 256,
            win_length: 1024,
            n_mels: 80,
            fmin: 0.0,
            fmax: 8_000.0,
        }
    }
}

impl MelConfig {
    /// 频点数（单边谱）。
    pub fn bins(&self) -> usize {
        self.n_fft / 2 + 1
    }
}

/// 周期 Hann 窗。
pub fn hann_window(length: usize) -> Vec<f32> {
    (0..length)
        .map(|n| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / length as f32).cos())
        .collect()
}

/// 就地基 2 FFT（输入长度必须是 2 的幂）。
pub fn fft_in_place(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert_eq!(n, im.len());
    debug_assert!(n.is_power_of_two());

    // 位反转置换。
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    let mut len = 2;
    while len <= n {
        let angle = -2.0 * std::f32::consts::PI / len as f32;
        let (twiddle_re, twiddle_im) = (angle.cos(), angle.sin());
        let half = len / 2;

        let mut start = 0;
        while start < n {
            let mut cur_re = 1.0f32;
            let mut cur_im = 0.0f32;
            for k in 0..half {
                let (ur, ui) = (re[start + k], im[start + k]);
                let (vr_raw, vi_raw) = (re[start + k + half], im[start + k + half]);
                let vr = vr_raw * cur_re - vi_raw * cur_im;
                let vi = vr_raw * cur_im + vi_raw * cur_re;

                re[start + k] = ur + vr;
                im[start + k] = ui + vi;
                re[start + k + half] = ur - vr;
                im[start + k + half] = ui - vi;

                let next_re = cur_re * twiddle_re - cur_im * twiddle_im;
                cur_im = cur_re * twiddle_im + cur_im * twiddle_re;
                cur_re = next_re;
            }
            start += len;
        }
        len <<= 1;
    }
}

/// 单边幅度谱，返回帧数乘频点数。
pub fn stft_magnitude(samples: &[f32], config: &MelConfig) -> Vec<Vec<f32>> {
    let n_fft = config.n_fft;
    if !n_fft.is_power_of_two() || samples.is_empty() {
        return Vec::new();
    }

    let win_length = config.win_length.min(n_fft);
    let window = hann_window(win_length);
    let padding = n_fft / 2;
    let padded = reflect_pad(samples, padding);

    if padded.len() < n_fft {
        return Vec::new();
    }

    let hop = config.hop_length.max(1);
    let frames = (padded.len() - n_fft) / hop + 1;
    let bins = config.bins();
    let window_offset = (n_fft - win_length) / 2;

    let mut out = Vec::with_capacity(frames);
    for frame in 0..frames {
        let start = frame * hop;
        let mut re = vec![0.0f32; n_fft];
        let mut im = vec![0.0f32; n_fft];

        for i in 0..win_length {
            let index = start + i;
            if index < padded.len() {
                re[window_offset + i] = padded[index] * window[i];
            }
        }

        fft_in_place(&mut re, &mut im);
        let mut magnitude = Vec::with_capacity(bins);
        for k in 0..bins {
            magnitude.push((re[k] * re[k] + im[k] * im[k]).sqrt());
        }
        out.push(magnitude);
    }

    out
}

/// 三角 mel 滤波器组。
#[derive(Debug, Clone)]
pub struct MelFilterbank {
    /// 行是 mel 通道，列是频点。
    filters: Vec<Vec<f32>>,
}

impl MelFilterbank {
    /// 按配置构造滤波器组。
    pub fn new(config: &MelConfig) -> Self {
        let bins = config.bins();
        let fft_freqs: Vec<f32> = (0..bins)
            .map(|k| k as f32 * config.sample_rate as f32 / config.n_fft as f32)
            .collect();

        let mel_min = hz_to_mel(config.fmin);
        let mel_max = hz_to_mel(config.fmax.max(config.fmin));
        let points: Vec<f32> = (0..config.n_mels + 2)
            .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (config.n_mels + 1) as f32)
            .collect();
        let hz_points: Vec<f32> = points.iter().map(|mel| mel_to_hz(*mel)).collect();

        let mut filters = Vec::with_capacity(config.n_mels);
        for m in 0..config.n_mels {
            let lower = hz_points[m];
            let center = hz_points[m + 1];
            let upper = hz_points[m + 2];
            // Slaney 面积归一化：让每个三角的面积一致。
            let enorm = if upper > lower {
                2.0 / (upper - lower)
            } else {
                0.0
            };

            let mut row = vec![0.0f32; bins];
            for (k, freq) in fft_freqs.iter().enumerate() {
                let weight = if *freq >= lower && *freq <= center && center > lower {
                    (freq - lower) / (center - lower)
                } else if *freq > center && *freq <= upper && upper > center {
                    (upper - freq) / (upper - center)
                } else {
                    0.0
                };
                row[k] = weight * enorm;
            }
            filters.push(row);
        }

        Self { filters }
    }

    /// 对一帧幅度谱做 mel 投影。
    pub fn apply(&self, magnitude: &[f32]) -> Vec<f32> {
        self.filters
            .iter()
            .map(|row| {
                row.iter()
                    .zip(magnitude.iter())
                    .map(|(weight, value)| weight * value)
                    .sum()
            })
            .collect()
    }

    /// mel 通道数。
    pub fn len(&self) -> usize {
        self.filters.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }
}

/// 取对数后的 mel 特征，返回帧数乘通道数。
pub fn log_mel(samples: &[f32], config: &MelConfig) -> Vec<Vec<f32>> {
    let spectrum = stft_magnitude(samples, config);
    if spectrum.is_empty() {
        return Vec::new();
    }
    let filterbank = MelFilterbank::new(config);
    spectrum
        .iter()
        .map(|magnitude| {
            filterbank
                .apply(magnitude)
                .into_iter()
                .map(|value| value.max(1e-5).ln())
                .collect()
        })
        .collect()
}

/// 反射填充，避免首尾帧能量塌陷。
fn reflect_pad(samples: &[f32], padding: usize) -> Vec<f32> {
    if padding == 0 {
        return samples.to_vec();
    }
    if samples.len() < 2 {
        let fill = samples.first().copied().unwrap_or(0.0);
        return vec![fill; samples.len() + padding * 2];
    }

    let len = samples.len();
    let mut out = Vec::with_capacity(len + padding * 2);
    for i in 0..padding {
        let index = reflect_index(i as isize - padding as isize, len);
        out.push(samples[index]);
    }
    out.extend_from_slice(samples);
    for i in 0..padding {
        let index = reflect_index(len as isize + i as isize, len);
        out.push(samples[index]);
    }
    out
}

fn reflect_index(mut index: isize, len: usize) -> usize {
    let len = len as isize;
    while index < 0 || index >= len {
        if index < 0 {
            index = -index;
        } else {
            index = 2 * len - 2 - index;
        }
    }
    index as usize
}

fn hz_to_mel(hz: f32) -> f32 {
    const F_SP: f32 = 200.0 / 3.0;
    const MIN_LOG_HZ: f32 = 1_000.0;
    const MIN_LOG_MEL: f32 = 15.0;
    let logstep = (6.4f32).ln() / 27.0;
    if hz >= MIN_LOG_HZ {
        MIN_LOG_MEL + (hz / MIN_LOG_HZ).ln() / logstep
    } else {
        hz / F_SP
    }
}

fn mel_to_hz(mel: f32) -> f32 {
    const F_SP: f32 = 200.0 / 3.0;
    const MIN_LOG_HZ: f32 = 1_000.0;
    const MIN_LOG_MEL: f32 = 15.0;
    let logstep = (6.4f32).ln() / 27.0;
    if mel >= MIN_LOG_MEL {
        MIN_LOG_HZ * (logstep * (mel - MIN_LOG_MEL)).exp()
    } else {
        F_SP * mel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hann_window_is_periodic() {
        let window = hann_window(4);
        assert!((window[0] - 0.0).abs() < 1e-6);
        assert!((window[1] - 0.5).abs() < 1e-6);
        assert!((window[2] - 1.0).abs() < 1e-6);
        assert!((window[3] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn fft_of_impulse_is_flat() {
        let mut re = vec![1.0f32, 0.0, 0.0, 0.0];
        let mut im = vec![0.0f32; 4];
        fft_in_place(&mut re, &mut im);
        for value in &re {
            assert!((value - 1.0).abs() < 1e-5);
        }
        for value in &im {
            assert!(value.abs() < 1e-5);
        }
    }

    #[test]
    fn fft_finds_sine_bin() {
        let n = 64usize;
        let bin = 8usize;
        let mut re: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / n as f32).sin())
            .collect();
        let mut im = vec![0.0f32; n];
        fft_in_place(&mut re, &mut im);

        // 实信号频谱对称，只在单边谱里找峰值。
        let magnitude: Vec<f32> = (0..n / 2 + 1)
            .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt())
            .collect();
        let peak = magnitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(index, _)| index)
            .unwrap();
        assert_eq!(peak, bin);
    }

    #[test]
    fn log_mel_has_expected_shape() {
        let config = MelConfig::default();
        let samples = vec![0.1f32; 22_050];
        let mel = log_mel(&samples, &config);
        assert!(!mel.is_empty());
        assert!(mel.iter().all(|frame| frame.len() == config.n_mels));
    }

    #[test]
    fn silence_hits_log_floor() {
        let config = MelConfig::default();
        let samples = vec![0.0f32; 4_096];
        let mel = log_mel(&samples, &config);
        let floor = 1e-5f32.ln();
        for frame in mel {
            for value in frame {
                assert!((value - floor).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn tone_lands_in_expected_mel_bin() {
        let config = MelConfig::default();
        let freq = 1_000.0f32;
        let samples: Vec<f32> = (0..22_050)
            .map(|i| {
                (2.0 * std::f32::consts::PI * freq * i as f32 / config.sample_rate as f32).sin()
            })
            .collect();
        let mel = log_mel(&samples, &config);
        let frame = &mel[mel.len() / 2];
        let peak = frame
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(index, _)| index)
            .unwrap();

        // 找到中心频率最接近 1 kHz 的 mel 通道。
        let mel_min = hz_to_mel(config.fmin);
        let mel_max = hz_to_mel(config.fmax);
        let mut expected = 0usize;
        let mut best = f32::MAX;
        for m in 0..config.n_mels {
            let center_mel = mel_min + (mel_max - mel_min) * (m + 1) as f32 / (config.n_mels + 1) as f32;
            let distance = (mel_to_hz(center_mel) - freq).abs();
            if distance < best {
                best = distance;
                expected = m;
            }
        }
        assert!(
            (peak as i64 - expected as i64).abs() <= 2,
            "peak {peak}, expected {expected}"
        );
    }

    #[test]
    fn mel_filterbank_matches_expected_size() {
        let config = MelConfig::default();
        let filterbank = MelFilterbank::new(&config);
        assert_eq!(filterbank.len(), config.n_mels);
        assert!(!filterbank.is_empty());
    }
}
