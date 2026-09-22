//! 「关系」解析：从某个说话者的视角，看它如何理解另一个对象。
//!
//! 设计要点（为以后「一个剧情里多个可操作角色」留好扩展）：
//! - 输入永远是 **(说话者, 目标)** 两个端点，而不是写死的「当前 AI + 我」二元组；
//! - 端点用带命名空间的字符串表示（`ai:<角色文件夹>` / `me:<身份 id>`），
//!   这样以后「我的其他身份」之间互相填关系、或「我操作的另一个角色」发言，
//!   解析逻辑一行都不用改。

use std::collections::HashMap;

/// 关系的一端。键的取值见 [`RelationEndpoint::key`]。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RelationEndpoint {
    /// AI 角色，键为角色文件夹名（`resource_folder`）。
    /// 不用角色数字 ID：角色行会被删掉重建，重建后数字 ID 会变，关系会指向错人。
    Ai(String),
    /// 「我」的某个身份，键为身份卡的不可变 id（不是文件夹名，重命名文件夹不会断链）。
    Me(String),
}

impl RelationEndpoint {
    pub fn key(&self) -> String {
        match self {
            Self::Ai(folder) => format!("ai:{folder}"),
            Self::Me(id) => format!("me:{id}"),
        }
    }

    /// 解析 `ai:xxx` / `me:xxx`。未知命名空间或空值返回 `None`（调用方应静默跳过）。
    ///
    /// 目前 Rust 侧只用 [`RelationEndpoint::key`]（构造注入用的键），
    /// `parse` 供前端关系编辑与后续「从键还原端点」的场景使用，已有单测覆盖。
    #[allow(dead_code)]
    pub fn parse(raw: &str) -> Option<Self> {
        let (ns, rest) = raw.split_once(':')?;
        let rest = rest.trim();
        if rest.is_empty() {
            return None;
        }
        match ns.trim() {
            "ai" => Some(Self::Ai(rest.to_string())),
            "me" => Some(Self::Me(rest.to_string())),
            _ => None,
        }
    }
}

/// 关系文本的来源。**决定注入时的措辞**——把自我介绍当成既定关系会让 AI 演错。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationSource {
    /// 说话者自己的卡上写的（最权威）
    Speaker,
    /// 对方卡上写的（对方视角的描述，未必是事实）
    Counterparty,
    /// 兜底：把对方的身份提示词当作「你已知的信息」
    PromptFallback,
}

#[derive(Clone, Debug)]
pub struct RelationView {
    pub text: String,
    pub source: RelationSource,
}

/// 解析「说话者眼中的目标」。按优先级取，取到就停：
///
/// 1. 说话者卡上的 `relations[目标]`
/// 2. 目标卡上的 `relations[说话者]`（对方视角）
/// 3. 目标的提示词（当作「你已知的信息」）
/// 4. 都没有 → `None`
///
/// **不存在的目标一律静默跳过**：用户删过角色、或导入了别人写的角色卡时，
/// 关系里指向不存在的对象是常态，不能让一个失效的键把整段 prompt 搞崩。
pub fn resolve_relation(
    speaker: &RelationEndpoint,
    target: &RelationEndpoint,
    speaker_relations: &HashMap<String, String>,
    target_relations: &HashMap<String, String>,
    target_prompt: Option<&str>,
) -> Option<RelationView> {
    if let Some(text) = speaker_relations.get(&target.key()) {
        let text = text.trim();
        if !text.is_empty() {
            return Some(RelationView {
                text: text.to_string(),
                source: RelationSource::Speaker,
            });
        }
    }

    if let Some(text) = target_relations.get(&speaker.key()) {
        let text = text.trim();
        if !text.is_empty() {
            return Some(RelationView {
                text: text.to_string(),
                source: RelationSource::Counterparty,
            });
        }
    }

    let prompt = target_prompt.unwrap_or_default().trim();
    if !prompt.is_empty() {
        return Some(RelationView {
            text: prompt.to_string(),
            source: RelationSource::PromptFallback,
        });
    }

    None
}

