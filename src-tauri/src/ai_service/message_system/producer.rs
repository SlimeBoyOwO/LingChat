//! 流式生产者：从 LLM chunk 流中切分出完整的"一个情绪段"并投递到 sentence channel。
//!
//! 切分规则对标 Python `StreamProducer.run`：
//! - 遇到 `【` 进入候选状态，再遇到 `】` 闭合情绪 tag。
//! - 然后继续累积到下一个 `【` 之前的所有字符（正文 / 日文 / 动作 / 空白）。
//! - 把这一整段（含 tag）作为一个句子送出，索引递增。
//! - 结束时剩余缓冲（情绪tag + 尾部正文）单独作为最后一个句子，标记 `is_final=true`。
//!
//! 与旧版差异：
//! - Python 里还会调用一个 `num_end > 0 && buffer[num_end]=='】'` 的数字拆分分支，
//!   用于拦截 `【1】` 之类非情绪 tag 的起始符；这里等价处理（仍按 `【...】` 捕获）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::StreamExt;
use tauri::AppHandle;
use tokio::sync::{mpsc, Mutex};

use crate::ai_service::llm::{ChunkStream, LlmChunk};
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::processor::fix_ai_generated_text;

/// 一个完整情绪段的投递项：(句子文本, 有序索引, 是否为最后一项)
pub type SentenceItem = (String, usize, bool);

pub struct StreamProducer {
    llm_stream: ChunkStream,
    tx: mpsc::Sender<SentenceItem>,
    app: AppHandle,
    /// 与 consumer 共享的思考链缓冲：本轮生成的完整思考文本。
    thinking_buf: Arc<Mutex<String>>,
    /// 工具闭环执行过工具后，暂存最后一条有效句子，直到能确定真正的收尾句。
    tool_calls_seen: Arc<AtomicBool>,
}

impl StreamProducer {
    pub fn new(
        llm_stream: ChunkStream,
        tx: mpsc::Sender<SentenceItem>,
        app: AppHandle,
        thinking_buf: Arc<Mutex<String>>,
        tool_calls_seen: Arc<AtomicBool>,
    ) -> Self {
        Self {
            llm_stream,
            tx,
            app,
            thinking_buf,
            tool_calls_seen,
        }
    }

    /// 消耗整个 LLM 流；返回原始 accumulated_response（未拆分）。
    pub async fn run(mut self) -> Result<String> {
        let mut accumulated = String::new();
        let mut realtime_buffer = String::new();
        let mut last_display = Instant::now();

        let mut buffer = String::new();
        let mut sentence = String::new();
        let mut sentence_index: usize = 0;
        // 本轮回复内已投递句子的归一化集合：模型在多轮工具调用间容易把
        // 开场白复读一遍，逐字重复的句子直接丢弃（短句豁免，避免误伤语气词）。
        let mut seen_sentences = std::collections::HashSet::new();
        // 工具闭环开始后保留最后一条有效句子，流结束时再决定它是否为最终句。
        // 普通聊天没有工具调用，仍按原路径立即投递，不增加流式显示延迟。
        let mut pending_sentence: Option<String> = None;

        while let Some(item) = self.llm_stream.next().await {
            let chunk = item?;
            match chunk {
                LlmChunk::Content(text) => {
                    buffer.push_str(&text);
                    accumulated.push_str(&text);
                    realtime_buffer.push_str(&text);

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

                    // 句子切分
                    loop {
                        if !buffer.contains('【') {
                            break;
                        }

                        if !sentence.is_empty() {
                            // 已有 sentence 开头，等 】 闭合
                            let Some(end_byte) = buffer.find('】') else {
                                break;
                            };
                            let after_close = end_byte + '】'.len_utf8();
                            sentence.push_str(&buffer[..after_close]);
                            buffer.drain(..after_close);

                            // 再向后吃到下一个【
                            if let Some(next_start) = buffer.find('【') {
                                sentence.push_str(&buffer[..next_start]);
                                buffer.drain(..next_start);
                            } else {
                                sentence.push_str(&buffer);
                                buffer.clear();
                            }

                            Self::dispatch_sentence(
                                &self.tx,
                                &mut sentence,
                                &mut sentence_index,
                                &mut seen_sentences,
                                &mut pending_sentence,
                                &self.tool_calls_seen,
                            )
                            .await?;
                        } else {
                            // 寻找新情绪 tag 起点
                            let Some(start_byte) = buffer.find('【') else {
                                break;
                            };
                            let after_start = start_byte + '【'.len_utf8();
                            sentence.push_str(&buffer[..after_start]);
                            buffer.drain(..after_start);

                            // 处理 `【数字】` 这类非情绪标签（例如 【1】）
                            // 旧版检测：buffer 开头是不是全数字，若是就当作一个完整闭合句子
                            let mut num_end = 0usize;
                            for c in buffer.chars() {
                                if c.is_ascii_digit() {
                                    num_end += c.len_utf8();
                                } else {
                                    break;
                                }
                            }
                            if num_end > 0 && buffer[num_end..].chars().next() == Some('】') {
                                let close_end = num_end + '】'.len_utf8();
                                sentence.push_str(&buffer[..close_end]);
                                buffer.drain(..close_end);

                                if let Some(next_start) = buffer.find('【') {
                                    sentence.push_str(&buffer[..next_start]);
                                    buffer.drain(..next_start);
                                } else {
                                    sentence.push_str(&buffer);
                                    buffer.clear();
                                }
                                Self::dispatch_sentence(
                                    &self.tx,
                                    &mut sentence,
                                    &mut sentence_index,
                                    &mut seen_sentences,
                                    &mut pending_sentence,
                                    &self.tool_calls_seen,
                                )
                                .await?;
                            } else {
                                // 不完整句子，等下一轮 chunk
                                break;
                            }
                        }
                    }
                }
                LlmChunk::Reasoning(text) => {
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
                        events::emit_thinking_progress(&self.app, thinking_length);
                    }
                }
                LlmChunk::ToolCalls(_) => {
                    return Err(anyhow::anyhow!("工具调用片段不应进入正式回复流"));
                }
                LlmChunk::ToolCallProgress { .. } => {
                    // 参数生成进度：不进正文，由 tool_loop 直接转发为前端事件
                }
                LlmChunk::StreamEnd { .. } => {
                    // 终止信号：主对话流忽略（截断检测仅供剧本导师等工具闭环消费）。
                }
            }
        }

