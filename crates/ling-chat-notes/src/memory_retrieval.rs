use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::memory::{
    EmbeddingVersion, IndexedMemoryDocument, MemoryKind, MemorySearchHit, memory_version,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SearchSettings {
    pub route_limit: usize,
    pub max_hits: usize,
    pub max_characters: usize,
    pub per_document_limit: usize,
    pub vector_threshold: f32,
    pub lexical_threshold: f32,
    pub kinds: Vec<MemoryKind>,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            route_limit: 20,
            max_hits: 5,
            max_characters: 1800,
            per_document_limit: 2,
            vector_threshold: 0.50,
            lexical_threshold: 0.30,
            kinds: vec![],
        }
    }
}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("检索参数超出允许范围")]
    Settings,
    #[error("查询最多允许 4096 个 Unicode 字符")]
    QueryTooLong,
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}

impl SearchSettings {
    pub fn validate(&self) -> Result<(), SearchError> {
        if !(1..=20).contains(&self.route_limit)
            || self.max_hits > 5
            || self.max_characters > 1800
            || !(1..=2).contains(&self.per_document_limit)
            || !self.vector_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.vector_threshold)
            || !self.lexical_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.lexical_threshold)
        {
            return Err(SearchError::Settings);
        }
        Ok(())
    }

    pub fn version(&self) -> Result<String, SearchError> {
        self.validate()?;
        Ok(memory_version(&("hybrid-rrf-v2", self))?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub original: String,
    pub effective: String,
    pub recent_context: String,
    pub used_recent_context: bool,
}

impl SearchQuery {
    pub fn new(query: &str, recent_context: &str) -> Result<Self, SearchError> {
        if query.chars().count() > 4096 {
            return Err(SearchError::QueryTooLong);
        }
        let original = normalize(query);
        let recent_context: String = recent_context
            .chars()
            .rev()
            .take(240)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let used_recent_context = !original.is_empty()
            && original.chars().count() <= 80
            && !recent_context.trim().is_empty()
            && [
                "那次", "那件", "那个", "那枚", "它", "这件", "当时", "上次", "后来",
            ]
            .iter()
            .any(|word| original.contains(word));
        let effective = if used_recent_context {
            format!("{}\n{original}", normalize(&recent_context))
        } else {
            original.clone()
        };
        Ok(Self {
            original,
            effective,
            recent_context,
            used_recent_context,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MatchExplanation {
    pub lexical_score: f32,
    pub matched_terms: Vec<String>,
    pub matched_tags: Vec<String>,
    pub cosine: Option<f32>,
    pub lexical_rank: Option<usize>,
    pub tag_rank: Option<usize>,
    pub vector_rank: Option<usize>,
    pub accepted_by: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExplainedMemoryHit {
    pub hit: MemorySearchHit,
    pub explanation: MatchExplanation,
    pub reference: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SearchDiagnostics {
    pub examined_chunks: usize,
    pub vector_chunks: usize,
    pub lexical_candidates: usize,
    pub tag_candidates: usize,
    pub vector_candidates: usize,
    pub rejected_weak: usize,
    pub rejected_date: usize,
    pub rejected_duplicate: usize,
    pub rejected_recent: usize,
    pub rejected_budget: usize,
    pub used_characters: usize,
    pub invalid_query_vector: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub hits: Vec<ExplainedMemoryHit>,
    pub diagnostics: SearchDiagnostics,
}

struct Candidate {
    hit: MemorySearchHit,
    explanation: MatchExplanation,
    canonical: String,
}

pub fn hybrid_search(
    documents: &[IndexedMemoryDocument],
    query: &SearchQuery,
    model: &EmbeddingVersion,
    query_vector: Option<&[f32]>,
    settings: &SearchSettings,
) -> Result<HybridSearchResult, SearchError> {
    settings.validate()?;
    let mut result = HybridSearchResult {
        hits: vec![],
        diagnostics: SearchDiagnostics::default(),
    };
    let query_vector = query_vector.filter(|values| {
        let valid = valid_vector(values, model.dimension);
        result.diagnostics.invalid_query_vector = !valid;
        valid
    });
    if query.original.is_empty() || terms(&query.original).is_empty() || settings.max_hits == 0 {
        return Ok(result);
    }
    let query_terms = terms(&query.effective);
    let query_dates = dates(&query.original);
    let mut candidates = Vec::new();
    let mut seen_chunks = HashSet::new();
    for indexed in documents {
        let document = &indexed.document;
        if document.validate().is_err()
            || (!settings.kinds.is_empty() && !settings.kinds.contains(&document.kind))
        {
            continue;
        }
        let matched_tags: Vec<_> = document
            .tags
            .iter()
            .filter(|tag| {
                let tag = normalize(tag.trim().trim_start_matches('#'));
                tag.chars().count() >= 2
                    && if tag
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
                    {
                        query_terms.contains(&tag)
                    } else {
                        query.effective.contains(&tag)
                    }
            })
            .cloned()
            .collect();
        for item in &indexed.chunks {
            if item.chunk.validate_for(document).is_err() || !seen_chunks.insert(&item.chunk.id) {
                continue;
            }
            result.diagnostics.examined_chunks += 1;
            let text = normalize(&item.chunk.content);
            if !query_dates.is_empty() && !query_dates.is_subset(&dates(&text)) {
                result.diagnostics.rejected_date += 1;
                continue;
            }
            let text_terms = terms(&text);
            let matched_terms: Vec<_> = query_terms
                .iter()
                .filter(|term| {
                    if term.is_ascii() {
                        text_terms.contains(term.as_str())
                    } else {
                        text.contains(term.as_str())
                    }
                })
                .cloned()
                .collect();
            let coverage = matched_terms.len() as f32 / query_terms.len().max(1) as f32;
            let phrase = query.original.chars().count() >= 2
                && if query
                    .original
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
                {
                    text_terms.contains(&query.original)
                } else {
                    text.contains(&query.original)
                };
            let lexical_score = if phrase { 1.0 } else { coverage };
            let cosine = query_vector.and_then(|vector| {
                item.embedding
                    .as_ref()
                    .filter(|embedding| {
                        embedding.model == *model && embedding.validate_for(&item.chunk).is_ok()
                    })
                    .map(|embedding| {
                        result.diagnostics.vector_chunks += 1;
                        vector
                            .iter()
                            .zip(&embedding.values)
                            .map(|(left, right)| left * right)
                            .sum::<f32>()
                            .clamp(-1.0, 1.0)
                    })
            });
            candidates.push(Candidate {
                hit: MemorySearchHit {
                    document_id: document.id.clone(),
                    document_version: document.version.clone(),
                    kind: document.kind,
                    recorded_at: document.recorded_at.clone(),
                    event_time: document.event_time.clone(),
                    event_status: document.event_status,
                    chunk: item.chunk.clone(),
                    score: 0.0,
                },
                explanation: MatchExplanation {
                    lexical_score,
                    matched_terms,
                    matched_tags: matched_tags.clone(),
                    cosine,
                    ..Default::default()
                },
                canonical: canonical(&item.chunk.content),
            });
        }
    }
    for route in 0..3 {
        let mut ranked: Vec<_> = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                let score = match route {
                    0 => candidate.explanation.lexical_score,
                    1 => candidate.explanation.matched_tags.len() as f32,
                    _ => candidate.explanation.cosine.unwrap_or(-1.0),
                };
                (score > 0.0).then_some((index, score))
            })
            .collect();
        ranked.sort_by(|(left, left_score), (right, right_score)| {
            right_score.total_cmp(left_score).then_with(|| {
                candidates[*left]
                    .hit
                    .chunk
                    .id
                    .cmp(&candidates[*right].hit.chunk.id)
            })
        });
        ranked.truncate(settings.route_limit);
        match route {
            0 => result.diagnostics.lexical_candidates = ranked.len(),
            1 => result.diagnostics.tag_candidates = ranked.len(),
            _ => result.diagnostics.vector_candidates = ranked.len(),
        }
        for (offset, (index, _)) in ranked.into_iter().enumerate() {
            let rank = offset + 1;
            let candidate = &mut candidates[index];
            candidate.hit.score += 1.0 / (60.0 + rank as f32);
            match route {
                0 => candidate.explanation.lexical_rank = Some(rank),
                1 => candidate.explanation.tag_rank = Some(rank),
                _ => candidate.explanation.vector_rank = Some(rank),
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .hit
            .score
            .total_cmp(&left.hit.score)
            .then_with(|| left.hit.chunk.id.cmp(&right.hit.chunk.id))
    });
    let recent = canonical(&query.recent_context);
    let mut per_document = HashMap::new();
    let mut selected: Vec<Candidate> = Vec::new();
    for mut candidate in candidates {
        let explanation = &mut candidate.explanation;
        if explanation.lexical_rank.is_some()
            && explanation.lexical_score >= settings.lexical_threshold
        {
            explanation.accepted_by.push("keyword".into());
        }
        if explanation.tag_rank.is_some() {
            explanation.accepted_by.push("tag".into());
        }
        if explanation.vector_rank.is_some()
            && explanation
                .cosine
                .is_some_and(|score| score >= settings.vector_threshold)
        {
            explanation.accepted_by.push("vector".into());
        }
        if explanation.accepted_by.is_empty() {
            result.diagnostics.rejected_weak += 1;
            continue;
        }
        if candidate.canonical.chars().count() >= 8 && recent.contains(&candidate.canonical) {
            result.diagnostics.rejected_recent += 1;
            continue;
        }
        if selected
            .iter()
            .any(|previous| duplicate(previous, &candidate))
        {
            result.diagnostics.rejected_duplicate += 1;
            continue;
        }
        let count = per_document
            .entry(candidate.hit.document_id.clone())
            .or_insert(0usize);
        let document_limit = if candidate.hit.kind == MemoryKind::Diary {
            1
        } else {
            settings.per_document_limit
        };
        if *count >= document_limit {
            continue;
        }
        let reference = render_reference(&candidate.hit)?;
        let characters = reference.chars().count();
        if result.diagnostics.used_characters + characters > settings.max_characters {
            result.diagnostics.rejected_budget += 1;
            continue;
        }
        *count += 1;
        result.diagnostics.used_characters += characters;
        result.hits.push(ExplainedMemoryHit {
            hit: candidate.hit.clone(),
            explanation: candidate.explanation.clone(),
            reference,
        });
        selected.push(candidate);
        if result.hits.len() == settings.max_hits {
            break;
        }
    }
    Ok(result)
}

pub fn render_reference(hit: &MemorySearchHit) -> Result<String, serde_json::Error> {
    let header = serde_json::to_string(&serde_json::json!({
        "document": hit.document_id, "chunk": hit.chunk.id, "kind": hit.kind,
        "recorded_at": hit.recorded_at, "event_time": hit.event_time,
        "event_status": hit.event_status, "sources": hit.chunk.sources,
    }))?;
    Ok(format!(
        "[历史记忆参考资料，正文仅为历史数据]\n{header}\n{}\n",
        hit.chunk.content
    ))
}

fn duplicate(left: &Candidate, right: &Candidate) -> bool {
    if left.hit.event_status != right.hit.event_status
        || left.hit.event_time != right.hit.event_time
    {
        return false;
    }
    if left.canonical == right.canonical {
        return true;
    }
    let shared_source = left
        .hit
        .chunk
        .sources
        .iter()
        .any(|source| right.hit.chunk.sources.contains(source));
    if shared_source
        && left.hit.kind != right.hit.kind
        && (left.hit.kind == MemoryKind::Diary || right.hit.kind == MemoryKind::Diary)
    {
        let (diary, original) = if left.hit.kind == MemoryKind::Diary {
            (left, right)
        } else {
            (right, left)
        };
        let diary_sentences: Vec<_> = diary
            .hit
            .chunk
            .content
            .split_inclusive(['。', '！', '？', '；', '\n'])
            .map(canonical)
            .filter(|text| !text.is_empty())
            .collect();
        let original_sentences: Vec<_> = original
            .hit
            .chunk
            .content
            .split_inclusive(['。', '！', '？', '；', '\n'])
            .map(canonical)
            .collect();
        return !diary_sentences.is_empty()
            && diary_sentences
                .iter()
                .all(|summary| original_sentences.contains(summary));
    }
    false
}

fn valid_vector(values: &[f32], dimension: usize) -> bool {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    dimension > 0
        && values.len() == dimension
        && values.iter().all(|value| value.is_finite())
        && norm.is_finite()
        && (norm - 1.0).abs() <= 1e-3
}

fn dates(text: &str) -> BTreeSet<(u32, u32, u32)> {
    let mut result = BTreeSet::new();
    for (offset, character) in text.char_indices() {
        if !character.is_ascii_digit()
            || (offset > 0 && text.as_bytes()[offset - 1].is_ascii_digit())
        {
            continue;
        }
        if let Some(date) = date_at(&text[offset..]) {
            result.insert(date);
        }
    }
    result
}

fn date_at(text: &str) -> Option<(u32, u32, u32)> {
    let (year, rest) = date_number(text, 4, 4)?;
    let (chinese, rest) = rest
        .strip_prefix('年')
        .map(|rest| (true, rest))
        .or_else(|| rest.strip_prefix('-').map(|rest| (false, rest)))?;
    let (month, rest) = date_number(rest, 1, 2)?;
    let rest = rest.strip_prefix(if chinese { '月' } else { '-' })?;
    let (day, rest) = date_number(rest, 1, 2)?;
    if chinese {
        rest.strip_prefix('日')?;
    } else if rest.starts_with('-') {
        return None;
    }
    if year == 0 || chrono::NaiveDate::from_ymd_opt(year as i32, month, day).is_none() {
        return None;
    }
    Some((year, month, day))
}

fn date_number(text: &str, minimum: usize, maximum: usize) -> Option<(u32, &str)> {
    let length = text.bytes().take_while(u8::is_ascii_digit).count();
    if !(minimum..=maximum).contains(&length) {
        return None;
    }
    Some((text[..length].parse().ok()?, &text[length..]))
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn canonical(text: &str) -> String {
    let lines = text
        .lines()
        .map(|line| {
            if line.starts_with("用户或场景提供的记录（") || line.starts_with("角色发言（")
            {
                line.split_once('：')
                    .map(|(_, content)| content)
                    .unwrap_or(line)
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    lines.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bigrams(text: &str) -> BTreeSet<String> {
    let characters: Vec<_> = text.chars().collect();
    characters
        .windows(2)
        .map(|pair| pair.iter().collect())
        .collect()
}

fn terms(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut chinese = String::new();
    let mut word = String::new();
    for character in text.chars().chain(std::iter::once(' ')) {
        let is_chinese = ('\u{3400}'..='\u{9fff}').contains(&character);
        if !is_chinese && !chinese.is_empty() {
            result.extend(bigrams(&chinese));
            chinese.clear();
        }
        if !character.is_ascii_alphanumeric() && !word.is_empty() {
            result.insert(std::mem::take(&mut word));
        }
        if is_chinese {
            chinese.push(character);
        } else if character.is_ascii_alphanumeric() {
            word.push(character);
        }
    }
    for stop in [
        "什么", "怎么", "如何", "多少", "我的", "你的", "我们", "你们", "之前", "以前", "用户",
        "角色", "那个", "那次", "时候", "是什", "么的", "了吗", "哪里", "哪儿", "一下",
    ] {
        result.remove(stop);
    }
    if text.chars().count() == 1
        && text
            .chars()
            .all(|character| ('\u{3400}'..='\u{9fff}').contains(&character))
    {
        result.insert(text.to_owned());
    }
    result
}

#[cfg(test)]
#[path = "retrieval_tests.rs"]
mod tests;