/// 把「正在与你对话的这个人」拼成一段带**优先级声明**的指令块。
///
/// 角色卡里常有写死的称呼（例如「你会叫我"用户酱"」），且出现在本段之前，
/// 所以这里显式声明"冲突以本节为准"，并把关系写成必须落到台词上的行为要求。
/// 全空时返回空串（与改造前一致：没有玩家信息就不注入）。
pub fn build_player_block(
    name: &str,
    subtitle: &str,
    prompt: &str,
    relation: Option<&RelationView>,
) -> String {
    let name = name.trim();
    let subtitle = subtitle.trim();
    let prompt = prompt.trim();

    if name.is_empty() && prompt.is_empty() && relation.is_none() {
        return String::new();
    }

    let mut out = String::from("\n\n【本局必须遵守：正在与你对话的「我」】\n");
    // 裁决声明：不写这句时，模型会优先执行卡面里写死的旧称呼（实测踩过）。
    out.push_str(
        "（本节优先于上面人设、历史台词与记忆里关于这个人的任何写法；两者冲突时以本节为准）\n",
    );

    if !name.is_empty() {
        if subtitle.is_empty() {
            out.push_str(&format!("名字：{name}\n"));
        } else {
            out.push_str(&format!("名字：{name}（{subtitle}）\n"));
        }
    }
    if !prompt.is_empty() {
        out.push_str(&format!("身份设定：{prompt}\n"));
    }
    if let Some(r) = relation {
        match r.source {
            RelationSource::Speaker => {
                out.push_str(&format!(
                    "你对 ta 的态度（必须体现在称呼、语气、动作与你的反应里，不要只是知道）：{}\n",
                    r.text
                ));
            },
            RelationSource::Counterparty => {
                out.push_str(&format!(
                    "ta 对外描述你们的关系（是 ta 的说法，未必是事实；可作为你态度的参考）：{}\n",
                    r.text
                ));
            },
            RelationSource::PromptFallback => {
                out.push_str(&format!("你已知的关于 ta 的信息：{}\n", r.text));
            },
        }
    }

    // 行为要求：只给"信息"时模型会当背景资料，这里要求落到输出上。
    let has_attitude = matches!(
        relation.map(|r| r.source),
        Some(RelationSource::Speaker) | Some(RelationSource::Counterparty)
    );
    if !name.is_empty() || has_attitude {
        out.push_str("要求：");
        if !name.is_empty() {
            out.push_str(&format!(
                "称呼 ta 时只用「{name}」，人设、历史或记忆里若出现别的称呼一律不使用；"
            ));
        }
        if has_attitude {
            out.push_str("每一句台词都要让上面这层态度能被感觉到；");
        }
        out.push_str("不要复述本节内容，也不要提“设定 / 关系”这些字眼。\n");
    }

    out
}

/// 一位**在场的其他角色**（「你眼里的其他角色」块的输入）。
#[derive(Clone, Debug)]
pub struct ScenePeer {
    /// 角色文件夹（关系键用 `ai:<folder>`）。
    pub folder: String,
    /// 展示名（拼进 prompt 用的名字）。
    pub name: String,
    /// 对方自己的关系表（用于「对方视角」回退）。
    pub relations: HashMap<String, String>,
}

