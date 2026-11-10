//! 文本前端：把文本转成基座模型认识的符号 id 序列。
//!
//! 微调必须和基座用同一套符号表，否则导出的 ONNX 在推理端对不上。这里不做任何
//! 音素化算法，只读模型自带的 `tokens.txt` 与 `lexicon.txt`：
//!
//! - `tokens.txt`：每行 "符号 id"，符号可以是单个汉字、拼音、音素或特殊标记。
//! - `lexicon.txt`：每行 "词 符号1 符号2 ..."，中文模型里符号通常是带声调拼音。
//!
//! 切词用正向最长匹配。切不出词时退到单字条目，再退到单字符符号；都不认识的
//! 字符直接跳过，不影响其余文本。

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};

/// 符号表：符号到 id，以及反查表。
#[derive(Debug, Clone)]
pub struct TokenTable {
    symbol_to_id: HashMap<String, i32>,
    id_to_symbol: Vec<String>,
}

impl TokenTable {
    /// 读取 `tokens.txt`。
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 tokens 失败: {}", path.display()))?;
        Self::parse(&content).with_context(|| format!("解析 tokens 失败: {}", path.display()))
    }

    /// 解析 `tokens.txt` 文本内容。
    pub fn parse(content: &str) -> Result<Self> {
        let mut symbol_to_id = HashMap::new();
        let mut entries: Vec<(i32, String)> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // 从右侧切一次：最后一段是 id，前面整段是符号（符号本身可能含空格）。
            let mut parts = line.rsplitn(2, char::is_whitespace);
            let id_text = parts.next().unwrap_or_default();
            let symbol = parts.next().unwrap_or_default().trim().to_string();
            let id: i32 = id_text
                .parse()
                .with_context(|| format!("tokens 行的 id 不是整数: {line}"))?;
            // sherpa 的 tokens.txt 常以仅含 id 的行打头（例如 `0`，对应换行符/占位符）。
            // 空符号同样进查找表，保证符号总数（n_vocab）与基座模型一致。
            symbol_to_id.insert(symbol.clone(), id);
            entries.push((id, symbol));
        }

        if entries.is_empty() {
            bail!("tokens 文件为空");
        }

        let max_id = entries.iter().map(|(id, _)| *id).max().unwrap_or(0);
        let mut id_to_symbol = vec![String::new(); (max_id + 1).max(0) as usize];
        for (id, symbol) in entries {
            if id >= 0 {
                id_to_symbol[id as usize] = symbol;
            }
        }

        Ok(Self {
            symbol_to_id,
            id_to_symbol,
        })
    }

    /// 符号对应的 id。
    pub fn id(&self, symbol: &str) -> Option<i32> {
        self.symbol_to_id.get(symbol).copied()
    }

    /// id 对应的符号。
    pub fn symbol(&self, id: i32) -> Option<&str> {
        self.id_to_symbol.get(id as usize).map(String::as_str)
    }

    /// 符号总数。
    pub fn len(&self) -> usize {
        self.symbol_to_id.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.symbol_to_id.is_empty()
    }
}

/// 词表：词到符号序列。
#[derive(Debug, Clone)]
pub struct Lexicon {
    entries: HashMap<String, Vec<String>>,
    max_word_chars: usize,
}

impl Lexicon {
    /// 读取 `lexicon.txt`。
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 lexicon 失败: {}", path.display()))?;
        Self::parse(&content).with_context(|| format!("解析 lexicon 失败: {}", path.display()))
    }

    /// 解析 `lexicon.txt` 文本内容。
    pub fn parse(content: &str) -> Result<Self> {
        let mut entries = HashMap::new();
        let mut max_word_chars = 1;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(word) = parts.next() else {
                continue;
            };
            let phones: Vec<String> = parts.map(str::to_string).collect();
            if phones.is_empty() {
                continue;
            }
            max_word_chars = max_word_chars.max(word.chars().count());
            entries.insert(word.to_string(), phones);
        }

        if entries.is_empty() {
            bail!("lexicon 文件为空");
        }

        Ok(Self {
            entries,
            max_word_chars,
        })
    }

    /// 查词的符号序列。
    pub fn get(&self, word: &str) -> Option<&[String]> {
        self.entries.get(word).map(Vec::as_slice)
    }

    /// 最长词包含的字符数，用于限制最长匹配长度。
    pub fn max_word_chars(&self) -> usize {
        self.max_word_chars
    }

    /// 词条数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 文本前端：符号表 + 可选的词表。
