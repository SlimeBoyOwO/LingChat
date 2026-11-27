//! 信纸工具：角色把画、公式、代码、计划书等写成 Markdown 页，前端在人物旁边展示。

use async_trait::async_trait;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::{ensure_no_args, game_status_handle};
use crate::ai_service::types::ToolDefinition;
use crate::db::managers::save_letter_repo::{
    SaveLetterRepo, new_letter_id, read_letter, write_letter,
};

const LETTER_LIST_PROMPT: &str = "列出当前存档写过的信纸页（页码、标题、字数），页码从 1 开始";

const LETTER_READ_PROMPT: &str = "读取信纸某一页的完整 Markdown 内容，页码从 1 开始";

const LETTER_WRITE_PROMPT: &str = "在信纸上新写一页并立刻展示在你身旁给对方看。内容是 Markdown，\
适合对白里说不清、需要拿出来看的东西：画画、数学公式、代码、表格、详细计划书、清单、书信等。\
画画用 ```svg 代码块写一个完整的 <svg> 元素（带 viewBox）；公式用 KaTeX 语法，行内 $...$，独立成行 $$...$$；\
代码用带语言标注的代码块；图片用 ![说明](https 链接)。除 svg 代码块外不支持原始 HTML。\
信纸已经展示在对方眼前，对白里只需自然提一句";

const LETTER_UPDATE_PROMPT: &str = "整页改写信纸某一页（页码从 1 开始，可先用 letter_list 查看），\
改完立刻展示给对方。内容格式同 letter_write";

/// 前端监听的展示事件，载荷里的 page 从 0 开始。
const LETTER_SHOW_EVENT: &str = "letter:show";

fn require_object<'a>(
    arguments: &'a Value,
    tool: &str,
) -> Result<&'a Map<String, Value>, ToolError> {
    arguments
        .as_object()
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 参数必须是 JSON object")))
}

fn parse_content(obj: &Map<String, Value>, tool: &str) -> Result<String, ToolError> {
    obj.get("content")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 需要非空的 content")))
}

/// 解析 1 起的页码并换成下标；模型偶尔把数字写成字符串或浮点。
fn parse_page(obj: &Map<String, Value>, tool: &str, total: usize) -> Result<usize, ToolError> {
    let page = obj
        .get("page")
        .and_then(|v| match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        })
        .ok_or_else(|| ToolError::InvalidArguments(format!("{tool} 需要 page")))?;
    if page < 1.0 || page.fract() != 0.0 || page as usize > total {
        return Err(ToolError::InvalidArguments(format!(
            "页码 {page} 不存在，当前共有 {total} 页"
        )));
    }
    Ok(page as usize - 1)
}

fn letter_title(content: &str) -> String {
    let line = content
        .lines()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    line.chars().take(40).collect()
}

fn exec_err(e: impl std::fmt::Display) -> ToolError {
    ToolError::Execution(e.to_string())
}

fn emit_show(app: &AppHandle, page: usize) {
    if let Err(e) = app.emit(LETTER_SHOW_EVENT, json!({ "page": page })) {
        tracing::warn!("广播信纸展示事件失败: {e}");
    }
}

async fn session_letters(app: &AppHandle) -> Vec<String> {
    game_status_handle(app).await.lock().await.letters.clone()
}

/// letter_list：列出当前会话的信纸页。
pub struct LetterList;

#[async_trait]
impl Tool for LetterList {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "letter_list",
            LETTER_LIST_PROMPT,
            json!({
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": false
            }),
        )
        .with_rag_examples(["之前写的信纸呢", "你都写过哪几页", "翻翻信纸"])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        ensure_no_args(&arguments, "letter_list").map_err(ToolError::Execution)?;
        let app = context.require_app()?;
        let pages: Vec<Value> = session_letters(&app)
            .await
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let content = read_letter(id).unwrap_or_default();
                json!({
                    "page": index + 1,
                    "title": letter_title(&content),
                    "chars": content.chars().count(),
                })
            })
            .collect();
        Ok(json!({ "ok": true, "total": pages.len(), "pages": pages }))
    }
}

