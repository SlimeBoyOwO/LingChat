#[cfg(test)]
mod tests {
    use crate::memory_test_api::api::{ApiState, router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use std::sync::Arc;
    use tokio::sync::{Notify, oneshot};
    use tower::ServiceExt;

    fn state() -> ApiState {
        let (sender, _receiver) = oneshot::channel();
        ApiState {
            token: Arc::from("test-token"),
            shutdown: Arc::new(std::sync::Mutex::new(Some(sender))),
            busy: Arc::new(std::sync::Mutex::new(false)),
            closing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            idle: Arc::new(Notify::new()),
        }
    }

    #[tokio::test]
    async fn health_requires_bearer_token() {
        let response = router(state())
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn body_limit_rejects_oversized_payload() {
        let payload = "x".repeat(256 * 1024 + 1);
        let request = Request::post("/v1/memory/validate")
            .header(header::AUTHORIZATION, "Bearer test-token")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload))
            .unwrap();
        let response = router(state()).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn validation_rejects_unknown_scenario_and_accepts_scripted_success() {
        let app = router(state());
        let unauthorized = app
            .clone()
            .oneshot(
                Request::post("/v1/memory/validate")
                    .body(Body::from(r#"{"scenario":"basic-compression"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let request = Request::post("/v1/memory/validate")
            .header(header::AUTHORIZATION, "Bearer test-token")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"scenario":"unknown"}"#))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let request = Request::post("/v1/memory/validate")
            .header(header::AUTHORIZATION, "Bearer test-token")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"scenario":"basic-compression"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn built_in_failure_scenario_owns_fault_injection() {
        let response = router(state())
            .oneshot(
                Request::post("/v1/scenarios/one-section-fails")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"fail_section":"not-a-real-section"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["outcome"], "not_committed");
        assert_eq!(body["committed"], false);
        assert_eq!(body["calls"], 4);
        assert_eq!(body["last_processed_global_idx"], 0);
    }

    #[tokio::test]
    async fn autosave_scenario_executes_real_save_and_retry() {
        let response = router(state())
            .oneshot(
                Request::post("/v1/scenarios/memory-finishes-after-line-save")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["outcome"], "succeeded");
        assert_eq!(body["persistence_roundtrip"], true);
        assert_eq!(body["details"]["persisted_last_processed_global_idx"], 2);
    }

    #[tokio::test]
    async fn concurrent_validation_returns_too_many_requests() {
        let app = router(state());
        let request = || {
            Request::post("/v1/memory/validate")
                .header(header::AUTHORIZATION, "Bearer test-token")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"scenario":"basic-compression","delay_ms":50}"#,
                ))
                .unwrap()
        };
        let first = app.clone().oneshot(request());
        tokio::task::yield_now().await;
        let second = app.clone().oneshot(request());
        let (first, second) = tokio::join!(first, second);
        let statuses = [first.unwrap().status(), second.unwrap().status()];
        assert!(statuses.contains(&StatusCode::TOO_MANY_REQUESTS));
        assert!(statuses.contains(&StatusCode::OK));
    }

    #[tokio::test]
    async fn shutdown_waits_for_in_flight_validation_before_signaling_exit() {
        let app = router(state());
        let validation = tokio::spawn({
            let app = app.clone();
            async move {
                app.oneshot(
                    Request::post("/v1/scenarios/memory-finishes-after-line-save")
                        .header(header::AUTHORIZATION, "Bearer test-token")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"delay_ms":100}"#))
                        .unwrap(),
                )
                .await
                .unwrap()
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let shutdown = app
            .oneshot(
                Request::post("/shutdown")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(shutdown.status(), StatusCode::OK);
        let validation = validation.await.unwrap();
        assert_eq!(validation.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn panic_validation_releases_single_flight_slot() {
        let app = router(state());
        let response = app
            .clone()
            .oneshot(
                Request::post("/v1/scenarios/panic-compression")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1024 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["outcome"], "not_committed");
        assert_eq!(body["committed"], false);
        assert_eq!(body["triggered"], true);
        assert_eq!(body["calls"], 4);
        assert_eq!(body["last_processed_global_idx"], 0);
        let response = app
            .oneshot(
                Request::post("/v1/memory/validate")
                    .header(header::AUTHORIZATION, "Bearer test-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"scenario":"basic-compression"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
