//! 嵌入模型分词器（读 `tokenizer.json`，与 HF tokenizers 行为对齐）。
//!
//! 支持两类常见小模型的配置：
//! - WordPiece（BERT）：`normalizer: BertNormalizer` +
//!   `pre_tokenizer: BertPreTokenizer` + `model: WordPiece`。覆盖
//!   `moka-ai/m3e-small`、`text2vec` 等 BERT 系模型。
//! - Unigram（SentencePiece，XLM-RoBERTa 系）：`pre_tokenizer: Metaspace(▁)` +
//!   `model: Unigram`，以 log-score 做 Viterbi 最优切分。覆盖
//!   `sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2`（多语言，
//!   含中/英/日）等共享 250k XLM-R 词表的模型。
//!
//! 记忆库每次启动重建，向量不持久化，因此不需要与任何历史输出对齐。
//!
//! 已知简化（Unigram 路径）：
//! - 跳过 XLM-R 的 `Precompiled` 归一化表（SentencePiece 的 NFKC/兼容映射），
//!   只做 `" {2,}"→" "` 折叠 + Metaspace 空格替换。对常规 NFC 的中英日文本无差异；
//!   全角/组合字符等冷门输入可能与 Python 参考略有出入。
//! - 未知字符按单字符 `<unk>` 输出（score 取词表最小值，避免劣化正常切分）。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

const DEFAULT_MAX_LENGTH: usize = 512;

/// 一条已分词输入及 attention mask。
pub struct Tokenized {
    pub input_ids: Vec<i64>,
    pub attention_mask: Vec<i64>,
}

/// 统一分词器：按 `tokenizer.json` 的 `model.type` 分发。
pub enum Tokenizer {
    WordPiece(BertTokenizer),
    SentencePiece(SentencePieceTokenizer),
}

impl Tokenizer {
    /// 从模型目录加载 `tokenizer.json`（并用 `tokenizer_config.json` 覆盖 special ids）。
    pub fn load(model_dir: &Path) -> Result<Self> {
        let tok_path = model_dir.join("tokenizer.json");
        let raw = std::fs::read_to_string(&tok_path)
            .with_context(|| format!("读取 tokenizer.json 失败: {}", tok_path.display()))?;
        let v: serde_json::Value =
            serde_json::from_str(&raw).context("解析 tokenizer.json 失败")?;
        let kind = v["model"]["type"]
            .as_str()
            .unwrap_or("")
            .to_ascii_lowercase();
        match kind.as_str() {
            "wordpiece" | "bertwordpiece" => Ok(Tokenizer::WordPiece(BertTokenizer::from_json(
                &raw, model_dir,
            )?)),
            "unigram" | "sentencepiece" => Ok(Tokenizer::SentencePiece(
                SentencePieceTokenizer::from_json(&raw, model_dir)?,
            )),
            other => Err(anyhow!(
                "不支持的 tokenizer 类型: {other}（需要 WordPiece/BERT 或 Unigram/SentencePiece）"
            )),
        }
    }

    pub fn encode(&self, text: &str) -> Tokenized {
        match self {
            Tokenizer::WordPiece(t) => t.encode(text),
            Tokenizer::SentencePiece(t) => t.encode(text),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn sep_id(&self) -> i64 {
        match self {
            Tokenizer::WordPiece(t) => t.sep_id,
            Tokenizer::SentencePiece(t) => t.sep_id,
        }
    }
}

// ── WordPiece（BERT）───────────────────────────────────────────────────

#[derive(Deserialize)]
struct TokenizerFile<'a> {
    #[serde(borrow)]
    model: ModelDef<'a>,
    #[serde(default)]
    special_tokens: Vec<SpecialTokenDef>,
    normalizer: Option<NormalizerDef>,
}

#[derive(Deserialize)]
struct ModelDef<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    vocab: HashMap<String, i64>,
    #[serde(rename = "unknown_token")]
    unknown_token: Option<String>,
    #[serde(rename = "continuing_subword_prefix")]
    continuing_subword_prefix: Option<String>,
}

#[derive(Deserialize)]
struct SpecialTokenDef {
    content: String,
    id: i64,
}