/// letter_read：读取某一页。
pub struct LetterRead;

#[async_trait]
impl Tool for LetterRead {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "letter_read",
            LETTER_READ_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "page": {"type": "integer", "description": "页码，从 1 开始"}
                },
                "required": ["page"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples(["看看信纸第一页写了什么", "把上一页信纸读一下"])
    }

    fn lenient_arguments(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "letter_read")?;
        let app = context.require_app()?;
        let letters = session_letters(&app).await;
        let index = parse_page(obj, "letter_read", letters.len())?;
        let content = read_letter(&letters[index]).map_err(exec_err)?;
        Ok(json!({ "ok": true, "page": index + 1, "content": content }))
    }
}

/// letter_write：新写一页并展示。
pub struct LetterWrite;

#[async_trait]
impl Tool for LetterWrite {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "letter_write",
            LETTER_WRITE_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "content": {"type": "string", "description": "整页 Markdown 内容"}
                },
                "required": ["content"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples([
            "给我画一只小猫",
            "把这个公式写出来给我看",
            "写段代码演示一下",
            "帮我列一份详细的学习计划",
            "给我写封信吧",
        ])
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "letter_write")?;
        let content = parse_content(obj, "letter_write")?;
        let app = context.require_app()?;
        let db = app.state::<AppState>().db.clone();

        let id = new_letter_id();
        write_letter(&id, &content).map_err(exec_err)?;
        let index = {
            let gs = game_status_handle(&app).await;
            let mut gs = gs.lock().await;
            gs.letters.push(id);
            if let Some(save_id) = gs.active_save_id
                && let Err(e) = SaveLetterRepo::replace_all(&db, save_id, &gs.letters).await
            {
                // 正文文件留给启动清理
                gs.letters.pop();
                return Err(exec_err(e));
            }
            gs.letters.len() - 1
        };
        emit_show(&app, index);
        Ok(json!({ "ok": true, "page": index + 1 }))
    }
}

/// letter_update：整页改写并展示。
pub struct LetterUpdate;

#[async_trait]
impl Tool for LetterUpdate {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            "letter_update",
            LETTER_UPDATE_PROMPT,
            json!({
                "type": "object",
                "properties": {
                    "page": {"type": "integer", "description": "页码，从 1 开始"},
                    "content": {"type": "string", "description": "改写后的整页 Markdown 内容"}
                },
                "required": ["page", "content"],
                "additionalProperties": false
            }),
        )
        .with_rag_examples(["把刚才那页改一下", "在信纸上补充几句", "画错了重画"])
    }

    fn lenient_arguments(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let obj = require_object(&arguments, "letter_update")?;
        let content = parse_content(obj, "letter_update")?;
        let app = context.require_app()?;
        let letters = session_letters(&app).await;
        let index = parse_page(obj, "letter_update", letters.len())?;
        write_letter(&letters[index], &content).map_err(exec_err)?;
        emit_show(&app, index);
        Ok(json!({ "ok": true, "page": index + 1 }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_parsing_accepts_loose_numbers() {
        let obj = json!({"page": "2"});
        assert_eq!(parse_page(obj.as_object().unwrap(), "t", 3).unwrap(), 1);
        let obj = json!({"page": 3.0});
        assert_eq!(parse_page(obj.as_object().unwrap(), "t", 3).unwrap(), 2);
        let obj = json!({"page": 4});
        assert!(parse_page(obj.as_object().unwrap(), "t", 3).is_err());
        let obj = json!({"page": 0});
        assert!(parse_page(obj.as_object().unwrap(), "t", 3).is_err());
    }

    #[test]
    fn title_skips_heading_marks() {
        assert_eq!(letter_title("\n## 学习计划\n内容"), "学习计划");
        assert_eq!(letter_title(""), "");
    }
}
