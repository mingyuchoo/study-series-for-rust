use super::*;
use axum::{body::Body,
           http::Request};
use http_body_util::BodyExt;
use rdkafka::mocking::MockCluster;
use tower::ServiceExt;

async fn post_message(app: &Router, topic: &str, key: Option<&str>, payload: &str) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/messages")
                .header("content-type", "application/json")
                .body(Body::from(json!({"topic": topic, "key": key, "payload": payload}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn read_event(body: &mut Body, name: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let frame = body.frame().await.expect("SSE ended").unwrap();
            let Ok(bytes) = frame.into_data() else {
                continue;
            };
            let text = String::from_utf8(bytes.to_vec()).unwrap();
            assert!(!text.contains("event: kafka-error"), "{text}");
            if text.contains(&format!("event: {name}\n")) {
                let data = text.lines().find_map(|line| line.strip_prefix("data: ")).unwrap();
                return serde_json::from_str(data).unwrap();
            }
        }
    })
    .await
    .expect("SSE timeout")
}

#[tokio::test]
async fn kafka_http_sse_roundtrip_and_offset_selection() {
    let cluster = MockCluster::new(1).unwrap();
    cluster.create_topic("rust", 1, 1).unwrap();
    let state = AppState::new(cluster.bootstrap_servers()).unwrap();
    let app = router(state.clone(), "frontend/dist");
    let health = app.clone().oneshot(Request::get("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(post_message(&app, "rust", Some("old"), "이전 메시지").await.status(), StatusCode::OK);

    let response = app
        .clone()
        .oneshot(Request::get("/api/events?topic=rust&from=latest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut live = response.into_body();
    read_event(&mut live, "ready").await;

    // A second browser receives the same records rather than sharing a group
    // assignment.
    let response = app
        .clone()
        .oneshot(Request::get("/api/events?topic=rust&from=latest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let mut second = response.into_body();
    read_event(&mut second, "ready").await;
    let response = post_message(&app, "rust", None, "안녕하세요\n<script>alert(1)</script>").await;
    assert_eq!(response.status(), StatusCode::OK);
    let delivery: Value = serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let message = read_event(&mut live, "message").await;
    assert_eq!(message["key"], Value::Null);
    assert_eq!(message["payload"], "안녕하세요\n<script>alert(1)</script>");
    assert_eq!(message["offset"], delivery["offset"]);
    assert_eq!(message["partition"], delivery["partition"]);
    assert_eq!(read_event(&mut second, "message").await["offset"], delivery["offset"]);

    let response = app
        .clone()
        .oneshot(Request::get("/api/events?topic=rust&from=earliest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let mut history = response.into_body();
    read_event(&mut history, "ready").await;
    assert_eq!(read_event(&mut history, "message").await["payload"], "이전 메시지");
    assert_eq!(read_event(&mut history, "message").await["offset"], delivery["offset"]);
    assert_eq!(post_message(&app, "rust", Some(""), "").await.status(), StatusCode::OK);
    let empty = read_event(&mut history, "message").await;
    assert_eq!(empty["key"], "");
    assert_eq!(empty["payload"], "");
    state.shutdown();
    tokio::time::timeout(Duration::from_secs(2), history.collect())
        .await
        .expect("shutdown must close SSE")
        .unwrap();
    // Bodies/clients must be dropped before their mock broker goes away.
    drop((live, second, app, state));
}

#[tokio::test]
async fn rejects_invalid_input_without_contacting_kafka() {
    let app = router(AppState::new("127.0.0.1:1".into()).unwrap(), "frontend/dist");
    for topic in ["", ".", "..", "has space", "한글", "bad/topic"] {
        let response = post_message(&app, topic, None, "message").await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{topic}");
    }
    assert_eq!(
        post_message(&app, "rust", None, &"x".repeat(MAX_PAYLOAD + 1)).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(post_message(&app, "rust", Some(&"x".repeat(4097)), "").await.status(), StatusCode::BAD_REQUEST);
    for uri in ["/api/events?topic=..&from=latest", "/api/events?topic=rust&from=invalid"] {
        let response = app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn missing_topic_reports_sse_error_instead_of_ready() {
    let cluster = MockCluster::new(1).unwrap();
    // Explicit error prevents mock broker auto-creation.
    cluster
        .topic_error("missing", rdkafka::types::RDKafkaRespErr::RD_KAFKA_RESP_ERR_UNKNOWN_TOPIC_OR_PART)
        .unwrap();
    let app = router(AppState::new(cluster.bootstrap_servers()).unwrap(), "frontend/dist");
    let response = app
        .oneshot(Request::get("/api/events?topic=missing&from=latest").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = response.into_body();
    let bytes = tokio::time::timeout(Duration::from_secs(12), body.collect()).await.unwrap().unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("event: kafka-error"), "{text}");
    assert!(!text.contains("event: ready"), "{text}");
}

#[tokio::test]
async fn unavailable_broker_returns_bounded_errors() {
    let app = router(AppState::new("127.0.0.1:1".into()).unwrap(), "frontend/dist");
    let health = app.clone().oneshot(Request::get("/api/health").body(Body::empty()).unwrap());
    let publish = post_message(&app, "rust", None, "unavailable");
    let stream = async {
        let response = app
            .clone()
            .oneshot(Request::get("/api/events?topic=rust&from=latest").body(Body::empty()).unwrap())
            .await
            .unwrap();
        response.into_body().collect().await.unwrap().to_bytes()
    };
    let (health, publish, stream) = tokio::time::timeout(Duration::from_secs(12), async { tokio::join!(health, publish, stream) })
        .await
        .expect("broker errors must not hang indefinitely");
    assert_eq!(health.unwrap().status(), StatusCode::BAD_GATEWAY);
    assert_eq!(publish.status(), StatusCode::BAD_GATEWAY);
    let text = String::from_utf8(stream.to_vec()).unwrap();
    assert!(text.contains("event: kafka-error"), "{text}");
    assert!(!text.contains("event: ready"), "{text}");
}