#[derive(Deserialize)]
struct NormalizerDef {
    #[serde(default, rename = "lowercase")]
    lower_case: bool,
    #[serde(default)]
    clean_text: bool,
    #[serde(default)]
    handle_chinese_chars: bool,
}

/// BERT WordPiece 分词器。
pub struct BertTokenizer {
    vocab: HashMap<String, i64>,
    prefix: String,
    unk_id: i64,
    sep_id: i64,
    cls_id: i64,
    pad_id: i64,
    max_length: usize,
    lower_case: bool,
    clean_text: bool,
    handle_chinese_chars: bool,
}

impl BertTokenizer {
    fn from_json(raw: &str, model_dir: &Path) -> Result<Self> {
        let file: TokenizerFile<'_> =
            serde_json::from_str(raw).context("解析 tokenizer.json 失败")?;

        let kind = file.model.kind.to_ascii_lowercase();
        if !(kind == "wordpiece" || kind == "bertwordpiece") {
            return Err(anyhow!(
                "不支持的 tokenizer 类型: {}（需要 WordPiece/BERT）",
                file.model.kind
            ));
        }

        let mut vocab = file.model.vocab;
        for t in file.special_tokens {
            vocab.insert(t.content, t.id);
        }

        let unk_token = file
            .model
            .unknown_token
            .unwrap_or_else(|| "[UNK]".to_string());
        let mut unk_id = *vocab.get(unk_token.as_str()).unwrap_or(&100);
        let mut sep_id = *vocab.get("[SEP]").unwrap_or(&102);
        let mut cls_id = *vocab.get("[CLS]").unwrap_or(&101);
        let mut pad_id = *vocab.get("[PAD]").unwrap_or(&0);

        // tokenizer_config.json 里若有显式 special token id 则覆盖。
        if let Ok(cfg_raw) = std::fs::read_to_string(model_dir.join("tokenizer_config.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cfg_raw) {
                for (key, slot) in [
                    ("unk_token_id", &mut unk_id),
                    ("cls_token_id", &mut cls_id),
                    ("sep_token_id", &mut sep_id),
                    ("pad_token_id", &mut pad_id),
                ] {
                    if let Some(id) = v.get(key).and_then(|x| x.as_i64()) {
                        *slot = id;
                    }
                }
            }
        }

        let norm = file.normalizer.unwrap_or(NormalizerDef {
            lower_case: false,
            clean_text: true,
            handle_chinese_chars: true,
        });

        Ok(Self {
            vocab,
            prefix: file
                .model
                .continuing_subword_prefix
                .unwrap_or_else(|| "##".to_string()),
            unk_id,
            sep_id,
            cls_id,
            pad_id,
            max_length: max_length(),
            lower_case: norm.lower_case,
            clean_text: norm.clean_text,
            handle_chinese_chars: norm.handle_chinese_chars,
        })
    }

    /// 单条文本 → input_ids / attention_mask（[CLS]+tokens+[SEP]，右 padding）。
    pub fn encode(&self, text: &str) -> Tokenized {
        let normalized = self.normalize(text);
        let pieces = pre_tokenize(&normalized);
        let mut ids: Vec<i64> = Vec::with_capacity(self.max_length);
        ids.push(self.cls_id);
        let body_budget = self.max_length.saturating_sub(2);
        for piece in pieces {
            for tid in self.wordpiece(&piece) {
                if ids.len() >= 1 + body_budget {
                    break;
                }
                ids.push(tid);
            }
            if ids.len() >= 1 + body_budget {
                break;
            }
        }
        ids.push(self.sep_id);

        let mut attention_mask = vec![1i64; ids.len()];
        let pad = self.max_length - ids.len();
        ids.extend(std::iter::repeat(self.pad_id).take(pad));
        attention_mask.extend(std::iter::repeat(0i64).take(pad));
        Tokenized {
            input_ids: ids,
            attention_mask,
        }
    }

