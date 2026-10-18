use thiserror::Error;

use crate::memory::{MemoryChunk, MemoryContractError, MemoryDocument};

#[derive(Debug, Error)]
pub enum ChunkingError {
    #[error(transparent)]
    Contract(#[from] MemoryContractError),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error("分词失败: {0}")]
    Tokenizer(String),
    #[error("token 预算不足以容纳一个字符与特殊 token")]
    Budget,
}

pub fn chunk_document(
    document: &MemoryDocument,
    budget: usize,
    atomic: bool,
    count_tokens: impl Fn(&str) -> Result<usize, String>,
) -> Result<Vec<MemoryChunk>, ChunkingError> {
    document.validate()?;
    if document.content.trim().is_empty() {
        return Ok(Vec::new());
    }
    if atomic {
        return Ok(vec![whole_document_chunk(document)?]);
    }
    let count = |text: &str| count_tokens(text).map_err(ChunkingError::Tokenizer);
    if budget == 0 || count("")? >= budget {
        return Err(ChunkingError::Budget);
    }
    let mut pieces = Vec::new();
    let mut pending = String::new();
    for sentence in document
        .content
        .split_inclusive(['。', '！', '？', '\n', '.', '!', '?', '；', ';'])
    {
        let combined = format!("{pending}{sentence}");
        if count(&combined)? <= budget {
            pending = combined;
            if sentence.ends_with('\n') {
                pieces.push(std::mem::take(&mut pending));
            }
            continue;
        }
        if !pending.is_empty() {
            pieces.push(std::mem::take(&mut pending));
        }
        let mut boundaries: Vec<_> = sentence.char_indices().map(|(index, _)| index).collect();
        boundaries.push(sentence.len());
        let mut start = 0;
        while start < boundaries.len() - 1 {
            let end = fitting_prefix(sentence, &boundaries, start, budget, &count)?;
            let piece = &sentence[boundaries[start]..boundaries[end]];
            if end == boundaries.len() - 1 {
                pending.push_str(piece);
            } else {
                pieces.push(piece.to_owned());
            }
            start = end;
        }
        if sentence.ends_with('\n') && !pending.is_empty() {
            pieces.push(std::mem::take(&mut pending));
        }
    }
    if !pending.is_empty() {
        pieces.push(pending);
    }
    pieces
        .into_iter()
        .enumerate()
        .map(|(ordinal, content)| {
            make_chunk(document, ordinal as u32, content).map_err(ChunkingError::from)
        })
        .collect()
}

fn fitting_prefix(
    text: &str,
    boundaries: &[usize],
    start: usize,
    budget: usize,
    count: &impl Fn(&str) -> Result<usize, ChunkingError>,
) -> Result<usize, ChunkingError> {
    let remaining = boundaries.len() - 1 - start;
    let mut low = 0;
    let mut high = 1;
    loop {
        if count(&text[boundaries[start]..boundaries[start + high]])? > budget {
            break;
        }
        low = high;
        if high == remaining {
            return Ok(start + high);
        }
        high = high.saturating_mul(2).min(remaining);
    }
    high -= 1;
    while low < high {
        let middle = (low + high).div_ceil(2);
        if count(&text[boundaries[start]..boundaries[start + middle]])? <= budget {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    if low == 0 {
        return Err(ChunkingError::Budget);
    }
    Ok(start + low)
}

pub fn whole_document_chunk(document: &MemoryDocument) -> Result<MemoryChunk, serde_json::Error> {
    make_chunk(document, 0, document.content.clone())
}

fn make_chunk(
    document: &MemoryDocument,
    ordinal: u32,
    content: String,
) -> Result<MemoryChunk, serde_json::Error> {
    let mut chunk = MemoryChunk {
        id: format!("{}:{ordinal}", document.id),
        document_id: document.id.clone(),
        ordinal,
        content,
        version: String::new(),
        sources: document.sources.clone(),
    };
    chunk.version = chunk.content_version()?;
    Ok(chunk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Note, memory::MemoryDocument};

    fn document(content: &str) -> MemoryDocument {
        MemoryDocument::from_note(
            2,
            &Note {
                id: "long".into(),
                content: content.into(),
                tags: vec![],
                created_at: "2026-10-18".into(),
            },
        )
        .unwrap()
    }

    #[test]
    fn sentence_and_unicode_splits_preserve_every_character_and_source() {
        let document = document("第一句话。第二句话！🙂超长段落没有标点abcdef中文测试\n最后一句。");
        let chunks =
            chunk_document(&document, 10, false, |text| Ok(text.chars().count() + 2)).unwrap();
        assert!(chunks.len() > 2);
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.content.as_str())
                .collect::<String>(),
            document.content
        );
        for chunk in &chunks {
            assert!(chunk.content.chars().count() + 2 <= 10);
            chunk.validate_for(&document).unwrap();
        }
        assert!(chunks[0].content.ends_with('。'));
    }

    #[test]
    fn atomic_protocol_is_never_cut_to_fit_budget() {
        let document = document(&"工具返回".repeat(100));
        let chunks = chunk_document(&document, 10, true, |_| {
            panic!("完整事务由宿主决定是否可嵌入")
        })
        .unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, document.content);
    }

    #[test]
    fn tokenizer_errors_and_impossible_budgets_are_explicit() {
        let document = document("你好");
        assert!(matches!(
            chunk_document(&document, 10, false, |_| Err("missing".into())),
            Err(ChunkingError::Tokenizer(_))
        ));
        assert!(matches!(
            chunk_document(&document, 2, false, |text| Ok(text.chars().count() + 2)),
            Err(ChunkingError::Budget)
        ));
    }

    #[test]
    fn newline_topics_remain_separate_even_when_the_whole_document_fits() {
        let document = document("奶茶三分糖。\n晓禾送来银色书签。\n盆栽放在东窗台。");
        let chunks =
            chunk_document(&document, 480, false, |text| Ok(text.chars().count() + 2)).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "奶茶三分糖。\n");
        assert_eq!(chunks[1].content, "晓禾送来银色书签。\n");
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.content.as_str())
                .collect::<String>(),
            document.content
        );
        let atomic = chunk_document(&document, 480, true, |_| panic!("atomic")).unwrap();
        assert_eq!(atomic.len(), 1);
        assert_eq!(atomic[0].content, document.content);
    }

    #[test]
    fn long_unpunctuated_splitting_bounds_repeated_tokenizer_input() {
        let mut previous_bytes = 0;
        for size in [10_000, 20_000] {
            let document = document(&"猫".repeat(size));
            let input_bytes = std::cell::Cell::new(0usize);
            let calls = std::cell::Cell::new(0usize);
            let chunks = chunk_document(&document, 480, false, |text| {
                input_bytes.set(input_bytes.get() + text.len());
                calls.set(calls.get() + 1);
                Ok(text.chars().count() + 2)
            })
            .unwrap();
            assert_eq!(
                chunks
                    .iter()
                    .map(|chunk| chunk.content.as_str())
                    .collect::<String>(),
                document.content
            );
            assert!(
                chunks
                    .iter()
                    .all(|chunk| chunk.content.chars().count() + 2 <= 480)
            );
            assert!(input_bytes.get() < document.content.len() * 20);
            if previous_bytes > 0 {
                assert!(input_bytes.get() < previous_bytes * 23 / 10);
            }
            previous_bytes = input_bytes.get();
            println!(
                "优化后长文本：{size} 字符，{} 个分块，{} 次计数，{} 输入字节",
                chunks.len(),
                calls.get(),
                input_bytes.get()
            );
        }
    }
}
