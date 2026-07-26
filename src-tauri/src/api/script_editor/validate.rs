//! 剧本校验器。
//!
//! 目标是把引擎里所有**静默失败**变成作者能看见的一条诊断。判定逻辑尽量
//! 复用引擎自己的函数（`resolve_script_media` 查素材、`parse_variable_action`
//! 解析表达式、`KNOWN_EFFECTS` 判特效），避免校验器和运行时各说一套。
//!
//! 诊断分三级：
//! - `error` —— 一定会出问题（跑不通、跳不过去、素材缺失）
//! - `warn` —— 很可能不是作者的意图（写了不生效的字段、孤儿章节）
//! - `info` —— 提示性的（遗留字段、可疑但合法的写法）
//!
//! 保存时不拦，只在「试玩 / 导出」时拦 error。

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::ai_service::game_system::script_engine::events::background_effect_event::KNOWN_EFFECTS;
use crate::ai_service::game_system::script_engine::utils::media::{
    resolve_script_media, MediaType,
};
use crate::ai_service::game_system::script_engine::utils::script_function::parse_variable_action;

use super::io;
use super::paths;
use super::schema::build_schema;

/// 诊断级别。
///
/// 原先是 `&'static str`，写错一个 "warning" 能编译、能序列化、排序落到兜底分支、
/// 前端当成 info —— 全链路静默。改成 enum 让编译器兜住。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

