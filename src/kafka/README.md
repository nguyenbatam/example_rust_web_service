# Kafka Integration

This module handles all Kafka operations: producer to send events and consumer to receive events.

## Structure

```
kafka/
├── mod.rs          # Export modules
├── producer.rs     # Kafka producer
├── consumer.rs     # Kafka consumer
└── events.rs       # Event types and structs (type-safe)
```

## Producer (`producer.rs`)

### `KafkaProducer`

Struct managing Kafka producer.

```rust
pub struct KafkaProducer {
    producer: Arc<Mutex<BaseProducer>>,
}
```

**Thread Safety**: Uses `Arc<Mutex<>>` to share between multiple threads (Actix workers). The producer is wrapped in `Arc<Mutex<>>` to allow safe concurrent access.

### `KafkaProducer::new()`

Create new producer from config.

```rust
pub fn new(config: &Config) -> Result<Self>
```

**Configuration**:
- `bootstrap.servers`: Kafka brokers (from config)
- `message.timeout.ms`: 5000ms

### `send_message()`

Send message to Kafka topic.

```rust
pub async fn send_message(
    &self,
    topic: &str,
    key: &str,
    payload: &str
) -> Result<()>
```

**Parameters**:
- `topic`: Topic name (e.g., "feed_events")
- `key`: Message key (usually feed_id or user_id)
- `payload`: JSON string of event data

**Error Handling**: 
- Returns error if send failed
- Logs error with details (topic, key, error message)
- Calls `poll()` after sending to ensure message is queued and handle delivery reports
- Non-blocking: Errors are logged but do not crash the application

### Usage

```rust
let producer = KafkaProducer::new(&config)?;
producer.send_message(
    "feed_events",
    &feed_id.to_string(),
    &json_data.to_string()
).await?;
```

## Consumer (`consumer.rs`)

### `KafkaConsumer`

Struct managing Kafka consumer.

```rust
pub struct KafkaConsumer {
    consumer: Arc<Mutex<StreamConsumer>>,
    topics: Vec<String>,
}
```

### `KafkaConsumer::new()`

Create new consumer.

```rust
pub fn new(config: &Config, topics: Vec<String>) -> Result<Self>
```

**Configuration**:
- `group.id`: Consumer group ID (from config)
- `bootstrap.servers`: Kafka brokers
- `enable.partition.eof`: false
- `session.timeout.ms`: 6000
- `enable.auto.commit`: true (auto commit offsets)
- `auto.offset.reset`: "earliest" (read from beginning if no offset)

### `subscribe()`

Subscribe to topics.

```rust
pub async fn subscribe(&self) -> Result<()>
```

**Topics**: Set when creating consumer.

### `start_consuming()`

Start consuming messages with handler function.

```rust
pub async fn start_consuming<F>(&self, handler: F) -> Result<()>
where
    F: Fn(String, String, Vec<u8>) + Send + Sync + 'static
```

**Handler Function**:
- `topic`: Topic name
- `key`: Message key
- `payload`: Message payload (bytes)

**Process**:
1. Spawn background task (tokio task)
2. Loop forever:
   - Receive message from Kafka using `recv().await`
   - Extract topic, key, and payload
   - Call handler function with topic, key, and payload bytes
   - Handle errors gracefully

**Error Handling**:
- Logs errors but does not crash consumer
- Sleeps 1 second if there is error to avoid busy loop
- Handles empty messages and payload deserialization errors
- Consumer continues running even if individual messages fail

## Topics

### `user_events`

Events related to user.

**Event Types**:
- `user_created`: When user signs up

**Consumers**: Job handlers

### `feed_events`

Events related to feed.

**Event Types**:
- `created`: New feed created (serialized as "created")
- `liked`: Feed was liked (serialized as "liked")
- `commented`: Feed has new comment (serialized as "commented")
- `viewed`: Feed was viewed (serialized as "viewed")

**Consumers**: Notification service

## Events (`events.rs`)

This module defines all event types and structs for Kafka events. Uses enums and structs to ensure type safety.

**See details**: [EVENTS.md](./EVENTS.md)

### Event Types

- **Feed Events**: `FeedCreatedEvent`, `FeedLikedEvent`, `FeedCommentedEvent`, `FeedViewedEvent`
- **User Events**: `UserCreatedEvent`

### Usage

```rust
use crate::kafka::{FeedCreatedEvent, FeedLikedEvent};

// Create event
let event = FeedCreatedEvent::new(feed_id, user_id, content);

// Serialize
let json = serde_json::to_string(&event)?;

// Send to Kafka
producer.send_message("feed_events", &key, &json).await?;
```

### Event Format

All events are JSON strings with standard format. See details in [EVENTS.md](./EVENTS.md) for complete structure of each event type.

## Usage in Application

### Producer

Injected into API handlers via `web::Data<KafkaProducer>`:

```rust
use crate::kafka::{KafkaProducer, FeedCreatedEvent};

pub async fn create_feed(
    // ...
    kafka_producer: web::Data<KafkaProducer>,
) -> HttpResponse {
    // ...
    // Create event with type-safe struct
    let event = FeedCreatedEvent::new(feed_id, user_id, content);
    let payload = serde_json::to_string(&event)?;
    
    kafka_producer.send_message("feed_events", &key, &payload).await?;
    // ...
}
```

### Consumer

Started in `main.rs`:

```rust
use crate::kafka::{KafkaConsumer, parse_feed_event, FeedEventType};

let consumer = KafkaConsumer::new(&config, vec!["feed_events".to_string()])?;
consumer.subscribe().await?;
consumer.start_consuming(|topic, key, payload| {
    if let Ok(payload_str) = std::str::from_utf8(&payload) {
        match parse_feed_event(payload_str) {
            Ok((event_type, event_data)) => {
                match event_type {
                    FeedEventType::Liked => {
                        // Handle like event
                    }
                    FeedEventType::Commented => {
                        // Handle comment event
                    }
                    FeedEventType::Viewed => {
                        // Handle view event
                    }
                    FeedEventType::Created => {
                        // Handle created event
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to parse event: {:?}", e);
            }
        }
    }
}).await?;
```

## Error Handling

### Producer Errors

- **Non-blocking**: Errors do not block HTTP request
- **Logging**: Log warnings if send failed
- **Retry**: Can implement retry logic if needed

### Consumer Errors

- **Graceful**: Errors do not crash consumer
- **Logging**: Log all errors
- **Recovery**: Sleep and retry if there is error

## Best Practices

1. **Idempotency**:
   - Events should be idempotent (can process multiple times)
   - Use message key to deduplicate if needed

2. **Ordering**:
   - Messages with same key will be processed in order
   - Use feed_id or user_id as key

3. **Error Handling**:
   - Producer: Non-blocking, log warnings
   - Consumer: Graceful degradation, retry logic

4. **Monitoring**:
   - Monitor consumer lag
   - Monitor producer throughput
   - Alert on errors

5. **Partitioning**:
   - Kafka automatically partitions based on key
   - Same key → same partition → ordered processing

## Future Enhancements

1. **Dead Letter Queue**: Store failed messages
2. **Retry Logic**: Retry with exponential backoff
3. **Schema Registry**: Validate event schemas
4. **Compression**: Enable compression for messages
5. **Batching**: Batch multiple messages if needed

