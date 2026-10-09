//! 历史与上下文处理：从 core.rs 拆出的纯函数。

use std::path::Path;

use crate::ai_service::skill_agent::config::SkillAgentConfig;
use crate::ai_service::skill_agent::tools;
use crate::ai_service::types::LlmMessage;

/// 构建「当前剧本」段：给出 key/路径，并指示 agent 先看已有内容（实时读取，不注入静态快照）。
pub(super) fn build_script_block(
    sandbox_dir: &Path,
    script_key: Option<&str>,
    known_keys: &[String],
) -> String {
    let Some(key) = script_key else {
        let mut out = String::from(include_str!("prompts/core_build_script_block.md"));
        if !known_keys.is_empty() {
            out.push_str("\n\n（磁盘上现有的剧本包，仅供你告诉用户「有这些」：");
            out.push_str(&known_keys.join("、"));
            out.push('）');
        }
        return out;
    };
    match crate::utils::script_paths::resolve_script_dir(key) {
        Ok(dir) => {
            let rel = dir.strip_prefix(sandbox_dir).unwrap_or(&dir);
            format!(
                "\n\n【当前剧本上下文】\n剧本 key：{}\n剧本目录：{}（相对于文件沙箱根 {}）\n\n工作之前，请先用 list_files / read_file 查看剧本中已有的内容，再决定如何编写或修改。\n剧本中的素材引用（imagePath / musicPath / soundPath / ambientPath）只写素材文件名本身（如 夜晚.webp），不要带 backgrounds/、musics/ 等类型目录前缀；引擎会按事件类型自动到对应目录查找。",
                key,
                rel.display(),
                sandbox_dir.display()
            )
        },
        Err(_) => String::new(), // 剧本缺失/失效 → 降级，不阻断对话
    }
}

pub(super) fn build_system_prompt(
    config: &SkillAgentConfig,
    allowed: &[&str],
    skills_block: &str,
    script_block: &str,
    task_block: &str,
    sandbox_dir: &Path,
    skills_dir: &Path,
) -> String {
    let can = |name: &str| allowed.contains(&name);
    let tool_names = tools::tool_names(allowed);
    let platform = if cfg!(mobile) { "移动端" } else { "桌面" };

    let mut abilities = format!(
        "你是运行在本机 LingChat {platform}应用里的 AI 剧本创作助手。你拥有以下能力：\
         \n- 调用工具完成真实操作：{tool_names}"
    );
    if can("read_skill") {
        abilities.push_str("\n- 通过 read_skill 加载技能指令后再执行任务");
    }
    abilities.push_str(&format!(
        "\n- 文件路径默认相对于文件沙箱根目录（{}）\
         \n- 技能目录：{}（技能文件以 SKILL.md 存放，需要时可用 list_files / read_file 直接查看）",
        sandbox_dir.display(),
        skills_dir.display()
    ));
    if can("execute_command") {
        abilities.push_str(if cfg!(mobile) {
            "\n- 当前移动端不提供 execute_command，不能运行 shell 命令"
        } else {
            "\n- execute_command 可能需要用户确认；命令由系统 shell 执行，带空格的参数请用引号包裹（引号会原样传递）"
        });
    }

    let mut rules: Vec<String> = Vec::new();
    if can("read_skill") {
        rules.push(
            "当任务匹配某个技能的描述时，先调用 read_skill 加载该技能，再按指令执行；\
             已读取过的技能不要重复读取"
                .to_string(),
        );
    }
    let mut file_tools = vec!["list_files", "read_file"];
    for extra in ["write_file", "delete_file"] {
        if can(extra) {
            file_tools.push(extra);
        }
    }
    rules.push(format!("需要操作文件时使用 {}", file_tools.join(" / ")));
    if can("write_file") {
        rules.push(
            "任务必须完成到产出物为止：读取技能、查询配色、运行搜索都只是中间步骤，\
             最终必须调用 write_file 实际写出用户要求的文件，才算完成任务"
                .to_string(),
        );
        rules.push(
            "未写出文件之前禁止总结收尾，禁止以「已获取到所需信息」「以上就是设计建议」\
             之类的说法结束回答；继续调用工具，直到文件真正创建成功"
                .to_string(),
        );
        rules.push(
            // 催它动手的规矩旁边必须写出出口："总得写点什么"会让它多写正文，或把已有的那几章重写一遍。
            "**但本项本来就该一个字都不写时，就不写**：用户这一句里没有这件事、\
             或者这一项要的东西盘上已经有了（用户没说要改），就如实说清、**不要动任何文件** —— \
             上面两条不适用；宁可空手收尾，也不许为了「有产出」写出用户没要的东西"
                .to_string(),
        );
        rules.push(
            "写文件时一次性用 write_file 写完整内容，不要提前分段；只有当一次写入因参数过长\
             而失败（报错会附带 [诊断] 提示）时，才改用 write_file（append=true）分段补齐"
                .to_string(),
        );
    }
    rules.push("文件范围受限时如实说明，不要编造文件内容".to_string());

    let numbered = rules
        .iter()
        .enumerate()
        .map(|(i, r)| format!("\n{}. {}", i + 1, r))
        .collect::<String>();
    let default = format!("{abilities}\n使用规则：{numbered}");

    let base = match &config.system_prompt {
        Some(custom) if !custom.trim().is_empty() => custom.clone(),
        _ => default,
    };
    format!("{}{}{}{}", base, script_block, skills_block, task_block)
}