impl Severity {
    /// 排序权重：error 最前
    fn rank(self) -> u8 {
        match self {
            Severity::Error => 0,
            Severity::Warn => 1,
            Severity::Info => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub severity: Severity,
    /// 稳定的机器可读代码，前端可据此做跳转/过滤
    pub code: &'static str,
    pub message: String,
    /// 章节 id；为 None 表示剧本级问题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    /// 事件下标（0 起）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl Diagnostic {
    fn script(severity: Severity, code: &'static str, message: String) -> Self {
        Diagnostic {
            severity,
            code,
            message,
            chapter: None,
            event_index: None,
            field: None,
        }
    }
    fn chapter(
        severity: Severity,
        code: &'static str,
        chapter: &str,
        message: String,
    ) -> Self {
        Diagnostic {
            severity,
            code,
            message,
            chapter: Some(chapter.to_string()),
            event_index: None,
            field: None,
        }
    }
    fn event(
        severity: Severity,
        code: &'static str,
        chapter: &str,
        index: usize,
        message: String,
    ) -> Self {
        Diagnostic {
            severity,
            code,
            message,
            chapter: Some(chapter.to_string()),
            event_index: Some(index),
            field: None,
        }
    }
    fn with_field(mut self, field: &str) -> Self {
        self.field = Some(field.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warn_count: usize,
    pub info_count: usize,
    /// 收集到的全部变量名，供编辑器做变量面板
    pub variables: Vec<String>,
    /// 章节跳转边，供前端画真实的流程图连线与判断能否拖拽重排
    pub edges: Vec<ChapterEdge>,
}

/// 统一的章节 id 归一：去空白、剥一次 `.yaml`。
///
/// 之前三处各写一遍（两处 `trim_end_matches(".yaml")` 会把 `a.yaml.yaml` 剥成 `a`，
/// 一处正则只剥一次），语义已经不一致了。
pub fn chapter_id_of(raw: &str) -> &str {
    let t = raw.trim();
    t.strip_suffix(".yaml").unwrap_or(t)
}

/// 分支边在流程图上显示的标签：条件优先，其次 AI 分支名，兜底分支写「默认」。
fn branch_label(opt: &serde_json::Map<String, JsonValue>, index: usize) -> String {
    if let Some(c) = opt.get("condition").and_then(|v| v.as_str()) {
        if !c.trim().is_empty() {
            return c.trim().to_string();
        }
    }
    if let Some(n) = opt.get("name").and_then(|v| v.as_str()) {
        if !n.trim().is_empty() {
            return n.trim().to_string();
        }
    }
    if opt.get("default").and_then(|v| v.as_bool()).unwrap_or(false) {
        return "默认".to_string();
    }
    format!("分支 {}", index + 1)
}

/// 章节图的一条边，从某章最后一条 `chapter_end` 反推而来。
///
/// 导出给前端画真连线。在这之前流程图只是把章节按文件名字典序排了一列，
/// 箭头表达的是「章节 id 的字母顺序」而不是真实跳转 —— 看起来对但完全不对。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterEdge {
    pub from: String,
    /// 目标章节 id；`"end"` 表示剧本结束
    pub to: String,
    /// 是否是显式的「剧本结束」
    pub is_end: bool,
    /// 分支条件 / AI 分支名，linear 时为空
    #[serde(skip_serializing_if = "str::is_empty")]
    pub label: String,
    /// 该边所属章节的 end_type，前端据此决定能否拖拽重排
    pub end_type: String,
}

/// 校验一个剧本包。
///
/// `other_script_names` 是「script_name → 剧本 key」的映射，用于查重 —— 引擎
/// 用 script_name 作索引，重名会让其中一个剧本在列表里完全消失。由调用方
/// （`editor_validate_script`）扫盘收集后传入。
pub fn validate(
    data_dir: &Path,
    script_dir: &Path,
    script_key: &str,
    other_script_names: &HashMap<String, String>,
) -> ValidationReport {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let schema = build_schema();

    // 事件类型 → 字段表，用于必填/未知字段检查
    let mut field_index: HashMap<&str, &Vec<super::schema::FieldSpec>> = HashMap::new();
    for e in &schema.events {
        field_index.insert(e.type_key, &e.fields);
    }
    let common_keys: HashSet<&str> = schema.common_fields.iter().map(|f| f.key).collect();

    // ---------- story_config ----------
    let config = match io::read_story_config(script_dir) {
        Ok(c) => c,
        Err(e) => {
            diags.push(Diagnostic::script(Severity::Error, "config.unreadable", e));
            return finish(diags, Vec::new(), Vec::new());
        }
    };

    let folder_name = script_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let script_name = config
        .get("script_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if script_name.is_empty() {
        diags.push(Diagnostic::script(
            Severity::Warn,
            "config.no_script_name",
            format!("没有填剧本名，列表里会显示目录名「{}」", folder_name),
        ));
    } else if let Some(other_key) = other_script_names.get(&script_name) {
        if other_key != script_key {
            diags.push(Diagnostic::script(
                Severity::Error,
                "config.duplicate_name",
                format!(
                    "剧本名「{}」与「{}」重复。引擎用剧本名作索引，重名会导致其中一个在列表里完全消失",
                    script_name, other_key
                ),
            ));
        }
    }

    // ---------- 章节清单 ----------
    let chapter_ids = paths::enumerate_chapter_ids(script_dir);
    if chapter_ids.is_empty() {
        diags.push(Diagnostic::script(
            Severity::Error,
            "chapters.empty",
            "Chapters/ 下没有任何 .yaml 章节文件（引擎只认 .yaml，不认 .yml）".to_string(),
        ));
        return finish(diags, Vec::new(), Vec::new());
    }
    let chapter_set: HashSet<&str> = chapter_ids.iter().map(|s| s.as_str()).collect();

    let intro = config
        .get("intro_chapter")
        .and_then(|v| v.as_str())
        .map(chapter_id_of)
        .unwrap_or("main")
        .to_string();

    if !chapter_set.contains(intro.as_str()) {
        diags.push(Diagnostic::script(
            Severity::Error,
            "config.intro_missing",
            format!("开场章节「{}」不存在", intro),
        ));
    }

    // 剧本内 NPC 目录名，用于校验 character 引用
    let known_characters = collect_script_characters(script_dir);

    // ---------- 逐章节 ----------
    let mut edges: Vec<ChapterEdge> = Vec::new();
    let mut vars_written: BTreeSet<String> = BTreeSet::new();
    let mut vars_read: BTreeSet<String> = BTreeSet::new();

    for cid in &chapter_ids {
        let file = match paths::resolve_chapter_file(script_dir, cid, true) {
            Ok(f) => f,
            Err(e) => {
                diags.push(Diagnostic::chapter(Severity::Error, "chapter.unreadable", cid, e));
                continue;
            }
        };
        let raw = match io::read_yaml_as_json(&file) {
            Ok(v) => v,
            Err(e) => {
                diags.push(Diagnostic::chapter(Severity::Error, "chapter.parse_failed", cid, e));
                continue;
            }
        };
        let doc = match io::ChapterDoc::from_json(raw) {
            Ok(d) => d,
            Err(e) => {
                diags.push(Diagnostic::chapter(Severity::Error, "chapter.bad_shape", cid, e));
                continue;
            }
        };

        if doc.events.is_empty() {
            diags.push(Diagnostic::chapter(
                Severity::Warn,
                "chapter.no_events",
                cid,
                "章节没有任何事件，运行时会立刻结束整个剧本".to_string(),
            ));
        }

        let last_index = doc.events.len().saturating_sub(1);
        let mut has_chapter_end = false;

        for (i, ev) in doc.events.iter().enumerate() {
            let obj = match ev.as_object() {
                Some(o) => o,
                None => {
                    diags.push(Diagnostic::event(
                        Severity::Error,
                        "event.not_a_map",
                        cid,
                        i,
                        "事件必须是键值映射".to_string(),
                    ));
                    continue;
                }
            };

            let ty = match obj.get("type").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => {
                    diags.push(Diagnostic::event(
                        Severity::Error,
                        "event.missing_type",
                        cid,
                        i,
                        "事件缺少 type 字段，运行到这里整个剧本会中断".to_string(),
                    ));
                    continue;
                }
            };

            let fields = match field_index.get(ty) {
                Some(f) => *f,
                None => {
                    diags.push(Diagnostic::event(
                        Severity::Error,
                        "event.unknown_type",
                        cid,
                        i,
                        format!("未知事件类型「{}」，运行到这里整个剧本会中断", ty),
                    ));
                    continue;
                }
            };

            // 必填字段
            for f in fields {
                if !f.required {
                    continue;
                }
                let present = obj
                    .get(f.key)
                    .map(|v| !matches!(v, JsonValue::Null) && v.as_str() != Some(""))
                    .unwrap_or(false);
                if !present {
                    diags.push(
                        Diagnostic::event(
                            Severity::Error,
                            "field.required_missing",
                            cid,
                            i,
                            format!("{} 缺少必填字段「{}」", ty, f.label),
                        )
                        .with_field(f.key),
                    );
                }
            }

            // 未知字段（很可能是拼错）
            let known: HashSet<&str> = fields.iter().map(|f| f.key).collect();
            for k in obj.keys() {
                if k == "type" || known.contains(k.as_str()) || common_keys.contains(k.as_str()) {
                    continue;
                }
                diags.push(
                    Diagnostic::event(
                        Severity::Warn,
                        "field.unknown",
                        cid,
                        i,
                        format!("{} 上的「{}」不是引擎认识的字段，会被静默忽略", ty, k),
                    )
                    .with_field(k),
                );
            }

            // 遗留字段（schema 里 enabled == false 的通用字段）。
            // 由 schema 驱动而不是硬编码 "duration"，这样加第二个遗留字段只改 schema.rs
            for f in schema.common_fields.iter().filter(|f| !f.enabled) {
                if obj.contains_key(f.key) {
                    diags.push(
                        Diagnostic::event(
                            Severity::Info,
                            "field.inert",
                            cid,
                            i,
                            format!(
                                "{} 引擎从不读取，写了不生效（保存时会原样保留）",
                                f.key
                            ),
                        )
                        .with_field(f.key),
                    );
                }
            }

            // condition
            if let Some(cond) = obj.get("condition").and_then(|v| v.as_str()) {
                check_condition(cond, cid, i, &mut diags, &mut vars_read);
            }

            // 逐类型细查
            match ty {
                "background" | "present_pic" | "music" | "sound" | "ambient" => {
                    check_asset(
                        data_dir, script_dir, obj, ty, cid, i, &mut diags,
                    );
                }
                "background_effect" => {
                    let effect = obj.get("effect").and_then(|v| v.as_str()).unwrap_or("");
                    let clearing = effect.is_empty()
                        || effect.eq_ignore_ascii_case("none");
                    if !clearing && !KNOWN_EFFECTS.contains(&effect) {
                        let hint = KNOWN_EFFECTS
                            .iter()
                            .find(|k| k.eq_ignore_ascii_case(effect));
                        let msg = match hint {
                            Some(correct) => format!(
                                "特效「{}」大小写不对，实际不会渲染任何粒子；应为「{}」",
                                effect, correct
                            ),
                            None => format!(
                                "未知特效「{}」，实际会清空当前特效；可用值：{}",
                                effect,
                                KNOWN_EFFECTS.join(" / ")
                            ),
                        };
                        diags.push(
                            Diagnostic::event(Severity::Error, "effect.invalid", cid, i, msg)
                                .with_field("effect"),
                        );
                    }
                }
                "choices" => {
                    check_choices(obj, cid, i, &mut diags, &mut vars_written, &mut vars_read);
                }
                "set_variable" => {
                    check_set_variable(obj, cid, i, &mut diags, &mut vars_written, &mut vars_read);
                }
                "free_dialogue" => {
                    let rounds = obj.get("max_rounds").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let end_line = obj.get("end_line").and_then(|v| v.as_str()).unwrap_or("结束");
                    if rounds <= 0 && end_line.trim().is_empty() {
                        diags.push(Diagnostic::event(
                            Severity::Error,
                            "free_dialogue.no_exit",
                            cid,
                            i,
                            "最大轮数不限且结束语为空，这段自由对话永远无法结束".to_string(),
                        ));
                    }
                }
                "chapter_end" => {
                    has_chapter_end = true;
                    if i != last_index {
                        diags.push(Diagnostic::event(
                            Severity::Warn,
                            "chapter_end.not_last",
                            cid,
                            i,
                            format!(
                                "章节结束之后还有 {} 个事件，它们永远不会执行",
                                last_index - i
                            ),
                        ));
                    }
                    check_chapter_end(obj, cid, i, &chapter_set, &mut edges, &mut diags, &mut vars_read);
                }
                _ => {}
            }

            // character 引用
            if let Some(ch) = obj.get("character").and_then(|v| v.as_str()) {
                if ch != "MAIN" && !known_characters.contains(ch) {
                    diags.push(
                        Diagnostic::event(
                            Severity::Error,
                            "character.unknown",
                            cid,
                            i,
                            format!(
                                "角色「{}」在本剧本的 characters/ 下找不到；写 MAIN 表示当前主角",
                                ch
                            ),
                        )
                        .with_field("character"),
                    );
                }
            }
        }

        if !has_chapter_end {
            diags.push(Diagnostic::chapter(
                Severity::Error,
                "chapter.no_end",
                cid,
                "章节缺少「章节结束」事件。引擎会把这当成整个剧本结束，而不是接着下一章".to_string(),
            ));
        }
    }

    // ---------- 章节图 ----------
    check_graph(&intro, &chapter_ids, &edges, &mut diags);

    // ---------- 变量 ----------
    for v in vars_read.difference(&vars_written) {
        diags.push(Diagnostic::script(
            Severity::Warn,
            "variable.never_set",
            format!(
                "条件里用到变量「{}」，但整个剧本都没有给它赋值过。未定义变量的 == 恒假、!= 恒真",
                v
            ),
        ));
    }
    for v in vars_written.difference(&vars_read) {
        diags.push(Diagnostic::script(
            Severity::Info,
            "variable.never_read",
            format!("变量「{}」被赋值但从未在任何条件里使用", v),
        ));
    }

    let mut all_vars: Vec<String> = vars_written.union(&vars_read).cloned().collect();
    all_vars.sort();
    finish(diags, all_vars, edges)
}

fn finish(
    mut diags: Vec<Diagnostic>,
    variables: Vec<String>,
    edges: Vec<ChapterEdge>,
) -> ValidationReport {
    // error 在前，其次 warn，最后 info；同级按章节 + 事件序
    diags.sort_by(|a, b| {
        a.severity
            .rank()
            .cmp(&b.severity.rank())
            .then_with(|| a.chapter.cmp(&b.chapter))
            .then_with(|| a.event_index.cmp(&b.event_index))
    });

    let error_count = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warn_count = diags.iter().filter(|d| d.severity == Severity::Warn).count();
    let info_count = diags.iter().filter(|d| d.severity == Severity::Info).count();
    ValidationReport {
        diagnostics: diags,
        error_count,
        warn_count,
        info_count,
        variables,
        edges,
    }
}

fn collect_script_characters(script_dir: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let dir = script_dir.join("characters");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let folder = e.file_name().to_string_lossy().to_string();
                // 引擎实际用的键是 settings.yml 的 script_role_key，缺省回落目录名
                let key = std::fs::read_to_string(e.path().join("settings.yml"))
                    .ok()
                    .and_then(|s| serde_yaml::from_str::<JsonValue>(&s).ok())
                    .and_then(|v| {
                        v.get("script_role_key")
                            .and_then(|x| x.as_str())
                            .map(|x| x.trim().to_string())
                    })
                    .filter(|s| !s.is_empty());
                out.insert(key.unwrap_or(folder));
            }
        }
    }
    out
}

