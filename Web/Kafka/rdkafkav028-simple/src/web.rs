use axum::{Json,
           Router,
           extract::{DefaultBodyLimit,
                     Query,
                     State},
           http::StatusCode,
           response::{IntoResponse,
                      Response,
                      Sse,
                      sse::{Event,
                            KeepAlive}},
           routing::{get,
                     post}};
use futures_core::Stream;
use rdkafka::{ClientConfig,
              Message,
              Offset,
              TopicPartitionList,
              consumer::{Consumer,
                         StreamConsumer},
              error::KafkaError,
              producer::{FutureProducer,
                         FutureRecord,
                         Producer}};
use serde::{Deserialize,
            Serialize};
use serde_json::{Value,
                 json};
use std::{convert::Infallible,
          time::{Duration,
                 Instant}};
use tokio_util::sync::CancellationToken;
use tower_http::services::ServeDir;

const KAFKA_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_PAYLOAD: usize = 256 * 1024;

#[derive(Clone)]
pub struct AppState {
    brokers: String,
    producer: FutureProducer,
    shutdown: CancellationToken,
}

impl AppState {
    pub fn new(brokers: String) -> Result<Self, KafkaError> {
        let producer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("message.timeout.ms", "8000")
            .set("enable.idempotence", "true")
            .create()?;
        Ok(Self {
            brokers,
            producer,
            shutdown: CancellationToken::new(),
        })
    }

    pub fn shutdown(&self) { self.shutdown.cancel(); }
}

pub fn router(state: AppState, frontend: &str) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/messages", post(produce))
        .route("/api/events", get(consume))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .fallback_service(ServeDir::new(frontend))
        .with_state(state)
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response { (self.0, Json(json!({"error": self.1}))).into_response() }
}

impl From<KafkaError> for ApiError {
    fn from(error: KafkaError) -> Self { Self(StatusCode::BAD_GATEWAY, error.to_string()) }
}

fn validate_topic(topic: &str) -> Result<(), ApiError> {
    if topic.is_empty() || topic.len() > 249 || matches!(topic, "." | "..") || !topic.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "토픽은 영문, 숫자, . _ - 조합의 1~249자여야 합니다 (. 및 .. 제외).".into(),
        ));
    }
    Ok(())
}

async fn health(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let producer = state.producer.clone();
    let count = tokio::task::spawn_blocking(move || producer.client().fetch_metadata(None, KAFKA_TIMEOUT).map(|m| m.brokers().len()))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    if count == 0 {
        return Err(ApiError(StatusCode::BAD_GATEWAY, "Kafka 브로커가 없습니다.".into()));
    }
    Ok(Json(json!({"status": "ok", "brokers": state.brokers, "broker_count": count})))
}

#[derive(Deserialize)]
struct ProduceRequest {
    topic: String,
    key: Option<String>,
    payload: String,
}

