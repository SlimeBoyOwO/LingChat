//! 学习权重持久化：`learned_weights.bin` 的读写。
//!
//! 格式（小端）：
//! ```text
//! magic "FBLW1" (5B) | saved_at u64 | rewards u64 | punishes u64 | n_edges u64
//! | weight f32 × n_edges   （共 29 + 4·E 字节，E=2,484,367 时约 9.9MB）
//! ```
//! - 写出为原子操作（先 .tmp 再 rename；Windows 上 rename 覆盖已存在文件会失败，
//!   先删目标）；
//! - 加载校验 magic 与 n_edges 匹配才覆盖 weight；`w0`（出厂基线）保持不动，
//!   `reset_learned_weights` 语义不变；
//! - 头信息（saved_at/rewards/punishes）可独立于脑加载读取（model_status 用）。

use std::path::Path;

use tracing::{info, warn};

use super::engine::Brain;

pub const MAGIC: &[u8; 5] = b"FBLW1";
const HEADER_LEN: usize = 5 + 8 * 4;

/// learned 文件头（model_status 展示用）。
#[derive(Debug, Clone, Copy)]
pub struct LearnedHeader {
    pub saved_at: u64,
    pub rewards: u64,
    pub punishes: u64,
    pub n_edges: u64,
}

/// 当前时间（秒，UNIX epoch）。
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 是否有可保存的学习状态（开启过可塑性才有 w0 基线；从未开启时无可学权重）。
pub fn has_learned_state(brain: &Brain) -> bool {
    !brain.w0_is_empty()
}

/// 保存脑的当前权重到 path（原子写）。返回文件字节数。
pub fn save(path: &Path, brain: &Brain) -> Result<u64, String> {
    let e = brain.weight.len();
    let mut buf = Vec::with_capacity(HEADER_LEN + e * 4);
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&now_secs().to_le_bytes());
    buf.extend_from_slice(&brain.plastic_stats.rewards.to_le_bytes());
    buf.extend_from_slice(&brain.plastic_stats.punishes.to_le_bytes());
    buf.extend_from_slice(&(e as u64).to_le_bytes());
    for w in &brain.weight {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &buf).map_err(|e| format!("写 {} 失败: {e}", tmp.display()))?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("删除旧文件失败: {e}"))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| format!("rename 失败: {e}"))?;
    info!(
        "学习权重已保存: {} ({} 边, {} 字节)",
        path.display(),
        e,
        buf.len()
    );
    Ok(buf.len() as u64)
}

/// 从 path 加载权重覆盖脑的 weight。文件不存在返回 Ok(false)；
/// magic/n_edges 不匹配返回 Err（不改动脑）。
pub fn load_into(path: &Path, brain: &mut Brain) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let buf = std::fs::read(path).map_err(|e| format!("读 {} 失败: {e}", path.display()))?;
    if buf.len() < HEADER_LEN || &buf[..5] != MAGIC {
        return Err(format!("{} 不是 FBLW1 学习权重文件", path.display()));
    }
    let rd = |off: usize| u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
    let n_edges = rd(29) as usize;
    if n_edges != brain.weight.len() {
        return Err(format!(
            "学习权重边数 {} 与当前图 {} 不匹配，忽略加载",
            n_edges,
            brain.weight.len()
        ));
    }
    if buf.len() != HEADER_LEN + n_edges * 4 {
        return Err(format!("{} 长度 {} 与边数不符", path.display(), buf.len()));
    }
    for (i, w) in brain.weight.iter_mut().enumerate() {
        let off = HEADER_LEN + i * 4;
        *w = f32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
    }
    info!(
        "学习权重已加载: {} ({} 边, saved_at={}, rewards={}, punishes={})",
        path.display(),
        n_edges,
        rd(5),
        rd(13),
        rd(21)
    );
    Ok(true)
}

/// 只读文件头（不加载权重；model_status 用）。文件不存在/损坏返回 None。
pub fn read_header(path: &Path) -> Option<LearnedHeader> {
    let buf = std::fs::read(path).ok()?;
    if buf.len() < HEADER_LEN || &buf[..5] != MAGIC {
        warn!("{} 不是合法的 FBLW1 文件", path.display());
        return None;
    }
    let rd = |off: usize| u64::from_le_bytes(buf[off..off + 8].try_into().unwrap());
    Some(LearnedHeader {
        saved_at: rd(5),
        rewards: rd(13),
        punishes: rd(21),
        n_edges: rd(29),
    })
}

/// 删除 learned 文件（幂等：不存在也返回 Ok）。
pub fn remove(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            info!("学习权重文件已删除: {}", path.display());
            Ok(())
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("删除 {} 失败: {e}", path.display())),
    }
}
