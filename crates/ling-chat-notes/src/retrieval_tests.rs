use super::*;
use crate::Note;
use crate::chunking::{chunk_document, whole_document_chunk};
use crate::memory::{
    EventStatus, IndexedMemoryChunk, MemoryDocument, MemoryEmbeddingRecord, MemoryScope,
    MemorySource,
};

fn model() -> EmbeddingVersion {
    EmbeddingVersion {
        model: "test".into(),
        dimension: 2,
        preprocessing: "v1".into(),
    }
}

fn note(id: &str, text: &str, tags: &[&str], vector: Option<Vec<f32>>) -> IndexedMemoryDocument {
    let document = MemoryDocument::from_note(
        2,
        &Note {
            id: id.into(),
            content: text.into(),
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
            created_at: "2026-10-18".into(),
        },
    )
    .unwrap();
    let chunk = whole_document_chunk(&document).unwrap();
    let embedding = vector.map(|values| MemoryEmbeddingRecord {
        model: model(),
        chunk_version: chunk.version.clone(),
        values,
    });
    IndexedMemoryDocument {
        document,
        chunking_version: "v1".into(),
        chunks: vec![IndexedMemoryChunk { chunk, embedding }],
    }
}

fn search(
    documents: &[IndexedMemoryDocument],
    query: &str,
    vector: Option<&[f32]>,
) -> HybridSearchResult {
    hybrid_search(
        documents,
        &SearchQuery::new(query, "").unwrap(),
        &model(),
        vector,
        &SearchSettings::default(),
    )
    .unwrap()
}

#[test]
fn independent_routes_and_rrf_use_ranks() {
    let documents = vec![
        note("keyword", "林沐每周四教钢琴。", &[], None),
        note("tag", "每天晚上避免刺激性饮品。", &["睡眠"], None),
        note("semantic", "黄昏后不喝浓茶。", &[], Some(vec![1.0, 0.0])),
    ];
    assert_eq!(
        search(&documents, "林沐", None).hits[0]
            .explanation
            .accepted_by,
        ["keyword"]
    );
    assert_eq!(
        search(&documents, "睡眠", None).hits[0]
            .explanation
            .accepted_by,
        ["tag"]
    );
    let result = search(&documents, "夜里不碰提神饮料", Some(&[1.0, 0.0]));
    assert_eq!(result.hits[0].explanation.accepted_by, ["vector"]);
    assert!((result.hits[0].hit.score - 1.0 / 61.0).abs() < 1e-6);
    let both = note("both", "林沐教钢琴。", &["林沐"], Some(vec![1.0, 0.0]));
    let result = search(&[both], "林沐", Some(&[1.0, 0.0]));
    assert!((result.hits[0].hit.score - 3.0 / 61.0).abs() < 1e-6);
}

#[test]
fn high_fusion_rank_does_not_admit_weak_matches() {
    let documents = vec![note(
        "weak",
        "我喜欢散步。",
        &[],
        Some(vec![0.3, (1.0_f32 - 0.09).sqrt()]),
    )];
    let result = search(&documents, "量子计算纠错", Some(&[1.0, 0.0]));
    assert_eq!(result.diagnostics.vector_candidates, 1);
    assert!(result.hits.is_empty());
    assert_eq!(result.diagnostics.rejected_weak, 1);
}

#[test]
fn foreign_models_invalid_vectors_and_stale_embeddings_fall_back() {
    let good = note("good", "林沐教钢琴。", &[], Some(vec![1.0, 0.0]));
    for mode in 0..4 {
        let mut document = good.clone();
        let embedding = document.chunks[0].embedding.as_mut().unwrap();
        match mode {
            0 => embedding.model.model = "other".into(),
            1 => embedding.values = vec![f32::NAN, 0.0],
            2 => embedding.values = vec![0.0, 0.0],
            _ => embedding.chunk_version = "stale".into(),
        }
        let result = search(&[document], "林沐", Some(&[1.0, 0.0]));
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].explanation.cosine, None);
    }
    let result = search(&[good], "林沐", Some(&[1.0]));
    assert!(result.diagnostics.invalid_query_vector);
    assert_eq!(result.hits.len(), 1);
}

