//! 流式生产者：从 LLM chunk 流中切分出完整的"一个情绪段"并投递到 sentence channel。
//!
//! 切分规则对标 Python `StreamProducer.run`：
//! - 遇到 `【` 进入候选状态，再遇到 `】` 闭合情绪 tag。
//! - 然后继续累积到下一个 `【` 之前的所有字符（正文 / 日文 / 动作 / 空白）。
//! - 把这一整段（含 tag）作为一个句子送出，索引递增。
//! - 结束时剩余缓冲（情绪tag + 尾部正文）单独作为最后一个句子，标记 `is_final=true`。
//!
//! 分句只依赖标签边界，不依赖网络 chunk 大小；`【1】` 等数字标签同样处理。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::StreamExt;
use tauri::AppHandle;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::ai_service::llm::LlmChunk;
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::processor::{
    ends_with_sentence_end, find_turn_end, fix_ai_generated_text,
};

/// `join_sentences` 时最多扣住多少字：模型一直不给句末标点时不能无限等下去，
/// 超过就按现状投递（宁可句子被切开，也不能压着不发）。
const MAX_HELD_CHARS: usize = 60;

/// 呈现层流协议：provider chunk 原样透传，另加一个由工具闭环插入的"执行前栅栏"。
///
/// 栅栏不改变任何 provider 语义，只为"前导台词必须先于工具事件呈现"提供一个
/// 可等待的同步点——工具事件是裸 `app.emit`，而 `ai:reply` 要经过 consumer 的
/// 翻译/TTS 才由 publisher 发出，两者原本没有任何顺序关系。
pub enum PresentationChunk {
    Chunk(LlmChunk),
    /// 工具即将执行：携带一个 ack，publisher 按序处理完更早的项后回传
    /// "栅栏之前是否已发布过正式回复"。
    BeforeTools(oneshot::Sender<bool>),
}

pub type PresentationStream =
    std::pin::Pin<Box<dyn futures_util::Stream<Item = Result<PresentationChunk>> + Send>>;

/// 投递到 sentence channel 的工作项。
pub enum SentenceItem {
    /// 一个完整情绪段：(句子文本, 有序索引, 是否为最后一项)
    Reply(String, usize, bool),
    /// 呈现栅栏：**自身占用一个索引**，publisher 按序推进到它时才回 ack。
    BeforeTools {
        index: usize,
        ack: oneshot::Sender<bool>,
    },
}

/// 一轮生成的出口。
pub struct ProducerOutput {
    /// 模型原始输出（未拆分）。
    pub accumulated: String,
    /// 本轮是否发出过 `is_final=true` 的句子。
    pub sent_final: bool,
    /// 每道工具栅栏对应一组「栅栏之前发出的句子序号」。
    /// 空前导轮也保留一个空组，保证与工具轮按顺序一一对应。
    pub preamble_groups: Vec<Vec<usize>>,
}

pub struct StreamProducer {
    llm_stream: PresentationStream,
    tx: mpsc::Sender<SentenceItem>,
    /// 仅用于思考链进度上报；测试里为 `None`（单元测试拿不到真实 `AppHandle`）。
    app: Option<AppHandle>,
    /// 与 consumer 共享的思考链缓冲：本轮生成的完整思考文本。
    thinking_buf: Arc<Mutex<String>>,
    /// 工具闭环执行过工具后，暂存最后一条有效句子，直到能确定真正的收尾句。
    tool_calls_seen: Arc<AtomicBool>,
    /// TTS 按整句合成（sherpa-onnx 不看情绪）时置 true：把"还没收句"的片段先扣住，
    /// 等下一段送到时拼回一句再投递，免得同一句话被两次推理、语调和停顿各自重置。
    join_sentences: bool,
}

impl StreamProducer {
    pub fn new(
        llm_stream: PresentationStream,
        tx: mpsc::Sender<SentenceItem>,
        app: AppHandle,
        thinking_buf: Arc<Mutex<String>>,
        tool_calls_seen: Arc<AtomicBool>,
        join_sentences: bool,
    ) -> Self {
        Self {
            llm_stream,
            tx,
            app: Some(app),
            thinking_buf,
            tool_calls_seen,
            join_sentences,
        }
    }

