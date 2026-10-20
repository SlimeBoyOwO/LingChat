//! 按句子与实际 token 预算分块，保留原文字符和来源。

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
    // 工具调用与返回值作为完整事务保留，超长时由宿主降级为词法检索。
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
            // 换行通常表示话题切换，即使预算还有空余也独立成块。
            if sentence.ends_with('\n') {
                pieces.push(std::mem::take(&mut pending));
            }
            continue;
        }
        if !pending.is_empty() {
            pieces.push(std::mem::take(&mut pending));
        }
        // 长句只在字符边界切开，避免截断中文或表情的 UTF-8 编码。
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
    // 先倍增探测，再二分收窄，减少对整段长文本的重复分词。
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
