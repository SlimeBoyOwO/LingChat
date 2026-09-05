use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;

use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::LlmMessage;

use super::context::truncate_to_chars;

// ── 中文压缩提示词（与 Python PersistentMemorySystem._init_prompts 完全一致） ──

pub(crate) fn init_prompts() -> HashMap<String, String> {
    let base_role = concat!(
        "你是一个专业的【记忆档案管理员】。你的任务是基于【旧的记忆档案】和【新增的对话日志】，",
        "生成一份更新后的、逻辑连贯的记忆文本。\n",
        "通用规则：\n",
        "1. 视角：必须严格使用【第三人称】（例如：'（用户的名字）提到...'，'（本AI角色的名字）感到...'）。\n",
        "2. 时态：使用陈述语气，客观记录事实。\n",
        "3. 输出：直接输出更新后的内容本身，不要包含任何解释。\n",
        "4. 逻辑：如果没有新信息需要更新，请原样保留【旧的记忆档案】的内容。\n",
        "5. 内容完整性：如果【旧的记忆档案】中存在被截断或不完整的片段，请直接丢弃，不要保留或引用它们。\n",
    );

    let mut m = HashMap::new();
    m.insert(
        "short_term".to_string(),
        format!(
            "{}\n【任务目标】：生成一份【短期上下文摘要】，用于在下一次对话中承接话题。\n\
             【处理逻辑】：\n\
             1. 概括话题：他们刚才在聊什么？话题是否已经结束？\n\
             2. 捕捉氛围：当前的对话气氛如何？\n\
             3. 遗忘机制：删除旧记忆中已经过时、结束或不再相关的琐碎细节。\n\
             4. 篇幅控制：保持在 100-200 字以内。\n",
            base_role
        ),
    );
    m.insert(
        "long_term".to_string(),
        format!(
            "{}\n【任务目标】：编撰一份【角色经历编年史】，记录具有长期价值的核心事件。\n\
             【处理逻辑】：\n\
             1. 过滤噪音：忽略日常问候和闲聊。\n\
             2. 提取事件：只记录具有里程碑意义的事件。\n\
             3. 累积更新：将新发生的关键事件追加到旧档案中。\n",
            base_role
        ),
    );
    m.insert(
        "user_info".to_string(),
        format!(
            "{}\n【任务目标】：更新【taの画像】，确保 AI 了解屏幕对面的人。\n\
             【处理逻辑】：\n\
             1. 事实提取：提取用户的姓名、年龄、职业、喜好、雷点等。\n\
             2. 冲突修正：如果信息冲突（如换了工作），以【新增对话】为准。\n",
            base_role
        ),
    );
    m.insert(
        "promises".to_string(),
        format!(
            "{}\n【任务目标】：维护一份【待办与契约清单】。\n\
             【处理逻辑】：\n\
             1. 新增约定：提取对话中明确达成的承诺。\n\
             2. 状态核销：如果能够在【新增对话】中找到已完成的证据，从清单中【删除】该条目。\n",
            base_role
        ),
    );
    m
}

/// Return one updated section; the runtime owns retry/atomic commit policy.
pub(crate) async fn update_section(
    llm: &Arc<LlmClient>,
    prompts: &HashMap<String, String>,
    chat_text: &str,
    key: &str,
    old_content: &str,
    max_chars: usize,
    ai_name: &str,
) -> Result<String> {
    let prompt_req = match prompts.get(key) {
        Some(p) => p,
        None => return Ok(old_content.to_string()), // 配置缺失不是失败，保留旧内容
    };

    // 喂给压缩 LLM 前按上限截断旧内容。LLM 只能基于截断后的内容生成新记忆，
    // 因此超出上限的旧记忆片段会在本次压缩写回后被丢弃（写回本身不截断）。
    let original_count = old_content.chars().count();
    let exceeds_limit = max_chars != 0 && original_count > max_chars;
    let old = truncate_to_chars(old_content, max_chars);
    if exceeds_limit {
        tracing::warn!(
            "MemoryBank: 记忆段 '{}' 旧内容超长 ({} 字符 > 上限 {} 字符)，超限尾部将被本次压缩丢弃；如不希望丢失请调大上限或设为 0",
            key,
            original_count,
            max_chars
        );
    }

    let full_prompt = format!(
        "{}\n\n【角色名称】：{}\n【旧内容】：\n{}\n\n【新增对话】：\n{}\n\n【新内容】(直接输出结果，不要废话)：",
        prompt_req, ai_name, old, chat_text,
    );

    let messages = vec![LlmMessage::user(full_prompt)];

    let response = llm.complete(&messages).await?;
    let cleaned = response.trim();
    if cleaned.is_empty() {
        // 空响应视为失败：部分 provider 故障时可能返回空串。若按成功处理，
        // 会把空内容写回并推进指针，静默丢弃该批对话，违背重试语义。
        return Err(anyhow::anyhow!("LLM 返回空内容"));
    }
    Ok(cleaned.to_string())
}