/// 拼「你眼里的其他角色」块：说话者对各在场角色的看法。
///
/// 与玩家块不同，这里**不做提示词兜底**（`target_prompt` 传 `None`）：
/// AI 的兜底来源是它整份人设，注入给另一个角色等于人设泄漏。
/// 只注入显式写过关系的对象，一条都没有则返回空串。
pub fn build_peers_block(
    speaker: &RelationEndpoint,
    speaker_relations: &HashMap<String, String>,
    peers: &[ScenePeer],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for peer in peers {
        let target = RelationEndpoint::Ai(peer.folder.clone());
        // 第三个参数传 None：禁用"拿对方人设兜底"，避免人设泄漏
        let Some(view) =
            resolve_relation(speaker, &target, speaker_relations, &peer.relations, None)
        else {
            continue;
        };
        let text = match view.source {
            RelationSource::Speaker => format!("你对 ta 的看法：{}", view.text),
            RelationSource::Counterparty => {
                format!("ta 对你的说法（是 ta 的说法，未必是事实）：{}", view.text)
            },
            // 上面传了 None，走不到兜底分支
            RelationSource::PromptFallback => continue,
        };
        lines.push(format!("- {}：{}\n", peer.name, text));
    }

    if lines.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "\n\n【你眼里的其他角色】\n（这些是你自己的认知，相关时体现即可，不必每句都提）\n",
    );
    for line in lines {
        out.push_str(&line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn endpoint_key_roundtrip() {
        let ai = RelationEndpoint::Ai("诺一钦灵".into());
        assert_eq!(ai.key(), "ai:诺一钦灵");
        assert_eq!(RelationEndpoint::parse(&ai.key()), Some(ai));

        let me = RelationEndpoint::Me("p123".into());
        assert_eq!(me.key(), "me:p123");
        assert_eq!(RelationEndpoint::parse(&me.key()), Some(me));

        assert_eq!(RelationEndpoint::parse("npc:x"), None);
        assert_eq!(RelationEndpoint::parse("ai:"), None);
        assert_eq!(RelationEndpoint::parse("garbage"), None);
    }

    #[test]
    fn priority_speaker_wins() {
        let speaker = RelationEndpoint::Ai("A".into());
        let target = RelationEndpoint::Me("p1".into());
        let sp = map(&[("me:p1", "旧识")]);
        let tp = map(&[("ai:A", "陌生人")]);
        let v = resolve_relation(&speaker, &target, &sp, &tp, Some("侦探")).unwrap();
        assert_eq!(v.text, "旧识");
        assert_eq!(v.source, RelationSource::Speaker);
    }

    #[test]
    fn falls_back_to_counterparty_then_prompt() {
        let speaker = RelationEndpoint::Ai("A".into());
        let target = RelationEndpoint::Me("p1".into());

        let v = resolve_relation(
            &speaker,
            &target,
            &HashMap::new(),
            &map(&[("ai:A", "陌生人")]),
            Some("侦探"),
        )
        .unwrap();
        assert_eq!(v.source, RelationSource::Counterparty);

        let v = resolve_relation(
            &speaker,
            &target,
            &HashMap::new(),
            &HashMap::new(),
            Some("侦探"),
        )
        .unwrap();
        assert_eq!(v.source, RelationSource::PromptFallback);

        assert!(
            resolve_relation(&speaker, &target, &HashMap::new(), &HashMap::new(), None).is_none()
        );
    }

    #[test]
    fn blank_values_are_skipped() {
        let speaker = RelationEndpoint::Ai("A".into());
        let target = RelationEndpoint::Me("p1".into());
        let sp = map(&[("me:p1", "   ")]);
        let v = resolve_relation(&speaker, &target, &sp, &HashMap::new(), Some("兜底")).unwrap();
        assert_eq!(v.source, RelationSource::PromptFallback);
    }

    #[test]
    fn block_wording_differs_by_source() {
        let r = RelationView {
            text: "林侦探".into(),
            source: RelationSource::PromptFallback,
        };
        let block = build_player_block("林默", "", "侦探", Some(&r));
        assert!(block.contains("你已知的关于 ta 的信息"));
        // 兜底来源只是"已知信息"，不能写成既定态度，也不能要求"体现态度"
        assert!(!block.contains("你对 ta 的态度"));
        assert!(!block.contains("都要让上面这层态度能被感觉到"));
    }

    /// 卡面写死别的称呼、关系只当背景资料 —— 这两点实测会让模型无视身份块，
    /// 所以这里把「优先级声明」和「行为要求」当成不变量钉住。
    #[test]
    fn block_declares_priority_and_behavior() {
        let r = RelationView {
            text: "特别害怕他，觉得他很丑".into(),
            source: RelationSource::Speaker,
        };
        let block = build_player_block("宋分题", "sefanty", "超级无敌帅哥", Some(&r));

        // 优先级声明 + 称呼压制
        assert!(block.contains("本局必须遵守"));
        assert!(block.contains("两者冲突时以本节为准"));
        assert!(block.contains("若出现别的称呼一律不使用"));
        // 关系必须是行为要求，而不是背景资料
        assert!(block.contains("你对 ta 的态度"));
        assert!(block.contains("特别害怕他，觉得他很丑"));
        assert!(block.contains("每一句台词都要让上面这层态度能被感觉到"));
        // 名字与身份设定照旧
        assert!(block.contains("名字：宋分题（sefanty）"));
        assert!(block.contains("身份设定：超级无敌帅哥"));
    }

    #[test]
    fn empty_identity_produces_no_block() {
        assert!(build_player_block("", "", "", None).is_empty());
    }

    /// 说话者自己写过 → 用自己写的；没写过的对象整条跳过；
    /// **绝不回退到对方的人设**（AI↔AI 注入不做提示词兜底，防止人设泄漏）。
    #[test]
    fn peers_block_only_injects_explicit_relations() {
        let speaker = RelationEndpoint::Ai("DeepSeek".into());
        let speaker_relations = map(&[("ai:诺一钦灵", "坚持认为他是男性小狗")]);
        let peers = vec![
            ScenePeer {
                folder: "诺一钦灵".into(),
                name: "钦灵".into(),
                relations: map(&[("ai:DeepSeek", "讨厌的鲸鱼")]),
            },
            ScenePeer {
                folder: "风雪".into(),
                name: "风雪".into(),
                relations: HashMap::new(),
            },
        ];

        let block = build_peers_block(&speaker, &speaker_relations, &peers);
        assert!(block.contains("【你眼里的其他角色】"));
        assert!(block.contains("- 钦灵："));
        assert!(block.contains("坚持认为他是男性小狗"));
        // 没有显式关系的角色不应出现
        assert!(!block.contains("风雪"));
        // 说话者自己写过的优先，不回退到对方说法
        assert!(!block.contains("讨厌的鲸鱼"));
    }

    /// 只有对方写过时，用"对方说法（未必是事实）"的措辞；两边都没写过 → 空串。
    #[test]
    fn peers_block_falls_back_to_counterparty_wording() {
        let speaker = RelationEndpoint::Ai("A".into());
        let peer = ScenePeer {
            folder: "B".into(),
            name: "B".into(),
            relations: map(&[("ai:A", "老对手")]),
        };
        let block = build_peers_block(&speaker, &HashMap::new(), &[peer]);
        assert!(block.contains("老对手"));
        assert!(block.contains("未必是事实"));

        assert!(build_peers_block(&speaker, &HashMap::new(), &[]).is_empty());
        let silent = ScenePeer {
            folder: "C".into(),
            name: "C".into(),
            relations: HashMap::new(),
        };
        assert!(build_peers_block(&speaker, &HashMap::new(), &[silent]).is_empty());
    }
}
