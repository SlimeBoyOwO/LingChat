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

use std::time::{Duration, Instant};

use anyhow::Result;
use futures_util::StreamExt;
use tokio::sync::mpsc;

use crate::ai_service::llm::ChunkStream;
use crate::ai_service::message_system::processor::fix_ai_generated_text;

/// 一个完整情绪段的投递项：(句子文本, 有序索引, 是否为最后一项)
pub type SentenceItem = (String, usize, bool);

pub struct StreamProducer {
    llm_stream: ChunkStream,
    tx: mpsc::Sender<SentenceItem>,
}

impl StreamProducer {
    pub fn new(llm_stream: ChunkStream, tx: mpsc::Sender<SentenceItem>) -> Self {
        Self { llm_stream, tx }
    }

    /// 消耗整个 LLM 流；返回原始 accumulated_response（未拆分）。
    pub async fn run(mut self) -> Result<String> {
        let mut accumulated = String::new();
        let mut realtime_buffer = String::new();
        let mut last_display = Instant::now();

        let mut buffer = String::new();
        let mut sentence = String::new();
        let mut sentence_index: usize = 0;

        while let Some(item) = self.llm_stream.next().await {
            let chunk = item?;
            buffer.push_str(&chunk);
            accumulated.push_str(&chunk);
            realtime_buffer.push_str(&chunk);

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

                    Self::dispatch_sentence(&self.tx, &mut sentence, &mut sentence_index, false)
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
                        )
                        .await?;
                    } else {
                        // 不完整句子，等下一轮 chunk
                        break;
                    }
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
    ) -> Result<()> {
        let s = std::mem::take(sentence);
        let idx = *sentence_index;
        *sentence_index += 1;
        tx.send((s, idx, is_final))
            .await
            .map_err(|_| anyhow::anyhow!("sentence channel closed"))?;
        Ok(())
    }
}