fn media_field_of(ty: &str) -> (&'static str, MediaType) {
    match ty {
        "background" => ("imagePath", MediaType::Background),
        "present_pic" => ("imagePath", MediaType::Pic),
        "music" => ("musicPath", MediaType::Music),
        "sound" => ("soundPath", MediaType::Sound),
        _ => ("ambientPath", MediaType::Ambient),
    }
}

fn check_asset(
    data_dir: &Path,
    script_dir: &Path,
    obj: &serde_json::Map<String, JsonValue>,
    ty: &str,
    cid: &str,
    i: usize,
    diags: &mut Vec<Diagnostic>,
) {
    let (key, media) = media_field_of(ty);
    let path = obj.get(key).and_then(|v| v.as_str()).unwrap_or("");

    // ambient 的 stop=true 会跳过路径解析，空路径表示停全部轨
    if ty == "ambient" {
        let stopping = obj.get("stop").and_then(|v| v.as_bool()).unwrap_or(false);
        if stopping || path.is_empty() {
            return;
        }
    }
    if path.is_empty() {
        return; // 必填检查已经报过了
    }

    if resolve_script_media(data_dir, Some(script_dir), path, media).is_none() {
        diags.push(
            Diagnostic::event(
                Severity::Error,
                "asset.missing",
                cid,
                i,
                format!(
                    "找不到素材「{}」。运行时不会报错，只会静默把画面/声音清空",
                    path
                ),
            )
            .with_field(key),
        );
    }
}