    fn normalize(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() * 2);
        let mut prev_space = false;
        for ch in text.chars() {
            let code = ch as u32;
            if self.clean_text
                && (code == 0
                    || code == 0xfffd
                    || (code <= 0x1f && ch != '\t' && ch != '\n' && ch != '\r'))
            {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
                continue;
            }
            if ch.is_whitespace() {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
                continue;
            }
            if self.handle_chinese_chars && is_chinese_char(ch) {
                if !prev_space {
                    out.push(' ');
                }
                out.push(ch);
                out.push(' ');
                prev_space = true;
                continue;
            }
            let mut c = ch;
            if self.lower_case {
                if let Some(lc) = c.to_lowercase().next() {
                    c = lc;
                }
            }
            if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
                prev_space = true;
            } else {
                prev_space = false;
            }
            out.push(c);
        }
        out
    }

    /// 贪心最长匹配的 WordPiece；整词查不中则按 `##` 子词切，仍不中出 [UNK]。
    fn wordpiece(&self, token: &str) -> Vec<i64> {
        if token.is_empty() {
            return Vec::new();
        }
        if let Some(id) = self.vocab.get(token) {
            return vec![*id];
        }
        let chars: Vec<char> = token.chars().collect();
        let n = chars.len();
        let mut out = Vec::new();
        let mut start = 0;
        while start < n {
            let mut end = n;
            let mut best: Option<(i64, usize)> = None;
            while end > start {
                let candidate: String = if start == 0 {
                    chars[start..end].iter().collect()
                } else {
                    let mut s = String::with_capacity(self.prefix.len() + (end - start));
                    s.push_str(&self.prefix);
                    s.extend(chars[start..end].iter());
                    s
                };
                if let Some(id) = self.vocab.get(&candidate) {
                    best = Some((*id, end - start));
                    break;
                }
                end -= 1;
            }
            match best {
                Some((id, consumed)) => {
                    out.push(id);
                    start += consumed;
                },
                None => {
                    out.push(self.unk_id);
                    break;
                },
            }
        }
        out
    }
}

// ── Unigram（SentencePiece / XLM-RoBERTa）─────────────────────────────

/// 分词器文件的 Unigram 部分（vocab 形如 `[[piece, score], ...]`）。
#[derive(Deserialize)]
struct SpFile {
    #[serde(default)]
    model: SpModel,
    #[serde(default)]
    added_tokens: Vec<SpAddedToken>,
}

#[derive(Deserialize, Default)]
struct SpModel {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    unk_id: i64,
    #[serde(default)]
    vocab: Vec<(String, f64)>,
}

#[derive(Deserialize)]
struct SpAddedToken {
    content: String,
    id: i64,
}

/// SentencePiece Unigram 分词器（XLM-RoBERTa 系，如 paraphrase-multilingual-MiniLM）。
pub struct SentencePieceTokenizer {
    /// piece → (id, log-score)。ids 即 vocab 索引。
    pieces: HashMap<String, (i64, f32)>,
    /// 未知字符的 score（取词表最小值，保证任何真实词优先于 unk）。
    unk_score: f32,
    unk_id: i64,
    sep_id: i64,
    cls_id: i64,
    pad_id: i64,
    max_length: usize,
    add_prefix_space: bool,
}

