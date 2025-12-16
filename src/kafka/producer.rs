use crate::config::Config;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseProducer, BaseRecord};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct KafkaProducer {
    producer: Arc<Mutex<BaseProducer>>,
}

impl KafkaProducer {
    pub fn new(config: &Config) -> Result<Self, anyhow::Error> {
        let producer: BaseProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.kafka.brokers)
            .set("message.timeout.ms", "5000")
            .create()?;

        Ok(KafkaProducer {
            producer: Arc::new(Mutex::new(producer)),
        })
    }

    pub async fn send_message(
        &self,
        topic: &str,
        key: &str,
        payload: &str,
    ) -> Result<(), anyhow::Error> {
        let producer = self.producer.lock().await;

        log::debug!(
            "Sending Kafka message: topic={}, key={}, payload_size={} bytes",
            topic,
            key,
            payload.len()
        );

        match producer.send(BaseRecord::to(topic).key(key).payload(payload)) {
            Ok(_) => {
                // Poll to ensure message is sent and handle delivery reports
                producer.poll(std::time::Duration::from_millis(0));

                log::info!(
                    "Kafka message queued successfully: topic={}, key={}, size={} bytes",
                    topic,
                    key,
                    payload.len()
                );
                Ok(())
            }
            Err((e, _)) => {
                log::error!(
                    "Failed to queue Kafka message: topic={}, key={}, error={:?}",
                    topic,
                    key,
                    e
                );
                Err(anyhow::anyhow!("Kafka send error: {:?}", e))
            }
        }
    }
}
