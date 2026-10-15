//! Tool RAG：把下发给模型的工具定义从「权限全集」收窄为「常驻工具 + 检索激活的工具」。
//!
//! 语料是注册表里各工具的「名字 + 描述」，用 [`EmbeddingEngine`] 算句向量做余弦检索。
//! 检索结果进入按角色划分的激活缓存，后续轮次直接下发，避免模型每次都要先搜一遍。
//!
//! 不变量：本模块只负责「缩小」工具集，权限矩阵放行的集合是所有判断的前提，
//! 激活缓存读取时始终与它求交，因此不可能带来权限提升。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{Result, anyhow};

use crate::ai_service::embedding::{EmbeddingEngine, cosine_similarity};
use crate::ai_service::types::ToolDefinition;

/// 检索入口工具名。常驻下发，且不进检索语料（否则会检索到自己）。
pub const TOOL_SEARCH_NAME: &str = "tool_search";

/// 网页搜索工具名：用户要求常驻，不走检索。
pub const WEB_SEARCH_NAME: &str = "web_search";

/// 每次检索取回的候选数。
pub const SEARCH_TOP_K: usize = 5;

/// 单个角色同时激活的工具上限，超出按激活时间淘汰最久未激活的。
pub const ACTIVATION_CAPACITY: usize = 10;

/// 角色名缺失时的激活缓存键（正常流程下角色名总存在，这只是兜底）。
const DEFAULT_ROLE_KEY: &str = "";

/// 常驻工具：永不走检索，直接随每轮请求下发（仍需在权限集内）。
pub fn is_resident(name: &str) -> bool {
    name == TOOL_SEARCH_NAME || name == WEB_SEARCH_NAME
}

pub struct ToolRag {
    engine: Arc<EmbeddingEngine>,
    /// 工具名 -> (语料指纹, 归一化向量)。指纹变化或工具消失时自动失效，
    /// 因此插件热重载/卸载不需要任何主动通知。
    embeddings: RwLock<HashMap<String, (u64, Arc<[f32]>)>>,
    /// 角色名 -> 激活工具。下标 0 为最近激活，用于 LRU 淘汰。
    activated: Mutex<HashMap<String, Vec<String>>>,
}

impl ToolRag {
    pub fn new(engine: Arc<EmbeddingEngine>) -> Self {
        Self {
            engine,
            embeddings: RwLock::new(HashMap::new()),
            activated: Mutex::new(HashMap::new()),
        }
    }

    /// 嵌入引擎是否可用。不可用时调用方应回退到全量工具下发。
    pub fn is_enabled(&self) -> bool {
        self.engine.is_enabled()
    }

    /// 只读嵌入引擎，便于将来记忆检索复用同一个实例。
    pub fn engine(&self) -> &Arc<EmbeddingEngine> {
        &self.engine
    }