#[derive(Debug, Clone)]
pub struct TextFrontend {
    tokens: TokenTable,
    lexicon: Option<Lexicon>,
}

impl TextFrontend {
    /// 从模型目录加载。
    ///
    /// `lexicon` 为 None 时退化为逐字符查符号表，适合字符级或音素级模型。
    pub fn load(tokens_path: &Path, lexicon_path: Option<&Path>) -> Result<Self> {
        let tokens = TokenTable::load(tokens_path)?;
        let lexicon = lexicon_path.map(Lexicon::load).transpose()?;
        Ok(Self { tokens, lexicon })
    }

    /// 符号表引用。
    pub fn tokens(&self) -> &TokenTable {
        &self.tokens
    }

    /// 词表引用。
    pub fn lexicon(&self) -> Option<&Lexicon> {
        self.lexicon.as_ref()
    }

    /// 文本转 id 序列。
    ///
    /// 空白字符直接丢弃；拉丁字母统一小写；无法识别的字符跳过。
    pub fn text_to_ids(&self, text: &str) -> Vec<i32> {
        let chars: Vec<char> = text.chars().collect();
        let mut ids = Vec::new();
        let mut index = 0;

        while index < chars.len() {
            let current = chars[index];
            if current.is_whitespace() {
                index += 1;
                continue;
            }

            if let Some(lexicon) = &self.lexicon {
                let max_len = lexicon.max_word_chars().min(chars.len() - index);
                let mut matched = None;
                for len in (1..=max_len).rev() {
                    let word: String = chars[index..index + len]
                        .iter()
                        .flat_map(|c| c.to_lowercase())
                        .collect();
                    if let Some(phones) = lexicon.get(&word) {
                        matched = Some((len, phones.to_vec()));
                        break;
                    }
                }
                if let Some((len, phones)) = matched {
                    for phone in phones {
                        if let Some(id) = self.tokens.id(&phone) {
                            ids.push(id);
                        }
                    }
                    index += len;
                    continue;
                }
            }

            // 单字符兜底。
            let symbol: String = current.to_lowercase().collect();
            if let Some(id) = self.tokens.id(&symbol) {
                ids.push(id);
            }
            index += 1;
        }

        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frontend(tokens: &str, lexicon: Option<&str>) -> TextFrontend {
        let dir = tempfile::tempdir().unwrap();
        let tokens_path = dir.path().join("tokens.txt");
        std::fs::write(&tokens_path, tokens).unwrap();

        let lexicon_path = lexicon.map(|content| {
            let path = dir.path().join("lexicon.txt");
            std::fs::write(&path, content).unwrap();
            path
        });

        TextFrontend::load(&tokens_path, lexicon_path.as_deref()).unwrap()
    }

    #[test]
    fn parses_token_table() {
        let table = TokenTable::parse("<blank> 0\na 1\n\u{4f60} 2\n").unwrap();
        assert_eq!(table.id("<blank>"), Some(0));
        assert_eq!(table.id("a"), Some(1));
        assert_eq!(table.symbol(2), Some("\u{4f60}"));
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn rejects_bad_token_line() {
        assert!(TokenTable::parse("a not_a_number\n").is_err());
        assert!(TokenTable::parse("").is_err());
    }

    #[test]
    fn longest_match_prefers_words() {
        let fe = frontend(
            "\u{4f60} 0\n\u{597d} 1\nni3 2\nhao3 3\n",
            Some("\u{4f60}\u{597d} ni3 hao3\n\u{4f60} ni3\n\u{597d} hao3\n"),
        );
        assert_eq!(fe.text_to_ids("\u{4f60}\u{597d}"), vec![2, 3]);
    }

    #[test]
    fn falls_back_to_single_char() {
        let fe = frontend("a 0\nb 1\n", Some("ab a b\n"));
        // 词典里有 ab，但只给单个字符时走单字符兜底。
        assert_eq!(fe.text_to_ids("a"), vec![0]);
    }

    #[test]
    fn unknown_characters_are_skipped() {
        let fe = frontend("a 0\nb 1\n", None);
        // 问号和空格都不在符号表里，直接跳过。
        assert_eq!(fe.text_to_ids("a?b"), vec![0, 1]);
    }

    #[test]
    fn whitespace_is_ignored() {
        let fe = frontend("a 0\n", None);
        assert_eq!(fe.text_to_ids(" a a "), vec![0, 0]);
    }

    #[test]
    fn latin_is_lowercased() {
        let fe = frontend("a 0\n", None);
        assert_eq!(fe.text_to_ids("A"), vec![0]);
    }
}
