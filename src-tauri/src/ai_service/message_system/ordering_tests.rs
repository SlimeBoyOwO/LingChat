//! #784: drive the actual producer and ordered publisher without desktop I/O.
use super::generator::{PublishItem, publish_ordered};
use super::producer::{PresentationChunk, SentenceItem, StreamProducer};
use super::responses::ReplyResponse;
use crate::ai_service::llm::LlmChunk;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

async fn run_round(preamble: &str, suffix: &str) -> (Vec<String>, bool, bool) {
    let preamble = preamble.to_owned();
    let suffix = suffix.to_owned();
    let trace = Arc::new(Mutex::new(Vec::<String>::new()));
    let tool_trace = trace.clone();
    let seen = Arc::new(AtomicBool::new(false));
    let tool_seen = seen.clone();
    let stream = async_stream::try_stream! {
        if !preamble.is_empty() { yield PresentationChunk::Chunk(LlmChunk::Content(preamble)); }
        let (tx, rx) = oneshot::channel();
        yield PresentationChunk::BeforeTools(tx);
        rx.await.expect("publisher must acknowledge");
        tool_trace.lock().unwrap().push("TOOL".into());
        tool_seen.store(true, Ordering::Release);
        if !suffix.is_empty() { yield PresentationChunk::Chunk(LlmChunk::Content(suffix)); }
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
    let output = tokio::time::timeout(std::time::Duration::from_secs(3), producer.run())
        .await
        .unwrap()
        .unwrap();
    consumer.await.unwrap();
    let published = publisher.await.unwrap();
    let result = trace.lock().unwrap().clone();
    (result, output.sent_final, published)
}

#[tokio::test]
async fn preamble_is_published_before_tool_and_suffix_is_final() {
    for preamble in [
        "我先查询一下。",
        "【正常】我先查询一下。",
        "【正常】先说明。【开心】然后查询。",
    ] {
        let (trace, sent, published) = run_round(preamble, "【正常】查询完成。").await;
        let tool = trace.iter().position(|s| s == "TOOL").unwrap();
        assert!(tool > 0, "{trace:?}");
        assert!(trace[..tool].iter().all(|s| s.ends_with(":false")));
        assert!(trace.last().unwrap().ends_with(":true"));
        assert!(sent && published);
    }
}

#[tokio::test]
async fn tool_only_does_not_wait_for_nonexistent_preamble() {
    let (trace, sent, published) = run_round("", "完成").await;
    assert_eq!(trace[0], "TOOL");
    assert!(sent && published);
}

#[tokio::test]
async fn empty_or_repeated_post_tool_content_does_not_replay_preamble_as_final() {
    let preamble = "【正常】我先查询一下资料。";
    for suffix in ["", preamble] {
        let (trace, sent, published) = run_round(preamble, suffix).await;
        assert_eq!(trace.len(), 2, "{trace:?}");
        assert!(
            !sent && !published,
            "caller must take explicit error/reset path"
        );
    }
}

#[tokio::test]
async fn publisher_fence_waits_for_earlier_indexes_including_skipped_results() {
    let (tx, rx) = mpsc::channel(4);
    let (ack, mut done) = oneshot::channel();
    tx.send(PublishItem::BeforeTools { index: 2, ack })
        .await
        .unwrap();
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
    assert!(done.await.unwrap());
    drop(tx);
    assert!(!task.await.unwrap());
}

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
    assert!(!publish_ordered(rx, |_| anyhow::bail!("injected emit failure")).await);
    assert!(done.await.is_err());
}
