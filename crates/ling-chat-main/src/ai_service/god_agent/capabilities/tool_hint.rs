//! 工具推荐：每轮用户发言前，把「这一轮可能用得上」的工具点出来。
//!
//! 面向的是「模型看得到工具、却想不起来用」这个常见毛病：Tool RAG 把工具收窄之后，
//! 不在本轮定义里的工具只能靠模型自己想起来去 `tool_search`，而多数时候它不会。
//!
//! 两条路（`ToolRecommendMode` 二选一，也可以都关掉）：
//! - `Agent`：上帝 Agent 看一遍最近的对话，按语义挑；
//! - `Rag`：拿玩家这一句话静默检索一次工具，分数过阈值的才留下。
//!
//! 两者都只产出一条**临时 system 提示**（只搭本轮请求，不写台词表、不进记忆），外加
//! 几个要并进本轮工具集的名字。推荐不构成授权：命中的名字在调用方
//! （`tool_loop::activate_recommended`）还要过一遍权限集才会真正下发。

use anyhow::Result;
use serde_json::Value;
use tauri::AppHandle;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::tools::agent::AgentTask;
use crate::ai_service::tools::god_agent::RECOMMEND_TOOLS;
use crate::ai_service::tools::tool_rag::{AUTO_RECOMMEND_MIN_SCORE, AUTO_RECOMMEND_TOP_K, ToolRag};
use crate::ai_service::types::{GameLine, ToolDefinition};

use crate::ai_service::god_agent::core::{GodAgentCore, tail_lines};

/// 最多点几件。再多就等于把工具清单原样塞回去，反而没有重点。
pub const MAX_RECOMMENDATIONS: usize = 5;

/// 提示里每件工具用途的截断长度：完整说明在工具定义里，这里只做「认得出」。
const PURPOSE_MAX_CHARS: usize = 40;

/// 推荐任务的输入视图（锁内取台词，锁外拼工具清单）。
pub struct ToolHintView {
    /// 最近若干条台词（由旧到新）。
    pub lines: Vec<GameLine>,
    /// (工具名, 用途)，已按权限过滤、且不含 tool_search 这类元工具。
    pub tools: Vec<(String, String)>,
}

impl ToolHintView {
    /// 锁内快照最近台词。工具清单依赖当前角色权限，由调用方算好再 [`with_tools`] 挂上。
    pub fn capture(gs: &GameStatus, window: usize) -> Self {
        Self {
            lines: tail_lines(gs, window),
            tools: Vec::new(),
        }
    }

    pub fn with_tools(mut self, tools: Vec<(String, String)>) -> Self {
        self.tools = tools;
        self
    }
}

/// 推荐结果：可以直接拼进请求的那句提示，外加要并进本轮工具集的名字。
pub struct ToolHint {
    pub text: String,
    pub names: Vec<String>,
}