impl SentencePieceTokenizer {
    fn from_json(raw: &str, model_dir: &Path) -> Result<Self> {
        let file: SpFile = serde_json::from_str(raw).context("解析 tokenizer.json 失败")?;
        let kind = file.model.kind.to_ascii_lowercase();
        if !(kind == "unigram" || kind == "sentencepiece") {
            return Err(anyhow!(
                "不支持的 tokenizer 类型: {}（需要 Unigram/SentencePiece）",
                file.model.kind
            ));
        }

        let mut pieces: HashMap<String, (i64, f32)> = HashMap::with_capacity(300000);
        let mut min_score = f32::MAX;
        for (i, (piece, score)) in file.model.vocab.into_iter().enumerate() {
            let s = score as f32;
            pieces.insert(piece, (i as i64, s));
            if s < min_score {
                min_score = s;
            }
        }
        // added_tokens（如 <mask>）合入词表，id 取显式值。
        for t in file.added_tokens {
            pieces.insert(t.content, (t.id, min_score));
        }

        // 特殊 token id：XLM-R 约定 <s>=0, <pad>=1, </s>=2, <unk>=3。
        let id_of = |name: &str, fallback: i64| -> i64 {
            pieces.get(name).map(|&(id, _)| id).unwrap_or(fallback)
        };
        let mut unk_id = if file.model.unk_id != 0 {
            file.model.unk_id
        } else {
            id_of("<unk>", 3)
        };
        let mut sep_id = id_of("</s>", 2);
        let mut cls_id = id_of("<s>", 0);
        let mut pad_id = id_of("<pad>", 1);

        // tokenizer_config.json 里若有显式 special token id 则覆盖。
        if let Ok(cfg_raw) = std::fs::read_to_string(model_dir.join("tokenizer_config.json")) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&cfg_raw) {
                for (key, slot) in [
                    ("unk_token_id", &mut unk_id),
                    ("cls_token_id", &mut cls_id),
                    ("sep_token_id", &mut sep_id),
                    ("pad_token_id", &mut pad_id),
                ] {
                    if let Some(id) = v.get(key).and_then(|x| x.as_i64()) {
                        *slot = id;
                    }
                }
            }
        }

        Ok(Self {
            pieces,
            unk_score: min_score,
            unk_id,
            sep_id,
            cls_id,
            pad_id,
            max_length: max_length(),
            add_prefix_space: true,
        })
    }

    /// 单条文本 → input_ids / attention_mask（<s>+pieces+</s>，右侧截断，右 padding）。
    pub fn encode(&self, text: &str) -> Tokenized {
        // 归一化：`" {2,}" → " "`（跳过 Precompiled 表，见模块注释）。
        let collapsed = collapse_space_runs(text);
        let segments = metaspace_pre_tokenize(&collapsed, self.add_prefix_space);

        let mut ids: Vec<i64> = Vec::with_capacity(self.max_length.min(4096));
        ids.push(self.cls_id);
        let body_budget = self.max_length.saturating_sub(2);
        'outer: for seg in segments {
            for tid in self.segment(&seg) {
                if ids.len() >= 1 + body_budget {
                    break 'outer;
                }
                ids.push(tid);
            }
            if ids.len() >= 1 + body_budget {
                break;
            }
        }
        ids.push(self.sep_id);

        let mut attention_mask = vec![1i64; ids.len()];
        let pad = self.max_length.saturating_sub(ids.len());
        ids.extend(std::iter::repeat(self.pad_id).take(pad));
        attention_mask.extend(std::iter::repeat(0i64).take(pad));
        Tokenized {
            input_ids: ids,
            attention_mask,
        }
    }

    /// 对单个 metaspace 段（形如 `▁word`）做 Unigram Viterbi 最优切分。
    fn segment(&self, piece: &str) -> Vec<i64> {
        if piece.is_empty() {
            return Vec::new();
        }
        // 快路径：整段即一个词（绝大多数英文词/中文整词）。
        if let Some(&(id, _)) = self.pieces.get(piece) {
            return vec![id];
        }
        let chars: Vec<char> = piece.chars().collect();
        let n = chars.len();
        if n == 0 {
            return Vec::new();
        }

        const MAX_LEN: usize = 64; // 大于任何常见 subword 长度
        let mut best = vec![f32::NEG_INFINITY; n + 1];
        let mut prev = vec![0usize; n + 1];
        let mut tok = vec![self.unk_id; n + 1];
        best[0] = 0.0;

        let mut cand = String::new();
        for s in 0..n {
            if best[s].is_infinite() {
                continue;
            }
            cand.clear();
            let max_e = (s + MAX_LEN).min(n);
            let mut found = false;
            for e in s + 1..=max_e {
                cand.push(chars[e - 1]);
                if let Some(&(id, score)) = self.pieces.get(cand.as_str()) {
                    let v = best[s] + score;
                    if v > best[e] {
                        best[e] = v;
                        prev[e] = s;
                        tok[e] = id;
                    }
                    found = true;
                }
            }
            // 该位置无任何词覆盖：按单字符 <unk> 兜底（用最小 score）。
            if !found && best[s] + self.unk_score > best[s + 1] {
                best[s + 1] = best[s] + self.unk_score;
                prev[s + 1] = s;
                tok[s + 1] = self.unk_id;
            }
        }

        if best[n].is_infinite() {
            return vec![self.unk_id];
        }
        let mut out = Vec::with_capacity(n / 2 + 1);
        let mut i = n;
        while i > 0 {
            out.push(tok[i]);
            i = prev[i];
        }
        out.reverse();
        out
    }
}