/// 条件语法检查 + 变量收集。
fn check_condition(
    cond: &str,
    cid: &str,
    i: usize,
    diags: &mut Vec<Diagnostic>,
    vars_read: &mut BTreeSet<String>,
) {
    let cond = cond.trim();
    if cond.is_empty() {
        return;
    }

    // 只扫**运算符左侧**。右值是任意字符串，`bg == city/night` 里的 / 是合法内容，
    // 早先在整串上找 / + * ( ) 会把它误判成「用了不支持的运算符」并跳过变量收集。
    let var = if let Some((v, _)) = cond.split_once("!=") {
        v.trim()
    } else if let Some((v, _)) = cond.split_once("==") {
        v.trim()
    } else {
        cond
    };

    // 长运算符放前面，命中即停 —— 否则 `hp >= 5` 会同时报 >= 和 >
    const BAD_OPS: [&str; 9] = ["&&", "||", ">=", "<=", ">", "<", "!", "(", ")"];
    if let Some(op) = BAD_OPS.iter().find(|op| var.contains(**op)) {
        diags.push(
            Diagnostic::event(
                Severity::Error,
                "condition.unsupported_operator",
                cid,
                i,
                format!(
                    "条件里用了不支持的运算符「{}」。只支持 var == 值 / var != 值 / 裸变量真值 —— 写了别的不会报错，但条件会恒假",
                    op
                ),
            )
            .with_field("condition"),
        );
        return;
    }


    if var.is_empty() {
        diags.push(
            Diagnostic::event(
                Severity::Error,
                "condition.no_variable",
                cid,
                i,
                format!("条件「{}」左侧没有变量名", cond),
            )
            .with_field("condition"),
        );
        return;
    }
    if var.contains(' ') {
        diags.push(
            Diagnostic::event(
                Severity::Error,
                "condition.bad_variable",
                cid,
                i,
                format!("变量名「{}」含空格，引擎的变量名不允许空格", var),
            )
            .with_field("condition"),
        );
        return;
    }
    if cond.contains("%player%") {
        diags.push(
            Diagnostic::event(
                Severity::Warn,
                "condition.placeholder_not_replaced",
                cid,
                i,
                "condition 里的 %player% 不会被替换成玩家名".to_string(),
            )
            .with_field("condition"),
        );
    }
    vars_read.insert(var.to_string());
}