/// 从上两句「纯对话」里取最近 N 轮，用于解「继续 / 都行 / 按你说的」；只取 assistant 的纯文本回复（带 tool_calls 的不算）。
pub(super) fn recent_turns(history: &[LlmMessage], n: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (i, msg) in history.iter().enumerate().rev() {
        if msg.role != "assistant" || msg.tool_calls.is_some() || msg.content.trim().is_empty() {
            continue;
        }
        let Some(prev) = history[..i].iter().rev().find(|m| m.role == "user") else {
            continue;
        };
        out.push((prev.content.clone(), msg.content.clone()));
        if out.len() >= n {
            break;
        }
    }
    out.reverse();
    out
}

/// 规整从 DB 加载的历史，修复/丢弃不完整的工具轮次。
pub fn sanitize_history(history: Vec<LlmMessage>) -> Vec<LlmMessage> {
    let mut out = Vec::with_capacity(history.len());
    let mut i = 0usize;
    while i < history.len() {
        let msg = &history[i];

        if msg.role == "assistant" && msg.tool_calls.is_some() {
            let expected: Vec<String> = msg
                .tool_calls
                .as_ref()
                .map(|tcs| tcs.iter().map(|tc| tc.id.clone()).collect())
                .unwrap_or_default();

            let mut j = i + 1;
            let mut actual: Vec<String> = Vec::new();
            while j < history.len() && history[j].role == "tool" {
                if let Some(id) = history[j].tool_call_id.clone() {
                    actual.push(id);
                }
                j += 1;
            }

            let complete = !expected.is_empty() && expected.iter().all(|id| actual.contains(id));

            if complete {
                out.push(msg.clone());
                for t in history.iter().take(j).skip(i + 1) {
                    if let Some(id) = t.tool_call_id.as_ref() {
                        if expected.contains(id) {
                            out.push(t.clone());
                        }
                    }
                }
            } else {
                let mut fixed = msg.clone();
                fixed.tool_calls = None;
                out.push(fixed);
            }
            i = j;
        } else if msg.role == "tool" {
            i += 1;
        } else {
            out.push(msg.clone());
            i += 1;
        }
    }
    out
}

/// provider 的报错是不是"超出上下文"：各家措辞不同，只做宽松匹配。
pub(super) fn looks_like_context_overflow(err: &str) -> bool {
    const PATTERNS: [&str; 5] = [
        "maximum context length",
        "context_length_exceeded",
        "context window",
        "too many tokens",
        "reduce the length",
    ];
    let lower = err.to_lowercase();
    PATTERNS.iter().any(|p| lower.contains(p))
}