#[test]
fn normalization_preserves_names_dates_english_and_tags() {
    let documents = vec![
        note("name", "林沐教钢琴。", &[], None),
        note("date", "2026年10月1日收到书签。", &[], None),
        note("english", "Project Orion is ready", &[" #PROJECT "], None),
    ];
    assert_eq!(
        search(&documents, "林沐", None).hits[0].hit.document_id,
        "note:2:name"
    );
    assert_eq!(
        search(&documents, "2026年10月1日", None).hits[0]
            .hit
            .document_id,
        "note:2:date"
    );
    let english = search(&documents, "  PROJECT  ", None);
    assert_eq!(english.hits[0].explanation.matched_tags, [" #PROJECT "]);
    assert!(search(&documents, "projectile", None).hits.is_empty());
}

#[test]
fn bounded_context_only_enriches_followups() {
    let context = format!("{}青榆公园银杏", "前".repeat(300));
    let followup = SearchQuery::new("那次下雨了吗？", &context).unwrap();
    assert!(followup.used_recent_context);
    assert_eq!(followup.recent_context.chars().count(), 240);
    assert!(followup.effective.ends_with("那次下雨了吗？"));
    assert!(followup.effective.contains("青榆公园银杏"));
    let ordinary = SearchQuery::new("量子计算纠错", &context).unwrap();
    assert!(!ordinary.used_recent_context);
    assert_eq!(ordinary.effective, ordinary.original);
    assert!(search(&[], "   ", None).hits.is_empty());
    assert!(search(&[], "？！？", None).hits.is_empty());
    assert!(SearchQuery::new(&"中".repeat(4097), "").is_err());
}

#[test]
fn exact_content_duplicates_are_removed_across_sources() {
    let documents = vec![
        note("first", "我在晚上不喝咖啡，因为会睡不着。", &[], None),
        note("second", "我在晚上不喝咖啡，因为会睡不着。", &[], None),
    ];
    let result = search(&documents, "晚上咖啡", None);
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.diagnostics.rejected_duplicate, 1);
}

#[test]
fn shared_sources_keep_different_facts() {
    let mut document = note("multi", "书签是银色的。盆栽在东窗台。", &[], None);
    document.chunks = chunk_document(&document.document, 9, false, |text| {
        Ok(text.chars().count() + 2)
    })
    .unwrap()
    .into_iter()
    .map(|chunk| {
        let embedding = Some(MemoryEmbeddingRecord {
            model: model(),
            chunk_version: chunk.version.clone(),
            values: vec![1.0, 0.0],
        });
        IndexedMemoryChunk { chunk, embedding }
    })
    .collect();
    assert_eq!(document.chunks.len(), 2);
    let result = search(&[document], "历史物品", Some(&[1.0, 0.0]));
    assert_eq!(result.hits.len(), 2);
}

#[test]
fn recent_context_and_reference_budget_are_unicode_aware_and_lossless() {
    let document = note("coffee", "用户晚上不喝咖啡，因为会睡不着。", &[], None);
    let query = SearchQuery::new("咖啡", &document.document.content).unwrap();
    let result = hybrid_search(
        &[document.clone()],
        &query,
        &model(),
        None,
        &SearchSettings::default(),
    )
    .unwrap();
    assert!(result.hits.is_empty());
    assert_eq!(result.diagnostics.rejected_recent, 1);
    let result = search(&[document.clone()], "咖啡", None);
    let size = result.hits[0].reference.chars().count();
    assert_eq!(size, result.diagnostics.used_characters);
    assert!(
        result.hits[0]
            .reference
            .contains(&document.document.content)
    );
    let settings = SearchSettings {
        max_characters: size - 1,
        ..Default::default()
    };
    let result = hybrid_search(
        &[document],
        &SearchQuery::new("咖啡", "").unwrap(),
        &model(),
        None,
        &settings,
    )
    .unwrap();
    assert!(result.hits.is_empty());
    assert_eq!(result.diagnostics.rejected_budget, 1);
}