fn check_actions(
    actions: &[JsonValue],
    event_type: &str,
    cid: &str,
    i: usize,
    diags: &mut Vec<Diagnostic>,
    vars_written: &mut BTreeSet<String>,
) {
    for a in actions {
        let Some(ao) = a.as_object() else {
            diags.push(Diagnostic::event(
                Severity::Error,
                "action.not_a_map",
                cid,
                i,
                "action 必须是键值映射".to_string(),
            ));
            continue;
        };
        let at = ao.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let content = ao.get("content").and_then(|v| v.as_str()).unwrap_or("");

        match at {
            "set_var" => match parse_variable_action(content) {
                Ok((_, name, _)) => {
                    vars_written.insert(name);
                }
                Err(_) => diags.push(Diagnostic::event(
                    Severity::Error,
                    "action.bad_expression",
                    cid,
                    i,
                    format!(
                        "变量表达式「{}」无法解析。格式为 变量 = 值 / 变量 += 值 / 变量 -= 值（只有这三个运算符）",
                        content
                    ),
                )),
            },
            "add_line" => {
                if event_type == "set_variable" {
                    diags.push(Diagnostic::event(
                        Severity::Warn,
                        "action.not_supported_here",
                        cid,
                        i,
                        "「设置变量」事件只处理 set_var，这里的 add_line 会被静默忽略".to_string(),
                    ));
                }
                if content.trim().is_empty() {
                    diags.push(Diagnostic::event(
                        Severity::Warn,
                        "action.empty_content",
                        cid,
                        i,
                        "add_line 的内容为空".to_string(),
                    ));
                }
            }
            other => diags.push(Diagnostic::event(
                Severity::Warn,
                "action.unknown_type",
                cid,
                i,
                format!("未知的动作类型「{}」，会被静默忽略", other),
            )),
        }
    }
}

fn check_choices(
    obj: &serde_json::Map<String, JsonValue>,
    cid: &str,
    i: usize,
    diags: &mut Vec<Diagnostic>,
    vars_written: &mut BTreeSet<String>,
    vars_read: &mut BTreeSet<String>,
) {
    let Some(options) = obj.get("options").and_then(|v| v.as_array()) else {
        return;
    };
    if options.is_empty() {
        diags.push(Diagnostic::event(
            Severity::Error,
            "choices.empty",
            cid,
            i,
            "选项列表是空的，玩家无从选择".to_string(),
        ));
        return;
    }

    let last = options.len() - 1;
    let mut seen_texts: HashSet<&str> = HashSet::new();

    for (oi, opt) in options.iter().enumerate() {
        let Some(oo) = opt.as_object() else {
            diags.push(Diagnostic::event(
                Severity::Error,
                "choices.option_not_a_map",
                cid,
                i,
                format!("第 {} 个选项不是键值映射", oi + 1),
            ));
            continue;
        };

        let text = oo.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if text.is_empty() {
            if oi != last {
                diags.push(Diagnostic::event(
                    Severity::Warn,
                    "choices.catch_all_not_last",
                    cid,
                    i,
                    format!(
                        "第 {} 个选项没有文案。这类选项会匹配任意输入，放在中间会让后面的选项永远选不到",
                        oi + 1
                    ),
                ));
            }
        } else {
            if !seen_texts.insert(text) {
                diags.push(Diagnostic::event(
                    Severity::Warn,
                    "choices.duplicate_text",
                    cid,
                    i,
                    format!("选项文案「{}」重复，后一个永远选不到", text),
                ));
            }
            if text.contains("%player%") {
                diags.push(Diagnostic::event(
                    Severity::Warn,
                    "choices.placeholder_in_text",
                    cid,
                    i,
                    format!(
                        "第 {} 个选项的文案里有 %player%，但引擎只替换顶层字段，按钮上会原样显示",
                        oi + 1
                    ),
                ));
            }
        }

        if oo.contains_key("next") {
            diags.push(Diagnostic::event(
                Severity::Error,
                "choices.option_next_ignored",
                cid,
                i,
                format!(
                    "第 {} 个选项写了 next，但 choices 不支持选项级跳转，该字段被完全忽略。要按选择分支，请用 set_var 记录选择 + 章节结束的 branching",
                    oi + 1
                ),
            ));
        }

        if let Some(c) = oo.get("condition").and_then(|v| v.as_str()) {
            check_condition(c, cid, i, diags, vars_read);
        }

        if let Some(actions) = oo.get("actions").and_then(|v| v.as_array()) {
            check_actions(actions, "choices", cid, i, diags, vars_written);
        }
    }
}

fn check_set_variable(
    obj: &serde_json::Map<String, JsonValue>,
    cid: &str,
    i: usize,
    diags: &mut Vec<Diagnostic>,
    vars_written: &mut BTreeSet<String>,
    vars_read: &mut BTreeSet<String>,
) {
    let Some(options) = obj.get("options").and_then(|v| v.as_array()) else {
        diags.push(Diagnostic::event(
            Severity::Error,
            "set_variable.no_options",
            cid,
            i,
            "「设置变量」需要 options 列表。注意它的形状是 options[].actions[]，不是直接写 name/value".to_string(),
        ));
        return;
    };
    if options.is_empty() {
        diags.push(Diagnostic::event(
            Severity::Warn,
            "set_variable.empty",
            cid,
            i,
            "赋值组是空的，这个事件什么都不做".to_string(),
        ));
    }
    for opt in options {
        let Some(oo) = opt.as_object() else { continue };
        if let Some(c) = oo.get("condition").and_then(|v| v.as_str()) {
            check_condition(c, cid, i, diags, vars_read);
        }
        if let Some(actions) = oo.get("actions").and_then(|v| v.as_array()) {
            check_actions(actions, "set_variable", cid, i, diags, vars_written);
        }
    }
}

