//! Browser smoke-test helper. This is a librdkafka mock, not Apache Kafka.
use rdkafka::mocking::MockCluster;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cluster = MockCluster::new(1)?;
    cluster.create_topic("rust", 3, 1)?;
    println!("librdkafka 모의 브로커입니다. 실제 Apache Kafka가 아닙니다.");
    println!("다른 터미널에서 다음 명령을 실행하세요:");
    println!("KAFKA_BROKERS={} cargo run --bin web", cluster.bootstrap_servers());
    println!("종료: Ctrl+C (모든 모의 메시지가 사라집니다)");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