#[test]
fn limits_filters_and_settings_versions_are_enforced() {
    let documents: Vec<_> = (0..25)
        .map(|index| {
            note(
                &index.to_string(),
                &format!("第{index}个钢琴练习安排"),
                &[],
                None,
            )
        })
        .collect();
    let result = search(&documents, "钢琴", None);
    assert_eq!(result.diagnostics.lexical_candidates, 20);
    assert_eq!(result.hits.len(), 5);
    let settings = SearchSettings {
        kinds: vec![MemoryKind::Diary],
        ..Default::default()
    };
    assert!(
        hybrid_search(
            &documents,
            &SearchQuery::new("钢琴", "").unwrap(),
            &model(),
            None,
            &settings
        )
        .unwrap()
        .hits
        .is_empty()
    );
    assert_ne!(
        settings.version().unwrap(),
        SearchSettings::default().version().unwrap()
    );
    assert!(
        SearchSettings {
            max_hits: 6,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        SearchSettings {
            vector_threshold: f32::NAN,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}

fn raw_note(
    id: &str,
    text: &str,
    kind: MemoryKind,
    sources: Vec<MemorySource>,
) -> IndexedMemoryDocument {
    let mut item = note(id, text, &[], None);
    item.document.id = id.into();
    item.document.kind = kind;
    item.document.scope = MemoryScope::Save { save_id: 101 };
    item.document.sources = sources;
    item.document.version = item.document.content_version().unwrap();
    item.chunks[0].chunk = whole_document_chunk(&item.document).unwrap();
    item
}

#[test]
fn original_and_diary_dedup_requires_the_actual_fact_and_current_sources() {
    let source = MemorySource::Line {
        line_id: 2,
        version: "gift-v1".into(),
    };
    let diary = raw_note(
        "diary",
        "晓禾送给我一枚银色书签，我已经收到了。",
        MemoryKind::Diary,
        vec![source.clone()],
    );
    let original = raw_note(
        "original",
        "用户或场景提供的记录（测试用户）：我喝奶茶只选三分糖。\n用户或场景提供的记录（测试用户）：晓禾送给我一枚银色书签，我已经收到了。",
        MemoryKind::Raw,
        vec![source.clone()],
    );
    let result = search(&[original, diary.clone()], "银色书签", None);
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.diagnostics.rejected_duplicate, 1);
    let different = raw_note(
        "different",
        "用户或场景提供的记录（测试用户）：盆栽放在东窗台，这是另一件事情。",
        MemoryKind::Raw,
        vec![source],
    );
    let result = search(&[different, diary], "东窗台银色书签", None);
    assert_eq!(result.hits.len(), 2);
}

#[test]
fn ascii_words_and_dates_do_not_match_substrings() {
    let documents = vec![
        note("orion", "Orion telescope", &[], None),
        note("date", "2026年10月1日收到礼物", &[], None),
    ];
    assert!(search(&documents, "or", None).hits.is_empty());
    assert!(search(&documents, "26", None).hits.is_empty());
    assert_eq!(search(&documents, "ORION", None).hits.len(), 1);
}

#[test]
fn oversized_atomic_protocol_is_skipped_without_partial_return() {
    let document = note(
        "tool",
        &format!("工具请求：{{完整事务}}\n工具结果：{}", "钢琴".repeat(1000)),
        &[],
        None,
    );
    let result = search(&[document], "钢琴", None);
    assert!(result.hits.is_empty());
    assert_eq!(result.diagnostics.rejected_budget, 1);
}

#[test]
fn dedup_preserves_different_event_statuses() {
    let source = MemorySource::Line {
        line_id: 3,
        version: "trip".into(),
    };
    let mut planned = raw_note(
        "plan",
        "我们去海边旅行。",
        MemoryKind::Diary,
        vec![source.clone()],
    );
    planned.document.event_status = EventStatus::Planned;
    planned.document.version = planned.document.content_version().unwrap();
    planned.chunks[0].chunk = whole_document_chunk(&planned.document).unwrap();
    let mut happened = raw_note("fact", "我们去海边旅行。", MemoryKind::Diary, vec![source]);
    happened.document.event_status = EventStatus::Happened;
    happened.document.version = happened.document.content_version().unwrap();
    happened.chunks[0].chunk = whole_document_chunk(&happened.document).unwrap();
    let result = search(&[planned, happened], "海边", None);
    assert_eq!(result.hits.len(), 2);
    assert_ne!(
        result.hits[0].hit.event_status,
        result.hits[1].hit.event_status
    );
}

#[test]
fn single_character_keywords_and_different_dates_remain_searchable() {
    assert_eq!(
        search(&[note("cat", "我的猫叫团团。", &[], None)], "猫", None)
            .hits
            .len(),
        1
    );
    let source = MemorySource::Line {
        line_id: 2,
        version: "same-turn".into(),
    };
    let first = raw_note(
        "first",
        "2026年10月1日晓禾送给我一枚银色书签。",
        MemoryKind::Raw,
        vec![source.clone()],
    );
    let second = raw_note(
        "second",
        "2026年10月2日晓禾送给我一枚银色书签。",
        MemoryKind::Diary,
        vec![source],
    );
    let result = search(&[first, second], "银色书签", None);
    assert_eq!(result.hits.len(), 2);
}

#[test]
fn explicit_date_cannot_be_overridden_by_vector_similarity() {
    let documents = vec![
        note("gift", "2026年10月1日收到书签。", &[], Some(vec![1.0, 0.0])),
        note(
            "plan",
            "2026年10月25日计划去海边。",
            &[],
            Some(vec![1.0, 0.0]),
        ),
    ];
    let result = search(&documents, "2026-10-01收到了什么", Some(&[1.0, 0.0]));
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].hit.document_id, "note:2:gift");
    assert_eq!(result.diagnostics.rejected_date, 1);
}

