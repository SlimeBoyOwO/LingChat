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
}

impl StreamProducer {
    pub fn new(
        llm_stream: ChunkStream,
        tx: mpsc::Sender<SentenceItem>,
        app: AppHandle,
        thinking_buf: Arc<Mutex<String>>,
    ) -> Self {
        Self {
            llm_stream,
            tx,
            app,
            thinking_buf,
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
                                false,
                                &mut seen_sentences,
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
                                    false,
                                    &mut seen_sentences,
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

            self.tx
                .send((final_content, sentence_index, true))
                .await
                .map_err(|_| anyhow::anyhow!("sentence channel closed"))?;
        }

        Ok(accumulated)
    }

    async fn dispatch_sentence(
        tx: &mpsc::Sender<SentenceItem>,
        sentence: &mut String,
        sentence_index: &mut usize,
        is_final: bool,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        let s = std::mem::take(sentence);
        // 复读去重：非最终句与本轮已投递句子逐字重复（忽略空白差异）时丢弃。
        // 丢弃时不消耗索引，保证 publisher 收到的索引仍然连续；最终句始终放行，
        // 确保前端一定能收到 is_final 完成信号。
        if !is_final && Self::is_duplicate(seen, &s) {
            tracing::info!("[dedupe] 丢弃复读句子: {:.40}", s);
            return Ok(());
        }
        let idx = *sentence_index;
        *sentence_index += 1;
        tx.send((s, idx, is_final))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_detection_ignores_whitespace() {
        let mut seen = std::collections::HashSet::new();
        let first = "【高兴】\n好哒用户酱，进入工程创建阶段～\n<わかりました>";
        assert!(!StreamProducer::is_duplicate(&mut seen, first));
        // 逐字复读（仅空白有差异）会被识别并丢弃
        let repeated = "【高兴】 好哒用户酱，进入工程创建阶段～ \n<わかりました>\n";
        assert!(StreamProducer::is_duplicate(&mut seen, repeated));
    }

    #[test]
    fn short_sentences_are_exempt() {
        let mut seen = std::collections::HashSet::new();
        assert!(!StreamProducer::is_duplicate(&mut seen, "【高兴】嗯"));
        assert!(!StreamProducer::is_duplicate(&mut seen, "【高兴】嗯"));
    }

    #[test]
    fn distinct_sentences_pass() {
        let mut seen = std::collections::HashSet::new();
        assert!(!StreamProducer::is_duplicate(&mut seen, "【认真】先读取参考文件再动手写"));
        assert!(!StreamProducer::is_duplicate(&mut seen, "【高兴】已经写好落盘啦"));
    }
}