    /// 读取某角色已激活的工具（按最近激活在前）。
    pub fn activated_tools(&self, role: Option<&str>) -> Vec<String> {
        let key = role.unwrap_or(DEFAULT_ROLE_KEY);
        self.activated
            .lock()
            .map(|guard| guard.get(key).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    /// 把工具加入激活缓存。
    ///
    /// `names` 按优先级从高到低给出（检索结果即按相似度降序），最重要的那个最终
    /// 排在最前、最不容易被淘汰。超出容量时淘汰队尾（最久未激活的）。
    pub fn touch(&self, role: Option<&str>, names: &[String]) {
        if names.is_empty() {
            return;
        }
        let key = role.unwrap_or(DEFAULT_ROLE_KEY).to_string();
        let Ok(mut guard) = self.activated.lock() else {
            return;
        };
        let list = guard.entry(key).or_default();
        // 逆序插入，使 names[0] 最终位于队首。
        for name in names.iter().rev() {
            if let Some(pos) = list.iter().position(|existing| existing == name) {
                list.remove(pos);
            }
            list.insert(0, name.clone());
        }
        list.truncate(ACTIVATION_CAPACITY);
    }

    /// 检索最匹配的 top_k 个工具，并把它们加入该角色的激活缓存。
    ///
    /// 常驻工具（[`TOOL_SEARCH_NAME`]、[`WEB_SEARCH_NAME`]）不进语料：它们本来就
    /// 一直下发，占用候选位只会挤掉真正需要被发现的工具。
    pub async fn search_and_activate(
        &self,
        role: Option<&str>,
        query: &str,
        candidates: &[ToolDefinition],
        top_k: usize,
    ) -> Result<Vec<(String, f32)>> {
        if !self.is_enabled() {
            return Err(anyhow!("文本嵌入引擎未启用，无法检索工具"));
        }
        let query = query.trim();
        if query.is_empty() {
            return Err(anyhow!("检索关键词为空"));
        }

        let corpus = build_corpus(candidates);
        if corpus.is_empty() {
            return Ok(Vec::new());
        }

        // 1) 找出需要计算向量的语料（缺失或描述变了）。锁只在这一步与写回时持有，
        //    绝不跨 await/阻塞计算持有。
        let pending: Vec<(String, String)> = {
            let guard = self
                .embeddings
                .read()
                .map_err(|e| anyhow!("嵌入缓存锁中毒: {e}"))?;
            corpus
                .iter()
                .filter(|(name, _, fingerprint)| {
                    guard
                        .get(name)
                        .map(|(cached, _)| cached != fingerprint)
                        .unwrap_or(true)
                })
                .map(|(name, text, _)| (name.clone(), text.clone()))
                .collect()
        };

        // 2) 一次性把查询与缺失语料丢进阻塞线程池（ort 推理是同步阻塞的）。
        let engine = self.engine.clone();
        let query_owned = query.to_string();
        let (query_vec, computed) = tokio::task::spawn_blocking(move || {
            let query_vec = engine.embed(&query_owned)?;
            let mut computed = Vec::with_capacity(pending.len());
            for (name, text) in pending {
                let fingerprint = fingerprint_of(&text);
                let vector = engine.embed(&text)?;
                computed.push((name, fingerprint, vector));
            }
            Ok::<_, anyhow::Error>((query_vec, computed))
        })
        .await
        .map_err(|e| anyhow!("嵌入计算任务失败: {e}"))??;

        // 3) 写回缓存并修剪已消失的工具。
        let live: HashSet<&str> = corpus.iter().map(|(name, _, _)| name.as_str()).collect();
        let mut guard = self
            .embeddings
            .write()
            .map_err(|e| anyhow!("嵌入缓存锁中毒: {e}"))?;
        for (name, fingerprint, vector) in computed {
            guard.insert(name, (fingerprint, Arc::from(vector)));
        }
        guard.retain(|name, _| live.contains(name.as_str()));

        // 4) 打分排序取 top_k。
        let mut scored: Vec<(String, f32)> = corpus
            .iter()
            .filter_map(|(name, _, _)| {
                let (_, vector) = guard.get(name)?;
                Some((name.clone(), cosine_similarity(&query_vec, vector)))
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        drop(guard);

        // 5) 命中的工具进入该角色的激活缓存，下一轮即可作为工具定义下发。
        let hit_names: Vec<String> = scored.iter().map(|(name, _)| name.clone()).collect();
        self.touch(role, &hit_names);

        Ok(scored)
    }
}

/// 构建检索语料：`(工具名, 语料文本, 文本指纹)`。
///
/// 常驻工具被排除：它们本来就一直下发，占用候选位只会挤掉真正需要被发现的工具。
fn build_corpus(candidates: &[ToolDefinition]) -> Vec<(String, String, u64)> {
    candidates
        .iter()
        .filter(|def| !is_resident(&def.function.name))
        .map(|def| {
            let text = corpus_text(def);
            let fingerprint = fingerprint_of(&text);
            (def.function.name.clone(), text, fingerprint)
        })
        .collect()
}

/// 语料文本：英文工具名 + 中文描述一起编码，兼顾两种表述。
fn corpus_text(def: &ToolDefinition) -> String {
    format!("{}\n{}", def.function.name, def.function.description.trim())
}

fn fingerprint_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// 本轮实际下发给模型的工具名集合。
///
/// `rag_on == false` 时原样返回权限集，即今天的行为。开启时结果为
/// `{tool_search} ∪ {web_search} ∪ 激活集` 与权限集的交集。
///
/// 不变量：权限集为空（未归属角色组 / 场景组关闭）时不注入检索入口 —— 没有工具
/// 权限的角色不该白拿一个检索入口，结果也保证 ⊆ `base_allowed ∪ {tool_search}`。
pub fn effective_allowed(
    base_allowed: &HashSet<String>,
    rag: &ToolRag,
    role: Option<&str>,
    rag_on: bool,
) -> HashSet<String> {
    if !rag_on {
        return base_allowed.clone();
    }
    let mut effective = HashSet::new();
    if base_allowed.is_empty() {
        return effective;
    }
    effective.insert(TOOL_SEARCH_NAME.to_string());
    if base_allowed.contains(WEB_SEARCH_NAME) {
        effective.insert(WEB_SEARCH_NAME.to_string());
    }
    for name in rag.activated_tools(role) {
        if base_allowed.contains(&name) {
            effective.insert(name);
        }
    }
    effective
}

/// 清单里每个工具用途的截断长度。只做「认得出是什么」用，完整说明留给检索结果。
const MENU_PURPOSE_MAX_CHARS: usize = 36;

/// 没有可检索工具时的兜底描述。
const SEARCH_DESCRIPTION_FALLBACK: &str =
    "按需检索工具。当前没有可检索的工具，请如实告诉用户你做不到。";

/// 把 `tool_search` 的描述换成「当前可检索到的工具清单」。
///
/// 只给名字 + 一句用途，不给参数：模型必须先知道「有这些能力存在」才可能想到去检索，
/// 否则它会直接回答做不到；而具体用法仍由检索结果提供，token 收益得以保留。
pub fn apply_dynamic_search_description(
    definitions: &mut [ToolDefinition],
    available: &[ToolDefinition],
) {
    let Some(target) = definitions
        .iter_mut()
        .find(|def| def.function.name == TOOL_SEARCH_NAME)
    else {
        return;
    };
    target.function.description = search_description(available);
}

/// 渲染 `tool_search` 的动态描述。`available` 可以是权限集的全量定义，
/// 常驻工具（网页搜索等）会被自动跳过 —— 它们本来就一直下发，不必再列一遍。
pub fn search_description(available: &[ToolDefinition]) -> String {
    let mut menu = String::new();
    let mut count = 0usize;
    for def in available {
        if is_resident(&def.function.name) {
            continue;
        }
        let purpose = short_purpose(&def.function.description);
        if purpose.is_empty() {
            menu.push_str(&format!("- {}\n", def.function.name));
        } else {
            menu.push_str(&format!("- {}：{}\n", def.function.name, purpose));
        }
        count += 1;
    }

    if count == 0 {
        return SEARCH_DESCRIPTION_FALLBACK.to_string();
    }

    format!(
        "按需检索工具。下面是当前可用的工具清单（共 {count} 个，只列名字与用途，\
         完整参数需要检索后取得）：\n\
         {menu}\n\
         当用户要求你做某件事时，先从这份清单判断有没有合适的工具：有就用一句自然语言\
         描述你想完成的事调用本工具，它会返回匹配工具的名称与完整说明（含参数），\
         拿到之后再调用对应工具完成操作；若清单与检索结果里都没有合适的工具，\
         如实说明自己做不到，不要编造能力，也不要假装已经完成。"
    )
}

/// 取描述里第一句作为清单用途：优先到第一个句号为止，过长则按长度截断。
///
/// 截断时尽量切在逗号处（且保留长度不少于上限的一半），避免把一句话从词中间劈开、
/// 留下「返回 文件:行号: 内容…」这种读不出意思的尾巴。
fn short_purpose(description: &str) -> String {
    let first_line = description.lines().next().unwrap_or("").trim();
    let sentence = first_line
        .split('。')
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .unwrap_or(first_line);

    let chars: Vec<char> = sentence.chars().collect();
    if chars.len() <= MENU_PURPOSE_MAX_CHARS {
        return sentence.to_string();
    }

    let min_keep = MENU_PURPOSE_MAX_CHARS / 2;
    let clause_break = chars[..MENU_PURPOSE_MAX_CHARS]
        .iter()
        .rposition(|c| *c == '，')
        .filter(|index| *index >= min_keep);
    match clause_break {
        Some(index) => chars[..index].iter().collect(),
        None => {
            let mut truncated: String = chars[..MENU_PURPOSE_MAX_CHARS].iter().collect();
            truncated.push('…');
            truncated
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use serde_json::json;

    use super::{
        ACTIVATION_CAPACITY, TOOL_SEARCH_NAME, ToolRag, WEB_SEARCH_NAME, effective_allowed,
        is_resident,
    };
    use crate::ai_service::embedding::EmbeddingEngine;
    use crate::ai_service::types::ToolDefinition;

    /// 禁用态引擎：LRU 与权限求交的逻辑不依赖真实模型。
    fn disabled_rag() -> ToolRag {
        let engine = EmbeddingEngine::load(&PathBuf::from("__不存在的模型目录__"));
        assert!(!engine.is_enabled(), "该路径不应被识别为可用模型");
        ToolRag::new(engine)
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn definition(name: &str, description: &str) -> ToolDefinition {
        ToolDefinition::new(name, description, json!({"type": "object"}))
    }

    #[test]
    fn resident_tools_are_recognised() {
        assert!(is_resident(TOOL_SEARCH_NAME));
        assert!(is_resident(WEB_SEARCH_NAME));
        assert!(!is_resident("scene_switch"));
    }

    #[test]
    fn activation_keeps_most_recent_first() {
        let rag = disabled_rag();
        rag.touch(Some("雪之下"), &names(&["a", "b"]));
        rag.touch(Some("雪之下"), &["c".to_string()]);
        assert_eq!(rag.activated_tools(Some("雪之下")), names(&["c", "a", "b"]));
    }

    #[test]
    fn activation_dedupes_and_promotes_on_reuse() {
        let rag = disabled_rag();
        rag.touch(Some("r"), &names(&["a", "b", "c"]));
        rag.touch(Some("r"), &["c".to_string()]);
        assert_eq!(rag.activated_tools(Some("r")), names(&["c", "a", "b"]));
    }

    #[test]
    fn activation_evicts_least_recently_activated() {
        let rag = disabled_rag();
        // 按 t0..t9 的优先级依次激活：队首最不易淘汰，队尾 t9 最先出局。
        let filled: Vec<String> = (0..ACTIVATION_CAPACITY).map(|i| format!("t{i}")).collect();
        rag.touch(Some("r"), &filled);
        assert_eq!(rag.activated_tools(Some("r")).len(), ACTIVATION_CAPACITY);

        rag.touch(Some("r"), &["new".to_string()]);
        let after = rag.activated_tools(Some("r"));
        assert_eq!(after.len(), ACTIVATION_CAPACITY);
        assert_eq!(after[0], "new");
        assert!(!after.contains(&"t9".to_string()), "队尾应被淘汰");
        assert!(after.contains(&"t0".to_string()), "队首应保留");
    }

    #[test]
    fn activation_is_scoped_per_role() {
        let rag = disabled_rag();
        rag.touch(Some("甲"), &names(&["a"]));
        rag.touch(Some("乙"), &names(&["b"]));
        assert_eq!(rag.activated_tools(Some("甲")), names(&["a"]));
        assert_eq!(rag.activated_tools(Some("乙")), names(&["b"]));
        assert!(rag.activated_tools(Some("丙")).is_empty());
    }

    #[test]
    fn rag_off_returns_permission_set_unchanged() {
        let rag = disabled_rag();
        rag.touch(Some("r"), &names(&["scene_switch"]));
        let base: HashSet<String> = names(&["scene_switch", "current_status"])
            .into_iter()
            .collect();
        let effective = effective_allowed(&base, &rag, Some("r"), false);
        assert_eq!(effective, base);
    }

    #[test]
    fn empty_permission_set_gets_no_search_entry() {
        let rag = disabled_rag();
        let effective = effective_allowed(&HashSet::new(), &rag, Some("r"), true);
        assert!(effective.is_empty(), "无工具权限不该拿到检索入口");
    }

    #[test]
    fn effective_set_stays_within_permissions() {
        let rag = disabled_rag();
        rag.touch(Some("r"), &names(&["scene_switch", "越权工具"]));
        let base: HashSet<String> = names(&["scene_switch", "current_status"])
            .into_iter()
            .collect();
        let effective = effective_allowed(&base, &rag, Some("r"), true);

        // 检索入口被注入，但激活集里的越权项必须被剔除。
        assert!(effective.contains(TOOL_SEARCH_NAME));
        assert!(effective.contains("scene_switch"));
        assert!(!effective.contains("越权工具"), "激活集不得越过权限矩阵");
        for name in &effective {
            assert!(
                base.contains(name) || name == TOOL_SEARCH_NAME,
                "越权项: {name}"
            );
        }
    }

    #[test]
    fn web_search_only_resident_when_permitted() {
        let rag = disabled_rag();
        let with: HashSet<String> = names(&[WEB_SEARCH_NAME]).into_iter().collect();
        assert!(effective_allowed(&with, &rag, Some("r"), true).contains(WEB_SEARCH_NAME));

        let without: HashSet<String> = names(&["current_status"]).into_iter().collect();
        assert!(
            !effective_allowed(&without, &rag, Some("r"), true).contains(WEB_SEARCH_NAME),
            "用户关掉网页搜索时不该被常驻强行放行"
        );
    }

    #[test]
    fn corpus_excludes_resident_tools() {
        let candidates = vec![
            definition(TOOL_SEARCH_NAME, "检索工具"),
            definition(WEB_SEARCH_NAME, "联网搜索"),
            definition("scene_switch", "切换场景"),
        ];
        let corpus = super::build_corpus(&candidates);
        let names: Vec<&str> = corpus.iter().map(|(name, _, _)| name.as_str()).collect();
        assert_eq!(names, vec!["scene_switch"]);
        assert_eq!(corpus[0].1, "scene_switch\n切换场景");
    }

    #[test]
    fn search_description_lists_names_with_short_purpose() {
        let available = vec![
            definition(TOOL_SEARCH_NAME, "检索工具"),
            definition(WEB_SEARCH_NAME, "联网搜索网页，获取实时信息。"),
            definition("scene_switch", "切换到指定场景，改变当前所处的环境。"),
            definition("get_current_time", "获取当前的日期与时间。"),
        ];
        let text = super::search_description(&available);

        // 常驻工具不必再列一遍；其余工具要出现名字与用途。
        assert!(!text.contains(WEB_SEARCH_NAME), "常驻工具不该出现在清单里");
        assert!(!text.contains(TOOL_SEARCH_NAME));
        assert!(text.contains("- scene_switch：切换到指定场景，改变当前所处的环境"));
        assert!(text.contains("get_current_time：获取当前的日期与时间"));
        assert!(text.contains("共 2 个"), "应给出清单数量作为能力线索");
        // 具体参数不在清单里，留给检索结果。
        assert!(text.contains("完整参数需要检索后取得"));
    }

    #[test]
    fn search_description_cuts_long_purpose_at_clause_boundary() {
        let available = vec![definition(
            "query_weather",
            "查询指定城市的实时天气情况，包含温度湿度风力等多项指标，后面这句应当被丢弃。",
        )];
        let text = super::search_description(&available);
        assert!(
            text.contains(
                "- query_weather：查询指定城市的实时天气情况，包含温度湿度风力等多项指标"
            ),
            "应切在逗号处: {text}"
        );
        assert!(!text.contains("后面这句应当被丢弃"));
        assert!(!text.contains('…'), "切在子句边界时不该再补省略号");
    }

    #[test]
    fn search_description_marks_hard_cut_without_clause_break() {
        // 整句没有逗号可切，只能硬截断并补省略号。
        let available = vec![definition(
            "search_files",
            "按文件名通配符匹配在目录中递归查找文件并返回匹配到的相对路径列表信息内容详情说明尾巴",
        )];
        let text = super::search_description(&available);
        assert!(text.contains('…'), "无子句边界时应硬截断: {text}");
        assert!(!text.contains("详情说明尾巴"));
    }

    #[test]
    fn search_description_skips_later_lines() {
        let available = vec![definition(
            "x",
            "第一行用途。\n\n⚠ 第二行是注意事项，不该进清单。",
        )];
        let text = super::search_description(&available);
        assert!(text.contains("- x：第一行用途"));
        assert!(!text.contains("注意事项"));
    }

    #[test]
    fn search_description_without_searchable_tools_says_so() {
        let available = vec![definition(TOOL_SEARCH_NAME, "检索工具")];
        let text = super::search_description(&available);
        assert!(text.contains("没有可检索的工具"), "实际: {text}");
    }

    #[test]
    fn apply_dynamic_search_description_only_touches_tool_search() {
        let mut definitions = vec![
            definition(TOOL_SEARCH_NAME, "原始描述"),
            definition("scene_switch", "切换场景"),
        ];
        let available = vec![
            definition(TOOL_SEARCH_NAME, "原始描述"),
            definition("scene_switch", "切换到指定场景，改变当前所处的环境。"),
        ];
        super::apply_dynamic_search_description(&mut definitions, &available);

        assert!(definitions[0].function.description.contains("scene_switch"));
        assert_eq!(
            definitions[1].function.description, "切换场景",
            "其他工具不应被改动"
        );
    }
}
