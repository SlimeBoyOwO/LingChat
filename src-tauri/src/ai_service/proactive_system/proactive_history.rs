//! 主动搭话历史记录与反重复检测。
//!
//! 记录最近 N 条主动搭话文本及其时间戳，使用归一化后的编辑距离相似度
//! 判断新内容是否与近期内容重复。相似度超过阈值时建议跳过。

use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 默认保留历史条数。
const DEFAULT_MAX_HISTORY: usize = 10;
/// 默认相似度阈值，超过此值视为重复。
const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.85;
/// 默认历史有效期（毫秒），超过此时间的历史不再参与重复检测。
const DEFAULT_MAX_AGE_MS: u64 = 60 * 60 * 1000; // 1 小时

/// 一条主动搭话历史记录。
#[derive(Clone, Debug)]
pub struct ProactiveHistoryEntry {
    pub text: String,
    pub ts_ms: u64,
}

/// 主动搭话反重复器。
#[derive(Clone, Debug)]
pub struct ProactiveDeduplicator {
    max_history: usize,
    similarity_threshold: f64,
    max_age_ms: u64,
    entries: VecDeque<ProactiveHistoryEntry>,
}

impl Default for ProactiveDeduplicator {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_HISTORY,
            DEFAULT_SIMILARITY_THRESHOLD,
            DEFAULT_MAX_AGE_MS,
        )
    }
}

impl ProactiveDeduplicator {
    pub fn new(max_history: usize, similarity_threshold: f64, max_age_ms: u64) -> Self {
        Self {
            max_history: max_history.max(1),
            similarity_threshold: similarity_threshold.clamp(0.0, 1.0),
            max_age_ms,
            entries: VecDeque::new(),
        }
    }

    /// 把新内容加入历史。
    pub fn record(&mut self, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() || text.trim() == "[PASS]" {
            return;
        }
        let ts_ms = now_ms();
        self.entries
            .push_back(ProactiveHistoryEntry { text, ts_ms });
        self.evict(ts_ms);
    }

    /// 检查给定文本是否与近期历史重复。
    /// 返回 `(is_duplicate, best_similarity)`。
    pub fn is_duplicate(&self, text: &str) -> (bool, f64) {
        if text.trim().is_empty() || text.trim() == "[PASS]" {
            return (false, 0.0);
        }

        let now = now_ms();
        let current = normalize(text);
        if current.is_empty() {
            return (false, 0.0);
        }

        let mut best = 0.0;
        for entry in &self.entries {
            if now.saturating_sub(entry.ts_ms) > self.max_age_ms {
                continue;
            }
            let old = normalize(&entry.text);
            if old.is_empty() {
                continue;
            }
            let score = similarity_ratio(&current, &old);
            if score > best {
                best = score;
            }
            if best >= self.similarity_threshold {
                return (true, best);
            }
        }
        (false, best)
    }

    /// 如果内容不重复，记录并返回 false；如果重复，返回 true。
    #[cfg(test)]
    pub fn check_and_record(&mut self, text: impl Into<String>) -> (bool, f64) {
        let text = text.into();
        let (dup, score) = self.is_duplicate(&text);
        if !dup {
            self.record(text);
        }
        (dup, score)
    }

    fn evict(&mut self, now_ms: u64) {
        // 先移除过期
        while self
            .entries
            .front()
            .is_some_and(|e| now_ms.saturating_sub(e.ts_ms) > self.max_age_ms)
        {
            self.entries.pop_front();
        }
        // 再限制数量
        while self.entries.len() > self.max_history {
            self.entries.pop_front();
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

/// 归一化文本：小写、去标点、压缩空白、截断。
fn normalize(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_space = false;
    for ch in text.trim().to_lowercase().chars() {
        if ch.is_whitespace() {
            if !previous_space && !normalized.is_empty() {
                normalized.push(' ');
                previous_space = true;
            }
            continue;
        }
        if ch.is_ascii_punctuation() {
            continue;
        }
        normalized.push(ch);
        previous_space = false;
    }
    normalized.trim().chars().take(512).collect()
}

/// 编辑距离相似度：1 - distance / max_len，范围 [0, 1]。
fn similarity_ratio(left: &str, right: &str) -> f64 {
    if left == right {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();

    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (i, left_ch) in left_chars.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_ch) in right_chars.iter().enumerate() {
            let cost = usize::from(left_ch != right_ch);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    let distance = previous[right_chars.len()] as f64;
    let max_len = left_chars.len().max(right_chars.len()) as f64;
    (1.0 - distance / max_len).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_duplicate() {
        let mut dedup = ProactiveDeduplicator::default();
        assert!(!dedup.check_and_record("你好呀").0);
        assert!(dedup.check_and_record("你好呀").0);
    }

    #[test]
    fn test_similar_but_not_duplicate() {
        let mut dedup = ProactiveDeduplicator::new(10, 0.85, 60 * 60 * 1000);
        assert!(!dedup.check_and_record("今天天气不错").0);
        assert!(!dedup.check_and_record("今天天气很好").0);
    }

    #[test]
    fn test_pass_is_ignored() {
        let mut dedup = ProactiveDeduplicator::default();
        dedup.record("[PASS]");
        assert_eq!(dedup.len(), 0);
        assert!(!dedup.is_duplicate("[PASS]").0);
    }
}