#[test]
fn date_filter_precedes_all_route_limits_and_lexical_fallback() {
    for (tagged, semantic, lexical_threshold) in [
        (false, false, 0.30),
        (true, false, 1.0),
        (false, true, 1.0),
        (true, true, 0.30),
    ] {
        let tags: &[&str] = if tagged { &["书签"] } else { &[] };
        let mut documents: Vec<_> = (0..25)
            .map(|index| {
                note(
                    &format!("a-wrong-{index}"),
                    "2026年10月2日书签",
                    tags,
                    semantic.then(|| vec![1.0, 0.0]),
                )
            })
            .collect();
        documents.push(note(
            "zz-target",
            "2026-10-01收到书签。",
            tags,
            semantic.then(|| vec![0.8, 0.6]),
        ));
        let settings = SearchSettings {
            lexical_threshold,
            ..Default::default()
        };
        let result = hybrid_search(
            &documents,
            &SearchQuery::new("2026年10月1日书签", "").unwrap(),
            &model(),
            semantic.then_some([1.0, 0.0].as_slice()),
            &settings,
        )
        .unwrap();
        assert_eq!(result.hits.len(), 1, "tagged={tagged}, semantic={semantic}");
        assert_eq!(result.hits[0].hit.document_id, "note:2:zz-target");
        assert_eq!(result.diagnostics.rejected_date, 25);
        assert_eq!(result.diagnostics.lexical_candidates, 1);
        assert_eq!(result.diagnostics.tag_candidates, usize::from(tagged));
        assert_eq!(result.diagnostics.vector_candidates, usize::from(semantic));
        if lexical_threshold == 1.0 {
            assert!(
                !result.hits[0]
                    .explanation
                    .accepted_by
                    .iter()
                    .any(|route| route == "keyword")
            );
        }
    }
}

#[test]
fn mixed_date_formats_preserve_body_and_query_constraints() {
    let text = "2026-10-01收到书签，2026年10月2日整理书架。";
    assert_eq!(dates(text), BTreeSet::from([(2026, 10, 1), (2026, 10, 2)]));
    assert_eq!(
        dates("2026年10月2日整理书架，2026-10-01收到书签。"),
        dates(text)
    );
    let both = note("both", text, &[], Some(vec![1.0, 0.0]));
    let result = search(&[both.clone()], "2026年10月1日收到什么", Some(&[1.0, 0.0]));
    assert_eq!(result.hits.len(), 1);
    let second_only = note(
        "second-only",
        "2026年10月2日整理书架。",
        &[],
        Some(vec![1.0, 0.0]),
    );
    let result = search(
        &[both, second_only],
        "2026-10-01和2026年10月2日分别做了什么",
        Some(&[1.0, 0.0]),
    );
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].hit.document_id, "note:2:both");
    assert_eq!(result.diagnostics.rejected_date, 1);
}

