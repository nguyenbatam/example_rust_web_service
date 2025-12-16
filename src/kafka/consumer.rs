use crate::config::Config;
use log::{error, info};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{stream_consumer::StreamConsumer, Consumer};
use rdkafka::Message;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct KafkaConsumer {
    consumer: Arc<Mutex<StreamConsumer>>,
    topics: Vec<String>,
}

impl KafkaConsumer {
    pub fn new(config: &Config, topics: Vec<String>) -> Result<Self, anyhow::Error> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", &config.kafka.group_id)
            .set("bootstrap.servers", &config.kafka.brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()?;

        Ok(KafkaConsumer {
            consumer: Arc::new(Mutex::new(consumer)),
            topics,
        })
    }

    pub async fn subscribe(&self) -> Result<(), anyhow::Error> {
        let consumer = self.consumer.lock().await;
        consumer.subscribe(&self.topics.iter().map(|s| s.as_str()).collect::<Vec<_>>())?;
        Ok(())
    }

    pub async fn start_consuming<F>(&self, handler: F) -> Result<(), anyhow::Error>
    where
        F: Fn(String, String, Vec<u8>) + Send + Sync + 'static,
    {
        let consumer = Arc::clone(&self.consumer);

        tokio::spawn(async move {
            loop {
                match consumer.lock().await.recv().await {
                    Ok(message) => match message.payload_view::<str>() {
                        None => {
                            error!("Received empty message");
                        }
                        Some(Ok(_payload)) => {
                            let topic = message.topic().to_string();
                            let key = message
                                .key()
                                .and_then(|k| std::str::from_utf8(k).ok())
                                .unwrap_or("")
                                .to_string();
                            let payload_bytes = message.payload().unwrap_or(&[]).to_vec();

                            info!("Received message from topic: {}, key: {}", topic, key);
                            handler(topic, key, payload_bytes);
                        }
                        Some(Err(e)) => {
                            error!("Error while deserializing message payload: {:?}", e);
                        }
                    },
                    Err(e) => {
                        error!("Error receiving message: {:?}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }
}