/// 跑一次推荐。没有推荐（或模型没调用工具）时返回 `None`。
pub async fn recommend(
    core: &GodAgentCore,
    view: &ToolHintView,
    app: &AppHandle,
) -> Result<Option<ToolHint>> {
    if view.tools.is_empty() {
        return Ok(None);
    }
    let output = core.run_task(&build_task(view), app).await?;
    let Some(call) = output
        .calls
        .iter()
        .find(|call| call.name == RECOMMEND_TOOLS)
    else {
        return Ok(None);
    };
    if !call.ok {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(&call.result).unwrap_or(Value::Null);
    let picked: Vec<String> = value
        .get("names")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(render_hint(view, &picked))
}

/// RAG 推荐：拿玩家这一句话静默检索一次，分数过阈值的工具才推荐。
///
/// 与上帝 Agent 那条路的区别只在「谁来决策」：这里不额外调用模型，只做一次本地向量
/// 检索；命中的工具照常进入该角色的激活缓存（与 `tool_search` 共用同一套 LRU）。
pub async fn recommend_by_rag(
    rag: &ToolRag,
    role: Option<&str>,
    query: &str,
    candidates: &[ToolDefinition],
) -> Result<Option<ToolHint>> {
    let hits = rag
        .search_and_activate(
            role,
            query,
            candidates,
            AUTO_RECOMMEND_TOP_K,
            AUTO_RECOMMEND_MIN_SCORE,
        )
        .await?;
    Ok(render_rag_hint(candidates, &hits))
}

/// 把命中的工具渲染成临时提示。命中为空（全都没过阈值）时不加任何提示。
fn render_rag_hint(candidates: &[ToolDefinition], hits: &[(String, f32)]) -> Option<ToolHint> {
    let mut names = Vec::new();
    let mut lines = Vec::new();
    for (name, _) in hits.iter().take(AUTO_RECOMMEND_TOP_K) {
        let Some(def) = candidates.iter().find(|def| &def.function.name == name) else {
            continue;
        };
        names.push(name.clone());
        lines.push(format!(
            "- {}：{}",
            name,
            truncate(&def.function.description)
        ));
    }
    if names.is_empty() {
        return None;
    }
    Some(ToolHint {
        text: format!(
            "【本轮推荐工具】玩家这句话可能用得上这几件：\n{}\n需要就直接调用，定义已经随本次请求提供。",
            lines.join("\n")
        ),
        names,
    })
}

/// 把挑中的名字对回工具清单，拼成提示。清单之外的名字丢弃（模型会编）。
fn render_hint(view: &ToolHintView, picked: &[String]) -> Option<ToolHint> {
    let mut names = Vec::new();
    let mut lines = Vec::new();
    for name in picked {
        if names.len() >= MAX_RECOMMENDATIONS {
            break;
        }
        if names.iter().any(|existing: &String| existing == name) {
            continue;
        }
        let Some((_, purpose)) = view.tools.iter().find(|(tool, _)| tool == name) else {
            continue;
        };
        names.push(name.clone());
        lines.push(format!("- {}：{}", name, truncate(purpose)));
    }
    if names.is_empty() {
        return None;
    }
    Some(ToolHint {
        text: format!(
            "【本轮推荐工具】按最近对话，这几件可能用得上：\n{}\n需要就直接调用，定义已经随本次请求提供。",
            lines.join("\n")
        ),
        names,
    })
}

fn truncate(text: &str) -> String {
    // 只取第一行：描述里常带换行与注意事项，带进提示会破坏「一行一件」的格式。
    let cleaned = text.lines().next().unwrap_or("").trim();
    if cleaned.chars().count() <= PURPOSE_MAX_CHARS {
        return cleaned.to_string();
    }
    let head: String = cleaned.chars().take(PURPOSE_MAX_CHARS).collect();
    format!("{head}…")
}

// ============================================================
// 请求构建
// ============================================================

fn build_task(view: &ToolHintView) -> AgentTask {
    let mut dialog = String::from("【最近对话（由旧到新）】\n");
    if view.lines.is_empty() {
        dialog.push_str("（无对话记录）\n");
    } else {
        for line in &view.lines {
            let name = line.base.display_name.as_deref().unwrap_or("未知");
            dialog.push_str(&format!("{}: {}\n", name, line.base.content));
        }
    }

    let mut menu = String::from("【本次可用的全部工具】\n");
    for (name, purpose) in &view.tools {
        menu.push_str(&format!("- {}：{}\n", name, purpose.trim()));
    }

    // 与发言者选择同一条约束：system 只放任务说明，数据放 user（单发 system 时
    // Codex 的 Responses API 会转出空 input 被拒）。
    AgentTask {
        system_prompt:
            "你是一个工具推荐助手。根据最近的对话，判断对话模型接下来可能用得上哪些工具。\
                       只从给出的工具清单里挑，最多 5 个；都不相关就把 names 留空。\
                       不要推荐清单以外的名字，也不要把「可能有用」的工具全列一遍。"
                .to_string(),
        payload: format!("{}\n{}", dialog, menu),
        tool_names: vec![RECOMMEND_TOOLS.to_string()],
        max_rounds: 1,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{render_rag_hint, truncate};
    use crate::ai_service::types::ToolDefinition;

    fn definition(name: &str, description: &str) -> ToolDefinition {
        ToolDefinition::new(name, description, serde_json::json!({"type": "object"}))
    }

    #[test]
    fn rag_hint_uses_first_line_and_drops_unknown_names() {
        let candidates = vec![
            definition(
                "maid_see_screen",
                "看一眼玩家此刻屏幕上的画面。\n⚠ 第二行是注意事项，不该进提示。",
            ),
            definition("schedule_add_todo", "添加一条待办事项。"),
        ];
        let hits = vec![
            ("maid_see_screen".to_string(), 0.81),
            ("模型编出来的工具".to_string(), 0.79),
        ];
        let hint = render_rag_hint(&candidates, &hits).expect("有命中就该有提示");

        assert_eq!(hint.names, vec!["maid_see_screen"]);
        assert!(
            hint.text
                .contains("- maid_see_screen：看一眼玩家此刻屏幕上的画面。")
        );
        assert!(!hint.text.contains("注意事项"), "用途只取第一行");
        assert!(
            !hint.text.contains("模型编出来的工具"),
            "清单外的名字要丢弃"
        );
    }

    #[test]
    fn rag_hint_without_hits_adds_nothing() {
        let candidates = vec![definition("maid_see_screen", "看图")];
        assert!(render_rag_hint(&candidates, &[]).is_none());
    }

    #[test]
    fn truncate_takes_first_line_only() {
        assert_eq!(truncate("第一行\n第二行"), "第一行");
        assert_eq!(truncate("  "), "");
    }

    // ============================================================
    // 阈值校准（需真模型，平时忽略）
    // ============================================================

    fn repo_path(rel: &str) -> PathBuf {
        // 本 crate 位于 <repo>/crates/ling-chat-main
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(rel)
    }

    /// 校准语料：内置工具用真实定义，女仆插件工具照 manifest 原文补上。
    fn calibration_corpus() -> Vec<ToolDefinition> {
        use crate::ai_service::tools::clock::CurrentTimeTool;
        use crate::ai_service::tools::executor::Tool;
        use crate::ai_service::tools::{character, memory, scene, schedule, status, weather};

        let mut defs = vec![
            CurrentTimeTool.definition(),
            schedule::GetAllSchedule.definition(),
            schedule::AddTodo.definition(),
            schedule::UpdateTodo.definition(),
            schedule::DeleteTodo.definition(),
            memory::GetCurrentMemory.definition(),
            memory::GetNotes::new("__test__").definition(),
            memory::AddNote.definition(),
            memory::UpdateNote.definition(),
            memory::DeleteNote.definition(),
            status::CurrentStatus.definition(),
            status::SceneStatus.definition(),
            scene::SceneList.definition(),
            weather::WeatherTool.definition(),
            character::CharacterList.definition(),
            character::CharacterSwitch.definition(),
            character::CharacterGetClothes.definition(),
            character::CharacterSetClothes.definition(),
        ];
        // scene_switch 的定义会去读场景目录（测试里没有数据目录），按原文手写。
        defs.push(definition(
            "scene_switch",
            "切换到指定场景。当前可用场景：占卜摊、教室、公园。当用户提到自己所在的地点（如「我现在在占卜摊」）、或时间氛围发生变化（入夜、清晨、下雨）时，可主动切换到名称最匹配的场景，无需用户指示；找不到明确对应的场景就不要切换。",
        ));
        // 插件工具不在内置注册表里，按 manifest 的原文补进去。
        defs.push(definition(
            "maid_go_to",
            "让女仆走到玩家身边或最近的床边。target 可选 player（玩家，默认）或 bed（最近的床）；走过去要花点时间，到位后会有旁白告诉你结果。",
        ));
        defs.push(definition(
            "maid_look_around",
            "看一眼女仆身边现在有什么。返回她正前方挡着的方块或生物、脚下踩着哪、头顶有没有遮挡、附近一圈的活物、以及玩家在不在她身边。玩家问她「你周围有什么」「你看到什么了」时用它，不要凭印象编。距离只说近/中/远，活物不说方位。",
        ));
        defs.push(definition(
            "maid_see_screen",
            "看一眼玩家此刻屏幕上的画面，也就是你眼前看到的样子。玩家让你看他搭的东西、建的房子或爱心、手里的物品、面前的风景，或者问「你看得到我这里的什么吗」时用它；返回里会附上那张画面，照着画面说，不要凭印象编。",
        ));
        defs.push(definition(
            "maid_lie_down",
            "让女仆走到床上躺下睡觉。她按自己的作息睡，不在睡觉时段会自己起来；附近没有床也会失败，工具会如实告诉你原因。",
        ));
        defs
    }

    /// (查询, 期望被推荐的工具；`None` 表示这句话不该推荐任何工具)
    ///
    /// 后两条是校准时刻的已知边界，不是理想行为：
    /// - 「看看我建的爱心」检索排序是对的（maid_see_screen 排第一），但分数 0.44 压在
    ///   0.5 阈值下——再降阈值就会把「陪我聊聊天吧」（0.455）也放进来；
    /// - 「记得我上次说过什么」被场景工具压过，同样不过阈值。
    /// 这类省略句最终靠模型自己想到去 tool_search 兜底。
    const CALIBRATION_QUERIES: &[(&str, Option<&str>)] = &[
        ("帮我加个日程，明天下午三点开会", Some("schedule_add_todo")),
        ("我今天的待办还有哪些", Some("schedule_get_all")),
        ("看看我建的爱心", None),
        ("你现在身边有什么", Some("maid_look_around")),
        ("现在几点了", Some("get_current_time")),
        ("今天天气怎么样", Some("query_weather")),
        ("换个场景吧", Some("scene_switch")),
        ("你现在穿的是什么衣服", Some("character_get_clothes")),
        ("你还记得我上次说过什么吗", None),
        ("晚上好呀", None),
        ("陪我聊聊天吧", None),
        ("你叫什么名字", None),
        ("嗯嗯", None),
        ("我今天心情不太好", None),
    ];

    /// 校准 `AUTO_RECOMMEND_MIN_SCORE` 的冒烟：真加载模型，跑一遍安静推荐的完整路径。
    ///
    /// 与 embedding_smoke 同理，`ort::init_from` 在同一进程只应初始化一次，单独跑：
    /// `cargo test -p ling-chat-main --lib --ignored threshold_smoke -- --nocapture`
    ///
    /// 断言绑的是当前模型与语料（bge-small-zh + 仅描述语料）。换模型或改工具描述后
    /// 这里会失败，届时重跑一遍看分数、重新校准阈值即可。
    #[tokio::test]
    #[ignore = "需要已下载的嵌入模型与 onnxruntime.dll"]
    async fn threshold_smoke_reports_scores_for_real_queries() {
        use crate::ai_service::tools::tool_rag::{AUTO_RECOMMEND_MIN_SCORE, AUTO_RECOMMEND_TOP_K};

        // 只有 Windows 的 load-dynamic 模式暴露 ort::init_from；其余平台静态链接。
        #[cfg(target_os = "windows")]
        {
            let dll = repo_path("src-tauri/binaries/onnxruntime.dll");
            assert!(dll.exists(), "onnxruntime.dll 不存在: {}", dll.display());
            if let Err(e) = ort::init_from(&dll) {
                eprintln!("ort::init_from 跳过（可能已初始化）: {e}");
            }
        }

        let model_dir = repo_path("data/third_party/embedding_model_bge_small_zh");
        let engine = crate::ai_service::embedding::EmbeddingEngine::load(&model_dir, true);
        assert!(
            engine.is_enabled(),
            "引擎未启用，检查模型目录: {}",
            model_dir.display()
        );

        let rag = crate::ai_service::tools::tool_rag::ToolRag::new(engine);
        let corpus = calibration_corpus();
        for (query, expected) in CALIBRATION_QUERIES {
            let hits = rag
                .search_and_activate(
                    None,
                    query,
                    &corpus,
                    AUTO_RECOMMEND_TOP_K,
                    AUTO_RECOMMEND_MIN_SCORE,
                )
                .await
                .expect("检索失败");
            let rendered: Vec<String> = hits
                .iter()
                .map(|(name, score)| format!("{name}={score:.3}"))
                .collect();
            println!(
                "{query}  ->  {}  (期望: {})",
                rendered.join("  "),
                expected.unwrap_or("无")
            );
            match expected {
                Some(name) => assert!(
                    hits.iter().any(|(hit, _)| hit == name),
                    "{query} 应推荐 {name}"
                ),
                None => assert!(hits.is_empty(), "{query} 不该推荐任何工具"),
            }
        }

        // 「看看我建的爱心」虽然压不过阈值，排序本身应当是对的——改描述后这句仍可能
        // 被漏掉（已知边界），但至少不能排到日程工具后面去。
        let ranked = rag
            .search_and_activate(None, "看看我建的爱心", &corpus, 1, f32::NEG_INFINITY)
            .await
            .expect("检索失败");
        assert_eq!(
            ranked.first().map(|(name, _)| name.as_str()),
            Some("maid_see_screen")
        );
    }
}
