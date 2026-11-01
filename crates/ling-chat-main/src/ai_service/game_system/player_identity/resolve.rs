//! 「关系」解析：从某个说话者的视角，看它如何理解另一个对象。

use std::collections::HashMap;

use crate::ai_service::types::CharacterSettings;

/// 该角色眼里「我」的称呼：角色卡默认称呼优先，未配置时用身份名。
pub fn effective_user_alias(settings: &CharacterSettings, identity_name: &str) -> String {
    if !settings.user_name.trim().is_empty() {
        return settings.user_name.trim().to_string();
    }
    identity_name.trim().to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RelationEndpoint {
    Ai(String),
    Me(String),
}

impl RelationEndpoint {
    pub fn key(&self) -> String {
        match self {
            Self::Ai(folder) => format!("ai:{folder}"),
            Self::Me(id) => format!("me:{id}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationSource {
    Speaker,
    Counterparty,
    PromptFallback,
}

#[derive(Clone, Debug)]
pub struct RelationView {
    pub text: String,
    pub source: RelationSource,
}

/// 解析「说话者眼中的目标」：按说话者卡、对方卡、目标提示词的顺序取，取到即停；都没有返回 `None`。
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

/// 在场一位角色对玩家身份的称呼（玩家块的称呼一览用）。
#[derive(Clone, Debug)]
pub struct AliasEntry {
    /// 角色展示名（与旁白里出现的一致）。
    pub name: String,
    /// 该角色对玩家身份的称呼。
    pub alias: String,
    /// 是否本角色自己那一行。
    pub is_self: bool,
}

/// 把「正在与你对话的这个人」拼成一段带优先级声明的指令块；全空时返回空串。
/// 称呼有差异时输出身份名与全场称呼一览，并声明不同称呼指同一人。
pub fn build_player_block(
    identity_name: &str,
    alias: &str,
    prompt: &str,
    relation: Option<&RelationView>,
    player_view: Option<&str>,
    roster: &[AliasEntry],
) -> String {
    let identity_name = identity_name.trim();
    let alias = alias.trim();
    let prompt = prompt.trim();

    if identity_name.is_empty() && prompt.is_empty() && relation.is_none() && player_view.is_none()
    {
        return String::new();
    }

    let call_name = if alias.is_empty() {
        identity_name
    } else {
        alias
    };
    let has_diff = roster.iter().any(|e| e.alias != identity_name) || alias != identity_name;

    let mut out = format!("\n\n以下是我（{identity_name}）的身份\n");
    out.push_str("（以下是现实的基础设定）\n");

    if has_diff {
        out.push_str(&format!(
            "我的名字：{identity_name}（旁白与系统提示里会用这个名字称呼我）\n"
        ));
        if !roster.is_empty() {
            out.push_str(&format!("在场所有人对我（{identity_name}）的称呼\n"));
            for entry in roster {
                if entry.is_self {
                    out.push_str(&format!("- 你（{}）叫我{}\n", entry.name, entry.alias));
                } else {
                    out.push_str(&format!("- {}叫我{}\n", entry.name, entry.alias));
                }
            }
        }
    } else if !identity_name.is_empty() {
        out.push_str(&format!("我的名字：{identity_name}\n"));
    }
    if !prompt.is_empty() {
        out.push_str(&format!(
            "我的基础设定（这是对所有角色都公开、所有角色都看得到的设定）：{prompt}\n"
        ));
    }
    if let Some(r) = relation {
        match r.source {
            RelationSource::Speaker => {
                out.push_str(&format!(
                    "你对我的认知（你们的关系；这是你单方面的私人认知，必须体现在称呼、语气、动作与你的反应里，不要只是知道）：{}\n",
                    r.text
                ));
                out.push_str(
                    "关于这一条：它不一定与事实相符，也不必与其他角色的认知一致；这是我在基础设定以外的更特殊的联系。\n",
                );
            },
            RelationSource::Counterparty => {},
            RelationSource::PromptFallback => {},
        }
    }
    if let Some(text) = player_view {
        let text = text.trim();
        if !text.is_empty() {
            out.push_str(&format!(
                "我对你的认知（这是我对你的私人认知，同样不一定与事实相符）：{text}\n"
            ));
        }
    }

    let has_attitude = matches!(relation.map(|r| r.source), Some(RelationSource::Speaker));
    if !call_name.is_empty() || has_attitude {
        out.push('\n');
        if has_diff {
            out.push_str(&format!(
                "要求：回答时称呼 ta 用「{call_name}」；ta 的身份名是「{identity_name}」，旁白、系统提示与其他角色提到「{identity_name}」时，指的都是同一个 ta，不要因为不同角色对 ta 的称呼不同而影响你的判断；不要复述本节内容，也不要提“设定 / 关系 / 认知”这些字眼。\n"
            ));
        } else {
            out.push_str(&format!(
                "要求：在一般情况下，称呼 ta 时用「{call_name}」；不要复述本节内容，也不要提“设定 / 关系 / 认知”这些字眼。\n"
            ));
        }
    }

    out
}

#[derive(Clone, Debug)]
pub struct ScenePeer {
    pub folder: String,
    pub name: String,
    pub relations: HashMap<String, String>,
}

/// 拼「你眼里的其他角色」块；不做提示词兜底，只注入显式写过关系的对象。
pub fn build_peers_block(
    speaker: &RelationEndpoint,
    speaker_relations: &HashMap<String, String>,
    peers: &[ScenePeer],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    for peer in peers {
        let target = RelationEndpoint::Ai(peer.folder.clone());
        let Some(view) =
            resolve_relation(speaker, &target, speaker_relations, &peer.relations, None)
        else {
            continue;
        };
        let text = match view.source {
            RelationSource::Speaker => format!("你对 ta 的认知：{}", view.text),
            RelationSource::Counterparty => {
                format!("ta 对你的说法（是 ta 的说法，未必是事实）：{}", view.text)
            },
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
