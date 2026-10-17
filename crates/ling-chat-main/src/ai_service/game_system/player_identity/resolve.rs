//! 「关系」解析：从某个说话者的视角，看它如何理解另一个对象。

use std::collections::HashMap;

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

/// 把「正在与你对话的这个人」拼成一段带优先级声明的指令块；全空时返回空串。
pub fn build_player_block(
    name: &str,
    _subtitle: &str,
    prompt: &str,
    relation: Option<&RelationView>,
) -> String {
    let name = name.trim();
    let prompt = prompt.trim();

    if name.is_empty() && prompt.is_empty() && relation.is_none() {
        return String::new();
    }

    let mut out = String::from("\n\n【本局必须遵守：正在与你对话的「我」】\n");
    out.push_str(
        "（本节优先于上面人设、历史台词与记忆里关于这个人的任何写法；因为此处相当于现实的基础设定，所以两者冲突时以本节为主）\n",
    );

    if !name.is_empty() {
        out.push_str(&format!("名字：{name}\n"));
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
                    "你和我的关系（必须体现在称呼、语气、动作与你的反应里，不要只是知道）：{}\n",
                    r.text
                ));
                out.push_str("关于关系：上面你和我的关系是我的基础设定以外的更为特殊的联系。\n");
            },
            RelationSource::Counterparty => {},
            RelationSource::PromptFallback => {},
        }
    }

    let has_attitude = matches!(
        relation.map(|r| r.source),
        Some(RelationSource::Speaker)
    );
    if !name.is_empty() || has_attitude {
        out.push_str("\n");
        out.push_str(&format!(
            "要求：在一般情况下，称呼 ta 时用「{name}」，该称呼优先于其他设定；在合适的台词里可以适时体现这层态度；不要复述本节内容，也不要提“设定 / 关系”这些字眼。\n"
        ));
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
            RelationSource::Speaker => format!("你对 ta 的看法：{}", view.text),
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