        // flush 剩余实时缓冲
        if !realtime_buffer.trim().is_empty() {
            print!("{}", realtime_buffer);
        }

        // 最后一个句子
        let final_content_raw = {
            let mut s = String::new();
            s.push_str(&sentence);
            s.push_str(&buffer);
            s
        };
        if !final_content_raw.is_empty() {
            let final_content = fix_ai_generated_text(&final_content_raw);
            accumulated = fix_ai_generated_text(&accumulated);

            let final_is_duplicate = !final_content.is_empty()
                && self.tool_calls_seen.load(Ordering::Acquire)
                && Self::is_duplicate(&mut seen_sentences, &final_content);
            if final_is_duplicate {
                if let Some(pending) = pending_sentence.take() {
                    tracing::info!("[dedupe] 丢弃末尾复读句子: {:.40}", final_content);
                    Self::send_sentence(&self.tx, pending, &mut sentence_index, true).await?;
                } else {
                    // 理论上工具闭环启用暂存后不会走到这里；保留完成信号优先，
                    // 避免异常格式导致前端永远停在等待状态。
                    tracing::warn!("末尾句子重复但没有可提升为最终句的暂存内容");
                    Self::send_sentence(&self.tx, final_content, &mut sentence_index, true).await?;
                }
            } else if !final_content.is_empty() {
                if let Some(pending) = pending_sentence.take() {
                    Self::send_sentence(&self.tx, pending, &mut sentence_index, false).await?;
                }
                Self::send_sentence(&self.tx, final_content, &mut sentence_index, true).await?;
            } else if let Some(pending) = pending_sentence.take() {
                Self::send_sentence(&self.tx, pending, &mut sentence_index, true).await?;
            }
        } else if let Some(pending) = pending_sentence.take() {
            Self::send_sentence(&self.tx, pending, &mut sentence_index, true).await?;
        }

        Ok(accumulated)
    }

    async fn dispatch_sentence(
        tx: &mpsc::Sender<SentenceItem>,
        sentence: &mut String,
        sentence_index: &mut usize,
        seen: &mut std::collections::HashSet<String>,
        pending: &mut Option<String>,
        tool_calls_seen: &AtomicBool,
    ) -> Result<()> {
        let s = std::mem::take(sentence);
        // 复读去重：与本轮已接收句子逐字重复（忽略空白差异）时丢弃。
        // 丢弃时不消耗索引，保证 publisher 收到的索引仍然连续。
        if Self::is_duplicate(seen, &s) {
            tracing::info!("[dedupe] 丢弃复读句子: {:.40}", s);
            return Ok(());
        }

        if tool_calls_seen.load(Ordering::Acquire) {
            if let Some(previous) = pending.replace(s) {
                Self::send_sentence(tx, previous, sentence_index, false).await?;
            }
            return Ok(());
        }

        Self::send_sentence(tx, s, sentence_index, false).await
    }

    async fn send_sentence(
        tx: &mpsc::Sender<SentenceItem>,
        sentence: String,
        sentence_index: &mut usize,
        is_final: bool,
    ) -> Result<()> {
        let idx = *sentence_index;
        *sentence_index += 1;
        tx.send((sentence, idx, is_final))
            .await
            .map_err(|_| anyhow::anyhow!("sentence channel closed"))?;
        Ok(())
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