/// 折叠连续 ASCII 空格为单个（对应 normalizer 的 `" {2,}" → " "`）。
fn collapse_space_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for c in text.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// Metaspace 预分词：空白做分隔，每个非空段前加 `▁`。
///
/// `add_prefix_space` 语义等价于每段（含首段）都带前导 `▁`：tokenizers 实现中
/// `add_prefix_space` 会给整句补一个前导空格，随后该空格被替换为 `▁`。
fn metaspace_pre_tokenize(text: &str, _add_prefix_space: bool) -> Vec<String> {
    let mut out = Vec::new();
    for word in text.split_whitespace() {
        let mut t = String::with_capacity(word.len() + 3);
        t.push('▁');
        t.push_str(word);
        out.push(t);
    }
    out
}

fn max_length() -> usize {
    std::env::var("EMBEDDING_MAX_LENGTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_LENGTH)
}

// ── BertPreTokenizer 近似（WordPiece 路径）─────────────────────────────

/// BertPreTokenizer 近似：空白已在前一阶段归一化，这里按标点隔离切分。
fn pre_tokenize(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    for part in text.split(' ') {
        if part.is_empty() {
            continue;
        }
        let mut cur = String::new();
        for c in part.chars() {
            if is_word_char(c) {
                cur.push(c);
            } else {
                if !cur.is_empty() {
                    result.push(std::mem::take(&mut cur));
                }
                result.push(c.to_string());
            }
        }
        if !cur.is_empty() {
            result.push(cur);
        }
    }
    result
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || is_chinese_char(c)
}

/// BERT 官方 `_is_chinese_char`。
fn is_chinese_char(c: char) -> bool {
    let cp = c as u32;
    (cp >= 0x3400 && cp <= 0x4dbf)
        || (cp >= 0x4e00 && cp <= 0x9fff)
        || (cp >= 0xf900 && cp <= 0xfaff)
        || (cp >= 0x20000 && cp <= 0x3134f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 参考 ids 来自 Python `tokenizers.Tokenizer.from_file`（paraphrase-multilingual-
    /// MiniLM-L12-v2 的 250k XLM-R Unigram 词表）。若换模型/词表需同步更新。
    ///
    /// 注意：切分把前导 `▁` 合入首段，输出与 Python 参考一致（按 `</s>` 截断）。
    fn e5_ids(text: &str) -> Vec<i64> {
        let model =
            std::env::var("EMBEDDING_TEST_MODEL").unwrap_or_else(|_| "/tmp/tiny_emo".into());
        let repo_path = |p: &str| -> PathBuf {
            let b = PathBuf::from(p);
            if b.is_absolute() {
                b
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join(&b)
            }
        };
        let model_dir = match std::fs::canonicalize(repo_path(&model)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("跳过：模型目录不可用 ({model}): {e}");
                return Vec::new();
            },
        };
        let tok = Tokenizer::load(&model_dir).unwrap();
        let sep = tok.sep_id();
        tok.encode(text)
            .input_ids
            .into_iter()
            .take_while(|&id| id != sep)
            .collect()
    }

    #[test]
    fn unigram_matches_python_tokenizers_reference() {
        // 参考 ids 来自 Python `tokenizers.Tokenizer.from_file`（XLM-R 250k Unigram
        // 词表，2026-09 抓取）。若换模型/词表需同步更新。
        let expected: Vec<(String, Vec<i64>)> = vec![
            ("你好世界".into(), vec![0, 6, 124084, 3221]),
            ("hello world".into(), vec![0, 33600, 31, 8999]),
            ("こんにちは世界".into(), vec![0, 6, 192661, 3221]),
            ("今天天气怎么样".into(), vec![0, 61168, 70871, 93985]),
            ("user likes coffee".into(), vec![0, 38937, 1884, 7, 79497]),
        ];
        for (text, exp_body) in &expected {
            let got = e5_ids(text);
            if got.is_empty() {
                eprintln!("跳过（无模型）: {text}");
                continue;
            }
            assert_eq!(got, *exp_body, "text={text:?}");
        }
    }
}