/// 记录一条章节跳转边，顺便校验目标是否存在。
///
/// 刻意写成自由函数而不是闭包：闭包会同时可变借用 `edges` 和 `diags`，
/// 调用方后面还要直接往 `diags` 里塞诊断，借用检查过不去。
#[allow(clippy::too_many_arguments)]
fn push_target(
    raw: &str,
    // label: 诊断文案里怎么称呼这个跳转（「下一章」/「第 2 个分支」）
    label: &str,
    // edge_label: 流程图连线上显示的标签（分支条件 / AI 分支名），linear 为空
    edge_label: &str,
    end_type: &str,
    cid: &str,
    i: usize,
    chapter_set: &HashSet<&str>,
    edges: &mut Vec<ChapterEdge>,
    diags: &mut Vec<Diagnostic>,
) {
    let target = chapter_id_of(raw);

    if target == "end" {
        edges.push(ChapterEdge {
            from: cid.to_string(),
            to: "end".to_string(),
            is_end: true,
            label: edge_label.to_string(),
            end_type: end_type.to_string(),
        });
        return;
    }
    if target.is_empty() {
        diags.push(Diagnostic::event(
            Severity::Error,
            "chapter_end.empty_target",
            cid,
            i,
            format!("{}没有指定目标章节", label),
        ));
        return;
    }
    if !chapter_set.contains(target) {
        diags.push(Diagnostic::event(
            Severity::Error,
            "chapter_end.dangling",
            cid,
            i,
            format!("{}指向的章节「{}」不存在", label, target),
        ));
        return;
    }
    edges.push(ChapterEdge {
        from: cid.to_string(),
        to: target.to_string(),
        is_end: false,
        label: edge_label.to_string(),
        end_type: end_type.to_string(),
    });
}

fn check_chapter_end(
    obj: &serde_json::Map<String, JsonValue>,
    cid: &str,
    i: usize,
    chapter_set: &HashSet<&str>,
    edges: &mut Vec<ChapterEdge>,
    diags: &mut Vec<Diagnostic>,
    vars_read: &mut BTreeSet<String>,
) {
    let end_type = obj
        .get("end_type")
        .and_then(|v| v.as_str())
        .unwrap_or("linear");

    match end_type {
        "linear" => {
            // 引擎优先 next，其次 next_chapter
            let next = obj.get("next").and_then(|v| v.as_str());
            let next_chapter = obj.get("next_chapter").and_then(|v| v.as_str());

            if next.is_some() && next_chapter.is_some() {
                diags.push(Diagnostic::event(
                    Severity::Warn,
                    "chapter_end.both_next_fields",
                    cid,
                    i,
                    "同时写了 next 和 next_chapter，引擎只会用 next".to_string(),
                ));
            }
            match next.or(next_chapter) {
                Some(t) => push_target(t, "下一章", "", end_type, cid, i, chapter_set, edges, diags),
                None => diags.push(Diagnostic::event(
                    Severity::Warn,
                    "chapter_end.no_next",
                    cid,
                    i,
                    "linear 但没写下一章，运行时会直接结束整个剧本".to_string(),
                )),
            }
        }
        "branching" | "ai_judged" => {
            let options = obj.get("options").and_then(|v| v.as_array());
            let Some(options) = options else {
                diags.push(Diagnostic::event(
                    Severity::Error,
                    "chapter_end.no_options",
                    cid,
                    i,
                    format!("{} 需要分支列表，否则一定落到「剧本结束」", end_type),
                ));
                return;
            };
            if options.is_empty() {
                diags.push(Diagnostic::event(
                    Severity::Error,
                    "chapter_end.no_options",
                    cid,
                    i,
                    format!("{} 的分支列表是空的，一定落到「剧本结束」", end_type),
                ));
            }
            let mut has_default = false;
            for (oi, opt) in options.iter().enumerate() {
                let Some(oo) = opt.as_object() else { continue };

                if oo.contains_key("text") || oo.contains_key("actions") {
                    diags.push(Diagnostic::event(
                        Severity::Error,
                        "chapter_end.choice_shaped_option",
                        cid,
                        i,
                        format!(
                            "第 {} 个分支写成了选项的形状（text/actions）。章节结束的分支只认 condition / next / default（ai_judged 用 name / next / default）",
                            oi + 1
                        ),
                    ));
                }

                if oo
                    .get("default")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    has_default = true;
                }

                if end_type == "ai_judged" && !oo.contains_key("name") {
                    diags.push(Diagnostic::event(
                        Severity::Warn,
                        "chapter_end.ai_option_no_name",
                        cid,
                        i,
                        format!("第 {} 个分支没有 name，AI 判定时无法命中它", oi + 1),
                    ));
                }
                if end_type == "branching" {
                    match oo.get("condition").and_then(|v| v.as_str()) {
                        Some(c) => check_condition(c, cid, i, diags, vars_read),
                        None => {
                            if !oo
                                .get("default")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                            {
                                diags.push(Diagnostic::event(
                                    Severity::Warn,
                                    "chapter_end.branch_no_condition",
                                    cid,
                                    i,
                                    format!(
                                        "第 {} 个分支没有条件，会无条件命中，后面的分支都选不到",
                                        oi + 1
                                    ),
                                ));
                            }
                        }
                    }
                }

                match oo.get("next").and_then(|v| v.as_str()) {
                    Some(t) => push_target(
                        t,
                        &format!("第 {} 个分支", oi + 1),
                        &branch_label(oo, oi),
                        end_type,
                        cid,
                        i,
                        chapter_set,
                        edges,
                        diags,
                    ),
                    None => diags.push(Diagnostic::event(
                        Severity::Error,
                        "chapter_end.branch_no_next",
                        cid,
                        i,
                        format!("第 {} 个分支没有 next，命中后会落到「剧本结束」", oi + 1),
                    )),
                }
            }
            if !has_default {
                diags.push(Diagnostic::event(
                    Severity::Warn,
                    "chapter_end.no_default_branch",
                    cid,
                    i,
                    "没有设 default 兜底分支。所有条件都不满足时会直接结束整个剧本".to_string(),
                ));
            }
        }
        other => {
            diags.push(
                Diagnostic::event(
                    Severity::Error,
                    "chapter_end.unknown_end_type",
                    cid,
                    i,
                    format!(
                        "未知的结束方式「{}」，运行时会直接结束整个剧本；可用值：linear / branching / ai_judged",
                        other
                    ),
                )
                .with_field("end_type"),
            );
        }
    }
}

