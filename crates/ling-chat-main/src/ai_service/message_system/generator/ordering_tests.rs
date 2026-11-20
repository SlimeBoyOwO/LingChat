//! 提交与发布的顺序契约测试：只驱动 publish_ordered，不触真实 consumer 与插件。

use super::*;

fn reply(index: usize, is_final: bool) -> PublishItem {
    let mut response = ReplyResponse::new_reply();
    response.message = index.to_string();
    response.is_final = is_final;
    PublishItem::Reply {
        index,
        response: Some(response),
    }
}

#[tokio::test]
async fn out_of_order_arrival_commits_before_emit_in_index_order() {
    let (tx, rx) = mpsc::channel(4);
    tx.send(reply(1, true)).await.unwrap();
    tx.send(reply(0, false)).await.unwrap();
    drop(tx);

    let log = Arc::new(Mutex::new(Vec::new()));
    let captured = log.clone();
    assert!(
        publish_ordered(rx, move |index, response| {
            let log = captured.clone();
            async move {
                assert_eq!(response.message, index.to_string());
                log.lock().await.push(("commit", index));
                tokio::task::yield_now().await;
                log.lock().await.push(("emit", index));
                Ok(true)
            }
        })
        .await
    );
    assert_eq!(
        *log.lock().await,
        vec![("commit", 0), ("emit", 0), ("commit", 1), ("emit", 1)]
    );
}

#[tokio::test]
async fn fence_ack_waits_for_commit_and_emit() {
    let (tx, rx) = mpsc::channel(4);
    let (ack, mut fence) = oneshot::channel();
    tx.send(PublishItem::BeforeTools { index: 1, ack })
        .await
        .unwrap();
    tx.send(reply(0, false)).await.unwrap();
    drop(tx);

    let (entered, waiting) = oneshot::channel::<()>();
    let (release, released) = oneshot::channel::<()>();
    let mut entered = Some(entered);
    let mut released = Some(released);
    let publisher = tokio::spawn(publish_ordered(rx, move |_, _| {
        let entered = entered.take().unwrap();
        let released = released.take().unwrap();
        async move {
            let _ = entered.send(());
            let _ = released.await;
            Ok(true)
        }
    }));
    waiting.await.unwrap();
    assert!(matches!(
        fence.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    release.send(()).unwrap();
    assert!(fence.await.unwrap());
    // 没有终句，通道关闭后返回 false
    assert!(!publisher.await.unwrap());
}

#[tokio::test]
async fn failed_or_stale_commit_stops_later_commits_and_fails_fence_closed() {
    for stale in [false, true] {
        let (tx, rx) = mpsc::channel(4);
        tx.send(reply(2, true)).await.unwrap();
        let (ack, fence) = oneshot::channel();
        tx.send(PublishItem::BeforeTools { index: 1, ack })
            .await
            .unwrap();
        tx.send(reply(0, false)).await.unwrap();
        drop(tx);

        assert!(
            !publish_ordered(rx, move |index, _| async move {
                assert_eq!(index, 0);
                if stale {
                    Ok(false)
                } else {
                    Err(anyhow::anyhow!("注入的提交失败"))
                }
            })
            .await
        );
        assert!(fence.await.is_err(), "栅栏 ack 必须被丢弃");
    }
}

#[tokio::test]
async fn skipped_reply_does_not_block_final() {
    let (tx, rx) = mpsc::channel(2);
    tx.send(PublishItem::Reply {
        index: 0,
        response: None,
    })
    .await
    .unwrap();
    tx.send(reply(1, true)).await.unwrap();
    drop(tx);

    assert!(
        publish_ordered(rx, |index, _| async move {
            assert_eq!(index, 1);
            Ok(true)
        })
        .await
    );
}