    /// 消耗整个 LLM 流；返回原始 accumulated_response（未拆分）与是否已发出收尾句。
    pub async fn run(mut self) -> Result<ProducerOutput> {
        let mut accumulated = String::new();
        let mut realtime_buffer = String::new();
        let mut last_display = Instant::now();

        let mut buffer = String::new();
        let mut sentence_index: usize = 0;
        // 本轮回复内已投递句子的归一化集合：模型在多轮工具调用间容易把
        // 开场白复读一遍，逐字重复的句子直接丢弃（短句豁免，避免误伤语气词）。
        let mut seen_sentences = std::collections::HashSet::new();
        // 工具闭环开始后保留最后一条有效句子，流结束时再决定它是否为最终句。
        // 普通聊天没有工具调用，仍按原路径立即投递，不增加流式显示延迟。
        let mut pending_sentence: Option<String> = None;
        // `join_sentences` 时扣住的"还没收句"的片段：等下一段送到拼回一句再投递。
        // 它没有占用句子序号，所以必须在栅栏与流结束时补投，否则文本会丢。
        let mut held_fragment: Option<String> = None;
        // 本轮是否已发出收尾句。工具闭环下"工具后无新正文"会让这条保持 false，
        // 调用方据此补一次收尾（否则前端永远停在等待态）。
        let mut sent_final = false;
        // 本轮（两道栅栏之间）发出的句子序号。栅栏到来时整组转入 preamble_groups，
        // 供回填阶段按句子顺序重排前导行、并定位载体行的插入位置。
        let mut round_indices: Vec<usize> = Vec::new();
        let mut preamble_groups: Vec<Vec<usize>> = Vec::new();
        // 本轮是否已经出现过回合结束边界。出现后其内容一律不再解析——模型常在说完话之后
        // 继续替玩家发言（`\n\n\nuser\n…`），那些内容一旦落进台词表就会被记忆反复复读。
        let mut turn_ended = false;
        let mut logged_discard = false;

        while let Some(item) = self.llm_stream.next().await {
            match item? {
                PresentationChunk::Chunk(LlmChunk::Content(text)) => {
                    if turn_ended {
                        // 边界之后仍有内容：丢弃，并报一次警（便于定位写偏的回合）
                        Self::warn_discarded(&mut logged_discard, &text);
                        continue;
                    }

                    buffer.push_str(&text);
                    accumulated.push_str(&text);
                    realtime_buffer.push_str(&text);

                    // 结束边界：buffer 是 accumulated 的后缀，两边的偏移量对得上
                    if let Some((pos, kind)) = find_turn_end(&buffer) {
                        let consumed = accumulated.len() - buffer.len();
                        // 同一 chunk 里边界之后可能还跟着内容，先取出来（借用不能跨过 truncate）
                        let tail = buffer[pos..].to_string();
                        accumulated.truncate(consumed + pos);
                        buffer.truncate(pos);
                        turn_ended = true;
                        tracing::info!("[TurnEnd] 命中{kind}，截断于第 {pos} 字节");
                        Self::warn_discarded(&mut logged_discard, &tail);
                    }

                    let now = Instant::now();
                    if realtime_buffer.chars().count() >= 3
                        || now.duration_since(last_display) > Duration::from_millis(100)
                        || realtime_buffer.contains('\n')
                    {
                        if !realtime_buffer.trim().is_empty() {
                            print!("{}", realtime_buffer);
                        }
                        realtime_buffer.clear();
                        last_display = now;
                    }

                    // 当前标签闭合且下一个标签已开始，才说明当前段的正文完整。
                    // 一次耗尽所有完整段，不能把网络 chunk 边界当成句子边界。
                    while let Some(start) = buffer.find('【') {
                        let after_start = start + '【'.len_utf8();
                        let Some(close) = buffer[after_start..].find('】') else {
                            break;
                        };
                        let after_close = after_start + close + '】'.len_utf8();
                        let Some(next_start) = buffer[after_close..].find('【') else {
                            break;
                        };
                        let boundary = after_close + next_start;
                        let mut sentence = buffer.drain(..boundary).collect();
                        Self::dispatch_sentence(
                            &self.tx,
                            &mut sentence,
                            &mut sentence_index,
                            &mut round_indices,
                            &mut seen_sentences,
                            &mut pending_sentence,
                            &mut held_fragment,
                            &self.tool_calls_seen,
                            self.join_sentences,
                        )
                        .await?;
                    }
                },
                PresentationChunk::Chunk(LlmChunk::Reasoning(text)) => {
                    // 思考链内容：累积进共享缓冲（供 consumer 挂载到台词行），
                    // 并实时统计字数通知前端，但不加入正式回复。
                    if !text.is_empty() {
                        let mut buf = self.thinking_buf.lock().await;
                        // 部分供应商（kimi_code / genai）会在流结束时重发完整快照，
                        // 若新 chunk 以已有内容为前缀则整体替换，避免重复累积。
                        if text.starts_with(buf.as_str()) {
                            *buf = text;
                        } else {
                            buf.push_str(&text);
                        }
                        let thinking_length = buf.chars().count();
                        drop(buf);
                        if let Some(app) = &self.app {
                            events::emit_thinking_progress(app, thinking_length);
                        }
                    }
                },
                PresentationChunk::Chunk(LlmChunk::ToolCalls(_)) => {
                    return Err(anyhow::anyhow!("工具调用片段不应进入正式回复流"));
                },
                PresentationChunk::Chunk(LlmChunk::ToolCallProgress { .. }) => {
                    // 参数生成进度：不进正文，由 tool_loop 直接转发为前端事件
                },
                PresentationChunk::Chunk(LlmChunk::StreamEnd { .. }) => {
                    // 终止信号：主对话流忽略（截断检测仅供剧本导师等工具闭环消费）。
                },
                PresentationChunk::BeforeTools(ack) => {
                    // 栅栏是「一轮 LLM 生成」的边界：越过它的内容属于工具执行后的新一轮，
                    // 结束标记的判定随之重置（否则模型在前导台词后写了边界，工具后的话就全丢了）。
                    turn_ended = false;
                    logged_discard = false;

                    // 工具即将执行：把已缓冲的前导文本立刻以非最终句发出，再让栅栏
                    // 自己占用一个序号——publisher 按序推进到该序号时才回 ack，从而保证
                    // "前导台词的 ai:reply 已 emit"严格先于"工具事件 emit"。
                    if let Some(pending) = pending_sentence.take() {
                        Self::send_sentence(
                            &self.tx,
                            pending,
                            &mut sentence_index,
                            &mut round_indices,
                            false,
                        )
                        .await?;
                    }

                    let mut preamble = std::mem::take(&mut buffer);
                    // 扣住的"还没收句"的片段排在缓冲内容之前，一起作为前导台词发出，
                    // 否则它既不占序号又留在手里，最后会凭空消失。
                    if let Some(held) = held_fragment.take() {
                        preamble = format!("{held}{preamble}");
                    }
                    let preamble = fix_ai_generated_text(&preamble);
                    // `is_duplicate` 会顺带把文本记入 `seen_sentences`：工具后模型若原样
                    // 复读这段前导，EOF 收尾时才认得出它是重复（否则会多显示一条）。
                    if !preamble.is_empty() && !Self::is_duplicate(&mut seen_sentences, &preamble) {
                        Self::send_sentence(
                            &self.tx,
                            preamble,
                            &mut sentence_index,
                            &mut round_indices,
                            false,
                        )
                        .await?;
                    }

                    // 到栅栏为止发的都是「工具前说的话」：整组交给回填阶段，
                    // 由它按句子序号重排这些行并定位载体行。
                    preamble_groups.push(std::mem::take(&mut round_indices));

                    // 栅栏自身占一个索引，且不经 `send_sentence`（item 类型不同）。
                    let index = sentence_index;
                    sentence_index += 1;
                    self.tx
                        .send(SentenceItem::BeforeTools { index, ack })
                        .await
                        .map_err(|_| anyhow::anyhow!("sentence channel closed"))?;
                },
            }
        }

        // flush 剩余实时缓冲
        if !realtime_buffer.trim().is_empty() {
            print!("{}", realtime_buffer);
        }

        // 最后一个句子（被扣住的片段排在缓冲内容之前，拼成同一句）
        let mut final_content_raw = buffer;
        if let Some(held) = held_fragment.take() {
            final_content_raw = format!("{held}{final_content_raw}");
        }
        if !final_content_raw.is_empty() {
            let final_content = fix_ai_generated_text(&final_content_raw);
            accumulated = fix_ai_generated_text(&accumulated);

            let final_is_duplicate = !final_content.is_empty()
                && self.tool_calls_seen.load(Ordering::Acquire)
                && Self::is_duplicate(&mut seen_sentences, &final_content);
            if final_is_duplicate {
                if let Some(pending) = pending_sentence.take() {
                    tracing::info!("[dedupe] 丢弃末尾复读句子: {:.40}", final_content);
                    Self::send_sentence(
                        &self.tx,
                        pending,
                        &mut sentence_index,
                        &mut round_indices,
                        true,
                    )
                    .await?;
                    sent_final = true;
                } else {
                    // 工具后模型原样复读前导、且没有产出新正文。**不重放**——重放会让
                    // 用户看到同一句话两次；交给调用方按 `sent_final == false` 收尾。
                    tracing::info!(
                        "[dedupe] 末尾复读且无待提升句，本轮不发收尾句: {:.40}",
                        final_content
                    );
                }
            } else if !final_content.is_empty() {
                if let Some(pending) = pending_sentence.take() {
                    Self::send_sentence(
                        &self.tx,
                        pending,
                        &mut sentence_index,
                        &mut round_indices,
                        false,
                    )
                    .await?;
                }
                Self::send_sentence(
                    &self.tx,
                    final_content,
                    &mut sentence_index,
                    &mut round_indices,
                    true,
                )
                .await?;
                sent_final = true;
            } else if let Some(pending) = pending_sentence.take() {
                Self::send_sentence(
                    &self.tx,
                    pending,
                    &mut sentence_index,
                    &mut round_indices,
                    true,
                )
                .await?;
                sent_final = true;
            }
        } else if let Some(pending) = pending_sentence.take() {
            Self::send_sentence(
                &self.tx,
                pending,
                &mut sentence_index,
                &mut round_indices,
                true,
            )
            .await?;
            sent_final = true;
        }

        Ok(ProducerOutput {
            accumulated,
            sent_final,
            preamble_groups,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn dispatch_sentence(
        tx: &mpsc::Sender<SentenceItem>,
        sentence: &mut String,
        sentence_index: &mut usize,
        round_indices: &mut Vec<usize>,
        seen: &mut std::collections::HashSet<String>,
        pending: &mut Option<String>,
        held: &mut Option<String>,
        tool_calls_seen: &AtomicBool,
        join_sentences: bool,
    ) -> Result<()> {
        let mut s = std::mem::take(sentence);

        // 上一段没说完时先并进来：`【开心】哇，真的吗` + `【兴奋】那我们走！` 本来是一句。
        if let Some(previous) = held.take() {
            let mut joined = previous;
            joined.push_str(&s);
            s = joined;
        }

        // 还没收到句末标点、也没长到必须投递：先扣住等下一段，别让 TTS 分两次读。
        if join_sentences && s.chars().count() < MAX_HELD_CHARS && !ends_with_sentence_end(&s) {
            *held = Some(s);
            return Ok(());
        }

        // 复读去重：与本轮已接收句子逐字重复（忽略空白差异）时丢弃。
        // 丢弃时不消耗索引，保证 publisher 收到的索引仍然连续。
        if Self::is_duplicate(seen, &s) {
            tracing::info!("[dedupe] 丢弃复读句子: {:.40}", s);
            return Ok(());
        }

        if tool_calls_seen.load(Ordering::Acquire) {
            if let Some(previous) = pending.replace(s) {
                Self::send_sentence(tx, previous, sentence_index, round_indices, false).await?;
            }
            return Ok(());
        }

        Self::send_sentence(tx, s, sentence_index, round_indices, false).await
    }

    async fn send_sentence(
        tx: &mpsc::Sender<SentenceItem>,
        sentence: String,
        sentence_index: &mut usize,
        round_indices: &mut Vec<usize>,
        is_final: bool,
    ) -> Result<()> {
        let idx = *sentence_index;
        *sentence_index += 1;
        round_indices.push(idx);
        tx.send(SentenceItem::Reply(sentence, idx, is_final))
            .await
            .map_err(|_| anyhow::anyhow!("sentence channel closed"))?;
        Ok(())
    }

    /// 越界内容被丢弃时报警，同一轮内只报一次（避免刷屏）；没有实质内容就不报。
    fn warn_discarded(logged: &mut bool, text: &str) {
        let trimmed = text.trim();
        if *logged || trimmed.is_empty() {
            return;
        }
        *logged = true;
        let preview: String = trimmed.chars().take(80).collect();
        tracing::warn!("[TurnEnd] 回复越过结束边界仍有内容，已丢弃: {preview}");
    }

    /// 判断句子是否是本轮回复内的逐字复读（忽略所有空白字符）。
    /// 归一化后不足 8 个字符的短句不去重，避免误伤「嗯」「好哒」等合法重复。
    fn is_duplicate(seen: &mut std::collections::HashSet<String>, sentence: &str) -> bool {
        let normalized: String = sentence.chars().filter(|c| !c.is_whitespace()).collect();
        if normalized.chars().count() < 8 {
            return false;
        }
        !seen.insert(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dispatch_sentence` 的一组可变状态，省得每个用例都摊开写。
    struct Dispatcher {
        tx: mpsc::Sender<SentenceItem>,
        rx: mpsc::Receiver<SentenceItem>,
        index: usize,
        round: Vec<usize>,
        seen: std::collections::HashSet<String>,
        pending: Option<String>,
        held: Option<String>,
        tool_calls_seen: AtomicBool,
    }

    impl Dispatcher {
        fn new() -> Self {
            let (tx, rx) = mpsc::channel(8);
            Self {
                tx,
                rx,
                index: 0,
                round: Vec::new(),
                seen: std::collections::HashSet::new(),
                pending: None,
                held: None,
                tool_calls_seen: AtomicBool::new(false),
            }
        }

        async fn dispatch(&mut self, sentence: &str, join: bool) {
            let mut sentence = sentence.to_string();
            StreamProducer::dispatch_sentence(
                &self.tx,
                &mut sentence,
                &mut self.index,
                &mut self.round,
                &mut self.seen,
                &mut self.pending,
                &mut self.held,
                &self.tool_calls_seen,
                join,
            )
            .await
            .expect("投递失败");
        }

        /// 取出一条已投递的回复（没有则 None）。
        fn take(&mut self) -> Option<(String, usize, bool)> {
            match self.rx.try_recv() {
                Ok(SentenceItem::Reply(text, index, is_final)) => Some((text, index, is_final)),
                Ok(SentenceItem::BeforeTools { .. }) => panic!("不应收到栅栏"),
                Err(_) => None,
            }
        }
    }

    /// 开启合句时：没收到句末标点的片段先扣住，等下一段拼回一句再投递；关掉时逐个投递。
    #[tokio::test]
    async fn holds_unfinished_sentences_when_joining() {
        let mut d = Dispatcher::new();

        d.dispatch("【开心】哇，真的吗", true).await;
        assert!(d.take().is_none(), "没收句的片段应被扣住");
        assert_eq!(d.held.as_deref(), Some("【开心】哇，真的吗"));
        assert_eq!(d.index, 0, "扣住不占用句子序号");

        d.dispatch("【兴奋】那我们赶紧出发吧！", true).await;
        let (text, idx, is_final) = d.take().expect("拼回的整句应被投递");
        assert_eq!(text, "【开心】哇，真的吗【兴奋】那我们赶紧出发吧！");
        assert_eq!(idx, 0);
        assert!(!is_final);
        assert!(d.held.is_none());

        // 已经收句的片段直接投递。
        d.dispatch("【平静】就这样吧。", true).await;
        assert_eq!(d.take().unwrap().0, "【平静】就这样吧。");
        assert!(d.held.is_none());

        // 云端 TTS（不做合句）行为不变：片段照样逐个投递。
        d.dispatch("【开心】哇，真的吗", false).await;
        assert_eq!(d.take().unwrap().0, "【开心】哇，真的吗");
        assert!(d.held.is_none());
    }

    /// 模型一直不给句末标点时不能无限扣着不发。
    #[tokio::test]
    async fn flushes_held_fragment_when_too_long() {
        let mut d = Dispatcher::new();

        let long = format!("【平静】{}", "嗯".repeat(MAX_HELD_CHARS));
        d.dispatch(&long, true).await;
        assert_eq!(d.take().expect("超长必须直接投递").0, long);
        assert!(d.held.is_none());
    }
}
