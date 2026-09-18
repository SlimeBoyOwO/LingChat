//! issue #784：工具调用流式顺序的契约测试。
//!
//! 这里驱动**真实的** `StreamProducer` 与 `publish_ordered` 组合（而不是复制一份
//! 逻辑），用一条手写流复刻 `tool_loop` 的关键行为：正文 chunk → 置位
//! `tool_calls_seen` → 插入呈现栅栏并等待 ack → 执行"工具" → 续写正文。
//!
//! 不覆盖的部分（只能靠真机验证）：真实 consumer 池（翻译/TTS 的耗时）、
//! `stream_with_tool_loop` 是否真的 yield 了栅栏、以及前端接收侧的顺序。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::generator::{PublishItem, publish_ordered};
use super::producer::{PresentationChunk, SentenceItem, StreamProducer};
use super::responses::ReplyResponse;
use crate::ai_service::llm::LlmChunk;

/// 跑一轮工具闭环，返回 `(发布轨迹, producer 是否发出终句, publisher 是否发出终句)`。
///
/// 发布轨迹里 `"TOOL"` 代表"工具开始执行"这一刻（真实实现里就是工具事件的 emit 点）。
async fn run_round(preamble: &str, suffix: &str) -> (Vec<String>, bool, bool) {
    let preamble = preamble.to_owned();
    let suffix = suffix.to_owned();
    let trace = Arc::new(Mutex::new(Vec::<String>::new()));
    let tool_trace = trace.clone();
    let seen = Arc::new(AtomicBool::new(false));
    let tool_seen = seen.clone();

    let stream = async_stream::try_stream! {
        if !preamble.is_empty() {
            yield PresentationChunk::Chunk(LlmChunk::Content(preamble));
        }
        // 与生产一致：tool_loop 在插入栅栏**之前**就置位该标志（`tool_loop.rs:195`），
        // 所以栅栏分支必须绕过 dispatch_sentence，否则前导会被扣进 pending_sentence。
        tool_seen.store(true, Ordering::Release);
        let (tx, rx) = oneshot::channel();
        yield PresentationChunk::BeforeTools(tx);
        rx.await.expect("publisher 必须回 ack");
        tool_trace.lock().unwrap().push("TOOL".into());
        if !suffix.is_empty() {
            yield PresentationChunk::Chunk(LlmChunk::Content(suffix));
        }
    };

    let (tx, mut rx) = mpsc::channel(4);
    let producer = StreamProducer::without_app(Box::pin(stream), tx, seen);

    let (pub_tx, pub_rx) = mpsc::channel(4);
    let publish_trace = trace.clone();
    let publisher = tokio::spawn(publish_ordered(pub_rx, move |reply| {
        publish_trace
            .lock()
            .unwrap()
            .push(format!("{}:{}", reply.message, reply.is_final));
        Ok(())
    }));

    // 假 consumer：只做 item 映射，不做翻译/TTS（真实 consumer 的耗时不在本测试范围）。
    let consumer = tokio::spawn(async move {
        while let Some(item) = rx.recv().await {
            let item = match item {
                SentenceItem::Reply(text, index, is_final) => PublishItem::Reply {
                    index,
                    response: Some(ReplyResponse {
                        message: text,
                        is_final,
                        ..ReplyResponse::new_reply()
                    }),
                },
                SentenceItem::BeforeTools { index, ack } => PublishItem::BeforeTools { index, ack },
            };
            if pub_tx.send(item).await.is_err() {
                break;
            }
        }
    });

    let output = tokio::time::timeout(Duration::from_secs(3), producer.run())
        .await
        .expect("producer 不得挂死")
        .expect("producer 失败");
    consumer.await.unwrap();
    let published = publisher.await.unwrap();
    let result = trace.lock().unwrap().clone();
    (result, output.sent_final, published)
}

/// 栅栏之前必须有正确顺序的（非终句）前导，栅栏之后才有终句——
/// 三种前导形态（无情绪 tag / 带 tag / 两段带 tag）都要成立。
#[tokio::test]
async fn preamble_is_published_before_tool_and_suffix_is_final() {
    for preamble in [
        "我先查询一下。",
        "【正常】我先查询一下。",
        "【正常】先说明。【开心】然后查询。",
    ] {
        let (trace, sent, published) = run_round(preamble, "【正常】查询完成。").await;
        let tool = trace.iter().position(|s| s == "TOOL").unwrap();
        assert!(tool > 0, "工具不得抢在前导之前: {trace:?}");
        assert!(
            trace[..tool].iter().all(|s| s.ends_with(":false")),
            "栅栏之前不得出现终句: {trace:?}"
        );
        assert!(trace.last().unwrap().ends_with(":true"), "{trace:?}");
        assert!(sent && published, "{trace:?}");
    }
}

/// 纯工具轮（模型没有前导正文）：栅栏同样必须被 ack，不能死等。
#[tokio::test]
async fn tool_only_does_not_wait_for_nonexistent_preamble() {
    let (trace, sent, published) = run_round("", "完成").await;
    assert_eq!(trace[0], "TOOL", "{trace:?}");
    assert!(sent && published, "{trace:?}");
}

/// 工具后没有新正文、或原样复读前导时：不得把前导重放为终句
/// （那会让用户看到同一句话两次），改由调用方温和复位。
#[tokio::test]
async fn empty_or_repeated_post_tool_content_does_not_replay_preamble_as_final() {
    let preamble = "【正常】我先查询一下资料。";
    for suffix in ["", preamble] {
        let (trace, sent, published) = run_round(preamble, suffix).await;
        assert_eq!(trace.len(), 2, "只应有前导与工具两条: {trace:?}");
        assert!(
            !sent && !published,
            "不得重放前导为终句，调用方据此复位: {trace:?}"
        );
    }
}

/// 栅栏要等**所有**更小索引处理完——包括被判定为 `None`（处理失败/丢弃）的那些。
#[tokio::test]
async fn publisher_fence_waits_for_earlier_indexes_including_skipped_results() {
    let (tx, rx) = mpsc::channel(4);
    let (ack, mut done) = oneshot::channel();
    tx.send(PublishItem::BeforeTools { index: 2, ack })
        .await
        .unwrap();
    // 索引 1 是"处理过但无内容"的结果：它同样必须先被消费掉。
    tx.send(PublishItem::Reply {
        index: 1,
        response: None,
    })
    .await
    .unwrap();
    let task = tokio::spawn(publish_ordered(rx, |_| Ok(())));
    assert!(matches!(
        done.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    tx.send(PublishItem::Reply {
        index: 0,
        response: Some(ReplyResponse::new_reply()),
    })
    .await
    .unwrap();
    // 索引 0 已发布，因此栅栏回 ack=true；索引 1/2 被依次推进。
    assert!(done.await.unwrap());
    drop(tx);
    // 通道关闭且从未发出终句 → 返回 false。
    assert!(!task.await.unwrap());
}

/// 发布失败时必须丢弃栅栏（工具侧 fail-closed），而不是放行工具。
#[tokio::test]
async fn failed_publication_drops_fence_without_admitting_tools() {
    let (tx, rx) = mpsc::channel(4);
    let (ack, done) = oneshot::channel();
    tx.send(PublishItem::BeforeTools { index: 1, ack })
        .await
        .unwrap();
    tx.send(PublishItem::Reply {
        index: 0,
        response: Some(ReplyResponse::new_reply()),
    })
    .await
    .unwrap();
    drop(tx);
    assert!(!publish_ordered(rx, |_| anyhow::bail!("注入的 emit 失败")).await);
    assert!(done.await.is_err(), "ack 必须被丢弃");
}