async fn produce(State(state): State<AppState>, Json(input): Json<ProduceRequest>) -> Result<Json<Value>, ApiError> {
    validate_topic(&input.topic)?;
    if input.payload.len() > MAX_PAYLOAD || input.key.as_ref().is_some_and(|key| key.len() > 4096) {
        return Err(ApiError(StatusCode::BAD_REQUEST, "메시지는 UTF-8 256 KiB, 키는 4 KiB 이하여야 합니다.".into()));
    }
    let mut record = FutureRecord::<str, str>::to(&input.topic).payload(&input.payload);
    if let Some(key) = &input.key {
        record = record.key(key);
    }
    let delivery = state.producer.send(record, Duration::from_secs(1)).await.map_err(|(e, _)| ApiError::from(e))?;
    Ok(Json(
        json!({"topic": input.topic, "partition": delivery.partition, "offset": delivery.offset.to_string()}),
    ))
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StartPosition {
    Earliest,
    Latest,
}

#[derive(Deserialize)]
struct ConsumeQuery {
    topic: String,
    from: StartPosition,
}

// Read absolute offsets before announcing readiness. Using Offset::End here
// could skip a record produced after the UI says it is ready but before the
// first poll.
fn prepare_consumer(brokers: &str, query: &ConsumeQuery) -> Result<StreamConsumer, String> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", "solid-kafka-viewer")
        .set("enable.auto.commit", "false")
        .set("enable.auto.offset.store", "false")
        .set("allow.auto.create.topics", "false")
        .set("auto.offset.reset", "error")
        .create()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + KAFKA_TIMEOUT;
    let metadata = consumer.fetch_metadata(Some(&query.topic), KAFKA_TIMEOUT).map_err(|e| e.to_string())?;
    let topic = metadata
        .topics()
        .iter()
        .find(|t| t.name() == query.topic)
        .ok_or("토픽을 찾을 수 없습니다. 토픽을 먼저 생성하세요.")?;
    if let Some(error) = topic.error() {
        return Err(format!("토픽 조회 실패: {error:?}"));
    }
    if topic.partitions().is_empty() {
        return Err("토픽에 파티션이 없습니다.".into());
    }
    let mut assignment = TopicPartitionList::new();
    for partition in topic.partitions() {
        if let Some(error) = partition.error() {
            return Err(format!("파티션 조회 실패: {error:?}"));
        }
        let remaining = deadline.checked_duration_since(Instant::now()).ok_or("Kafka 연결 시간이 초과되었습니다.")?;
        let (low, high) = consumer.fetch_watermarks(&query.topic, partition.id(), remaining).map_err(|e| e.to_string())?;
        let offset = match query.from {
            | StartPosition::Earliest => low,
            | StartPosition::Latest => high,
        };
        assignment
            .add_partition_offset(&query.topic, partition.id(), Offset::Offset(offset))
            .map_err(|e| e.to_string())?;
    }
    consumer.assign(&assignment).map_err(|e| e.to_string())?;
    Ok(consumer)
}

#[derive(Serialize)]
struct KafkaMessage {
    topic: String,
    partition: i32,
    // Kafka offsets are i64; strings preserve precision in JavaScript.
    offset: String,
    key: Option<String>,
    payload: Option<String>,
    timestamp: Option<i64>,
}

fn event(name: &str, data: impl Serialize) -> Result<Event, Infallible> {
    Ok(Event::default().event(name).data(serde_json::to_string(&data).expect("serializable SSE data")))
}

async fn consume(State(state): State<AppState>, Query(query): Query<ConsumeQuery>) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    validate_topic(&query.topic)?;
    let shutdown = state.shutdown.clone();
    let stream = async_stream::stream! {
        let setup = tokio::task::spawn_blocking(move || prepare_consumer(&state.brokers, &query)).await;
        let consumer = match setup {
            Ok(Ok(consumer)) => consumer,
            other => {
                let error = match other { Ok(Err(e)) => e, Err(e) => e.to_string(), _ => unreachable!() };
                yield event("kafka-error", json!({"error": error}));
                return;
            }
        };
        yield event("ready", json!({"status": "ready"}));
        loop {
            let result = tokio::select! {
                _ = shutdown.cancelled() => break,
                result = consumer.recv() => result,
            };
            match result {
                Ok(message) => {
                    let message = KafkaMessage {
                        topic: message.topic().to_owned(), partition: message.partition(), offset: message.offset().to_string(),
                        key: message.key().map(|b| String::from_utf8_lossy(b).into_owned()),
                        payload: message.payload().map(|b| String::from_utf8_lossy(b).into_owned()),
                        timestamp: message.timestamp().to_millis(),
                    };
                    yield event("message", message);
                }
                Err(error) => {
                    yield event("kafka-error", json!({"error": error.to_string()}));
                    break;
                }
            }
        }
        // Dropping the HTTP body drops the stream and its consumer as well.
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(5))))
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
