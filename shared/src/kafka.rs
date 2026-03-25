use anyhow::Result;
use rdkafka::{
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    producer::FutureProducer,
};

pub fn create_producer(brokers: &str) -> Result<FutureProducer> {
    let producer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "5000")
        .create()?;
    Ok(producer)
}

pub fn create_consumer(
    brokers: &str,
    group_id: &str,
    topics: &[&str],
    auto_offset_reset: &str,
    enable_auto_commit: bool,
) -> Result<StreamConsumer> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set(
            "enable.auto.commit",
            if enable_auto_commit { "true" } else { "false" },
        )
        .set("auto.offset.reset", auto_offset_reset)
        .create()?;

    consumer.subscribe(topics)?;
    Ok(consumer)
}