#[test]
fn dates_require_contiguous_parts_and_valid_calendar_days() {
    for text in [
        "2026随手记年10月1日",
        "2026年10整理月1日",
        "2026年10月1整理日",
        "12026-10-01",
        "2026-010-01",
        "2026-10-001",
        "2026-10-01-99",
        "2026-02-29",
        "2026年4月31日",
        "0000-01-01",
        "2026-13-01",
    ] {
        assert!(dates(text).is_empty(), "{text}");
    }
    assert_eq!(
        dates("2024-02-29与2024年2月29日"),
        BTreeSet::from([(2024, 2, 29)])
    );
    assert_eq!(
        dates("2026随手记年10月1日，后来在2026-10-02整理书架。"),
        BTreeSet::from([(2026, 10, 2)])
    );
}

#[test]
fn dedup_and_recent_context_preserve_date_and_word_boundaries() {
    for (left, right, query) in [
        (
            "我的纪念日是2026-1-01。",
            "我的纪念日是2026-10-1。",
            "纪念日",
        ),
        ("Project AB is ready.", "Project A B is ready.", "project"),
        ("Project AB is ready.", "Project ab is ready.", "project"),
        ("温度是1.5度。", "温度是15度。", "温度"),
    ] {
        let first = note("first", left, &[], None);
        let second = note("second", right, &[], None);
        let result = search(&[first.clone(), second], query, None);
        assert_eq!(result.hits.len(), 2, "{left} / {right}");
        let result = hybrid_search(
            &[first],
            &SearchQuery::new(query, right).unwrap(),
            &model(),
            None,
            &SearchSettings::default(),
        )
        .unwrap();
        assert_eq!(result.hits.len(), 1, "recent context: {left} / {right}");
    }
}

#[test]
fn similar_notes_keep_different_names_colors_and_places() {
    let original = "那天下午我们沿着青榆公园的小路慢慢走到银杏树下躲雨，回家之前晓禾把一枚银色书签送给了我作为生日礼物。";
    for (before, after) in [("银色", "金色"), ("晓禾", "林沐"), ("青榆", "青松")] {
        let documents = [
            note("original", original, &[], None),
            note("changed", &original.replace(before, after), &[], None),
        ];
        let result = search(&documents, "书签", None);
        assert_eq!(result.hits.len(), 2, "{before} / {after}");
        assert_eq!(result.diagnostics.rejected_duplicate, 0);
    }
}

#[test]
fn shared_sources_do_not_erase_different_diary_facts_or_questions() {
    let original = "那天下午我们沿着青榆公园的小路慢慢走到银杏树下躲雨，回家之前晓禾把一枚银色书签送给了我作为生日礼物。";
    let source = MemorySource::Line {
        line_id: 2,
        version: "gift".into(),
    };
    for changed in [
        original.replace("银色", "金色"),
        original.replace("晓禾", "林沐"),
        original.replace('。', "？"),
    ] {
        let documents = [
            raw_note("original", original, MemoryKind::Raw, vec![source.clone()]),
            raw_note("diary", &changed, MemoryKind::Diary, vec![source.clone()]),
        ];
        assert_eq!(search(&documents, "书签", None).hits.len(), 2, "{changed}");
    }
}

#[test]
fn exact_dedup_still_normalizes_spacing_and_speaker_wrappers() {
    let source = MemorySource::Line {
        line_id: 2,
        version: "gift".into(),
    };
    let documents = [
        raw_note(
            "original",
            "用户或场景提供的记录（测试用户）：我收到 Project AB 书签。",
            MemoryKind::Raw,
            vec![source.clone()],
        ),
        raw_note(
            "diary",
            "我收到  Project AB 书签。",
            MemoryKind::Diary,
            vec![source],
        ),
    ];
    let result = search(&documents, "书签", None);
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.diagnostics.rejected_duplicate, 1);
}