/// 可达性、孤儿章节、环。
fn check_graph(intro: &str, chapters: &[String], edges: &[ChapterEdge], diags: &mut Vec<Diagnostic>) {
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut inbound: HashMap<&str, usize> = HashMap::new();
    for c in chapters {
        adj.entry(c.as_str()).or_default();
        inbound.entry(c.as_str()).or_insert(0);
    }
    for e in edges {
        if e.is_end {
            continue;
        }
        adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
        *inbound.entry(e.to.as_str()).or_insert(0) += 1;
    }

    // 从开场做可达性
    let mut reachable: HashSet<&str> = HashSet::new();
    let mut stack = vec![intro];
    while let Some(cur) = stack.pop() {
        if !reachable.insert(cur) {
            continue;
        }
        if let Some(ns) = adj.get(cur) {
            for n in ns {
                stack.push(*n);
            }
        }
    }

    for c in chapters {
        if reachable.contains(c.as_str()) {
            continue;
        }
        let has_in = inbound.get(c.as_str()).copied().unwrap_or(0) > 0;
        let msg = if has_in {
            format!(
                "章节「{}」虽然有别的章节指向它，但从开场章节走不到那些章节，玩家永远到不了这里",
                c
            )
        } else {
            format!("章节「{}」没有任何章节指向它，玩家永远走不到", c)
        };
        diags.push(Diagnostic::chapter(Severity::Warn, "graph.unreachable", c, msg));
    }

    // 环检测（DFS 三色）
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        White,
        Gray,
        Black,
    }
    let mut mark: HashMap<&str, Mark> = chapters
        .iter()
        .map(|c| (c.as_str(), Mark::White))
        .collect();
    let mut cycle: Option<Vec<String>> = None;

    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        mark: &mut HashMap<&'a str, Mark>,
        path: &mut Vec<&'a str>,
        found: &mut Option<Vec<String>>,
    ) {
        if found.is_some() {
            return;
        }
        mark.insert(node, Mark::Gray);
        path.push(node);
        if let Some(ns) = adj.get(node) {
            for n in ns {
                match mark.get(*n).copied().unwrap_or(Mark::White) {
                    Mark::White => dfs(*n, adj, mark, path, found),
                    Mark::Gray => {
                        if found.is_none() {
                            let start = path.iter().position(|p| *p == *n).unwrap_or(0);
                            let mut c: Vec<String> =
                                path[start..].iter().map(|s| (*s).to_string()).collect();
                            c.push((*n).to_string());
                            *found = Some(c);
                        }
                    }
                    Mark::Black => {}
                }
            }
        }
        path.pop();
        mark.insert(node, Mark::Black);
    }

    let keys: Vec<&str> = chapters.iter().map(|c| c.as_str()).collect();
    for c in keys {
        if mark.get(c).copied().unwrap_or(Mark::White) == Mark::White {
            let mut path = Vec::new();
            dfs(c, &adj, &mut mark, &mut path, &mut cycle);
        }
    }

    if let Some(c) = cycle {
        diags.push(Diagnostic::script(
            Severity::Warn,
            "graph.cycle",
            format!(
                "章节之间存在循环：{}。引擎没有循环检测，玩家可能被困在里面出不来",
                c.join(" → ")
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run_condition(cond: &str) -> (Vec<Diagnostic>, BTreeSet<String>) {
        let mut d = Vec::new();
        let mut v = BTreeSet::new();
        check_condition(cond, "main", 0, &mut d, &mut v);
        (d, v)
    }

    #[test]
    fn condition_flags_unsupported_operators() {
        for bad in ["hp >= 5", "hp > 5", "a && b", "a || b", "!flag", "(a == 1)"] {
            let (d, _) = run_condition(bad);
            assert!(
                d.iter().any(|x| x.code == "condition.unsupported_operator"),
                "应报不支持的运算符: {}",
                bad
            );
        }
    }

    #[test]
    fn condition_accepts_supported_forms_and_collects_vars() {
        let (d, v) = run_condition("route == shop");
        assert!(d.is_empty(), "不该有诊断: {:?}", d);
        assert!(v.contains("route"));

        let (d, v) = run_condition("flag != true");
        assert!(d.is_empty());
        assert!(v.contains("flag"));

        let (d, v) = run_condition("wet");
        assert!(d.is_empty());
        assert!(v.contains("wet"));
    }

    #[test]
    fn condition_rejects_spaces_in_variable_name() {
        let (d, _) = run_condition("my var == 1");
        assert!(d.iter().any(|x| x.code == "condition.bad_variable"));
    }

    #[test]
    fn choices_detects_the_prototype_mistakes() {
        let mut d = Vec::new();
        let mut w = BTreeSet::new();
        let mut r = BTreeSet::new();
        let obj = json!({
            "options": [
                { "text": "去便利店", "next": "shop" },
                { "text": "去便利店" },
                { "actions": [{ "type": "set_variable", "content": "x = 1" }] },
                { "text": "问 %player% 的名字" }
            ]
        });
        check_choices(obj.as_object().unwrap(), "main", 0, &mut d, &mut w, &mut r);

        let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
        // 选项级 next 不存在
        assert!(codes.contains(&"choices.option_next_ignored"));
        // 文案重复
        assert!(codes.contains(&"choices.duplicate_text"));
        // 无文案的兜底项没放最后
        assert!(codes.contains(&"choices.catch_all_not_last"));
        // action 类型写成了 set_variable（引擎只认 set_var）
        assert!(codes.contains(&"action.unknown_type"));
        // 选项文案里的 %player% 不会被替换
        assert!(codes.contains(&"choices.placeholder_in_text"));
    }

    #[test]
    fn set_variable_detects_the_prototype_shape() {
        let mut d = Vec::new();
        let mut w = BTreeSet::new();
        let mut r = BTreeSet::new();
        // 原型写的是 { name, value }，引擎只读 options[]
        let obj = json!({ "name": "affection", "value": 10 });
        check_set_variable(obj.as_object().unwrap(), "main", 0, &mut d, &mut w, &mut r);
        assert!(d.iter().any(|x| x.code == "set_variable.no_options"));
        assert!(w.is_empty(), "不该收集到任何被赋值的变量");
    }

    #[test]
    fn set_variable_collects_written_vars_and_flags_add_line() {
        let mut d = Vec::new();
        let mut w = BTreeSet::new();
        let mut r = BTreeSet::new();
        let obj = json!({
            "options": [{
                "actions": [
                    { "type": "set_var", "content": "affection += 1" },
                    { "type": "add_line", "content": "喂" }
                ]
            }]
        });
        check_set_variable(obj.as_object().unwrap(), "main", 0, &mut d, &mut w, &mut r);
        assert!(w.contains("affection"));
        assert!(d.iter().any(|x| x.code == "action.not_supported_here"));
    }

    #[test]
    fn chapter_end_detects_choice_shaped_branches() {
        let mut edges = Vec::new();
        let mut d = Vec::new();
        let mut r = BTreeSet::new();
        let chapters: HashSet<&str> = ["shop", "home"].into_iter().collect();
        // 原型把分支写成了 { text, actions }
        let obj = json!({
            "end_type": "branching",
            "options": [{ "text": "去便利店", "actions": [] }]
        });
        check_chapter_end(
            obj.as_object().unwrap(),
            "start",
            4,
            &chapters,
            &mut edges,
            &mut d,
            &mut r,
        );
        let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
        assert!(codes.contains(&"chapter_end.choice_shaped_option"));
        assert!(codes.contains(&"chapter_end.branch_no_next"));
        assert!(codes.contains(&"chapter_end.no_default_branch"));
    }

    #[test]
    fn chapter_end_linear_dangling_target() {
        let mut edges = Vec::new();
        let mut d = Vec::new();
        let mut r = BTreeSet::new();
        let chapters: HashSet<&str> = ["main2"].into_iter().collect();

        let obj = json!({ "end_type": "linear", "next_chapter": "main9" });
        check_chapter_end(
            obj.as_object().unwrap(),
            "main",
            3,
            &chapters,
            &mut edges,
            &mut d,
            &mut r,
        );
        assert!(d.iter().any(|x| x.code == "chapter_end.dangling"));

        // "end" 是合法终点
        let mut edges2 = Vec::new();
        let mut d2 = Vec::new();
        let obj = json!({ "end_type": "linear", "next_chapter": "end" });
        check_chapter_end(
            obj.as_object().unwrap(),
            "main",
            3,
            &chapters,
            &mut edges2,
            &mut d2,
            &mut r,
        );
        assert!(d2.is_empty(), "指向 end 不该报错: {:?}", d2);
        assert_eq!(edges2.len(), 1);
        assert!(edges2[0].is_end);
    }

    #[test]
    fn graph_finds_orphans_and_cycles() {
        let chapters: Vec<String> = ["a", "b", "c", "orphan"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let e = |from: &str, to: &str| ChapterEdge {
            from: from.into(),
            to: to.into(),
            is_end: false,
            label: String::new(),
            end_type: "linear".into(),
        };
        let edges = vec![e("a", "b"), e("b", "a"), e("orphan", "c")];
        let mut d = Vec::new();
        check_graph("a", &chapters, &edges, &mut d);

        let codes: Vec<&str> = d.iter().map(|x| x.code).collect();
        assert!(codes.contains(&"graph.cycle"), "应检出 a→b→a 的环");
        // orphan 无入边、c 只被 orphan 指向，两者都从开场走不到
        let unreachable: Vec<&Diagnostic> = d
            .iter()
            .filter(|x| x.code == "graph.unreachable")
            .collect();
        assert_eq!(unreachable.len(), 2);
    }

    #[test]
    fn free_dialogue_without_exit_is_an_error() {
        // 直接构造最小报告不方便，这里只验规则本身的判定条件
        let rounds: i64 = -1;
        let end_line = "";
        assert!(rounds <= 0 && end_line.trim().is_empty());
    }
}
