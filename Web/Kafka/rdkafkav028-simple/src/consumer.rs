use rdkafka::{ClientConfig,
              Message,
              consumer::{BaseConsumer,
                         Consumer}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let consumer: BaseConsumer = ClientConfig::new()
        .set("bootstrap.servers", "localhost:9092")
        .set("group.id", "my_group_id")
        .create()
        .expect("invalid consumer config");

    consumer.subscribe(&["rust"]).expect("topic subscribe failed");

    loop {
        for msg_result in consumer.iter() {
            let msg = msg_result.unwrap();
            let key: &str = msg.key_view().unwrap().unwrap();
            let value = msg.payload().unwrap();
            println!(
                "received key {} with value {:?} in offset {:?} from \
                      partition {}",
                key,
                String::from_utf8_lossy(value),
                msg.offset(),
                msg.partition()
            )
        }
    }
}
