# Kafka Events

This module defines all event types and structs for Kafka events. Uses enums and structs to ensure type safety and maintainability.

## Structure

This module provides:
- **Enums**: Define event types (type-safe)
- **Structs**: Define data structure for each event
- **Helper functions**: Parse events from JSON

## Event Types

### Feed Events

#### `FeedEventType` Enum

Enum defining event types related to Feed. Serializes/deserializes as snake_case strings.

```rust
pub enum FeedEventType {
    Created,      // New feed created
    Liked,        // Feed was liked
    Commented,    // Feed has new comment
    Viewed,       // Feed was viewed
}
```

**Methods**:
- `as_str()`: Convert enum to string (e.g., "created", "liked", "commented", "viewed")

**Serialization**: 
- Uses `#[serde(rename_all = "snake_case")]` to serialize as lowercase snake_case
- JSON format: `"created"`, `"liked"`, `"commented"`, `"viewed"`
- Deserialization is handled automatically by serde for type safety

#### Event Structs

##### `FeedCreatedEvent`

Event when a new feed is created.

```rust
pub struct FeedCreatedEvent {
    pub event_type: FeedEventType,
    pub feed_id: u64,
    pub user_id: i64,
    pub content: String,
    pub timestamp: String,
}
```

**Constructor**:
```rust
FeedCreatedEvent::new(feed_id, user_id, content)
```

**JSON Format**:
```json
{
  "event_type": "created",
  "feed_id": 1,
  "user_id": 1,
  "content": "Feed content",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

##### `FeedLikedEvent`

Event when a feed is liked.

```rust
pub struct FeedLikedEvent {
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64,
    pub timestamp: String,
}
```

**Constructor**:
```rust
FeedLikedEvent::new(feed_id, user_id)
```

**JSON Format**:
```json
{
  "event_type": "liked",
  "feed_id": 1,
  "user_id": 2,
  "timestamp": "2024-01-01T00:00:00Z"
}
```

##### `FeedCommentedEvent`

Event when a feed has a new comment.

```rust
pub struct FeedCommentedEvent {
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64,
    pub comment_id: String,
    pub content: String,
    pub timestamp: String,
}
```

**Constructor**:
```rust
FeedCommentedEvent::new(feed_id, user_id, comment_id, content)
```

**JSON Format**:
```json
{
  "event_type": "commented",
  "feed_id": 1,
  "user_id": 2,
  "comment_id": "uuid-string",
  "content": "Comment text",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

##### `FeedViewedEvent`

Event when a feed is viewed.

```rust
pub struct FeedViewedEvent {
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64,  // 0 if anonymous
    pub timestamp: String,
}
```

**Constructor**:
```rust
FeedViewedEvent::new(feed_id, user_id)
```

**JSON Format**:
```json
{
  "event_type": "viewed",
  "feed_id": 1,
  "user_id": 2,
  "timestamp": "2024-01-01T00:00:00Z"
}
```

**Note**: `user_id` = 0 if user is anonymous (not logged in).

### User Events

#### `UserEventType` Enum

Enum defining event types related to User:

```rust
pub enum UserEventType {
    UserCreated,  // New user created
}
```

**Methods**:
- `as_str()`: Convert enum to string (e.g., "user_created")

**Serialization**: 
- Uses `#[serde(rename_all = "snake_case")]` for automatic serialization
- Deserialization is handled automatically by serde

#### Event Structs

##### `UserCreatedEvent`

Event when a new user is created (signup).

```rust
pub struct UserCreatedEvent {
    pub event_type: UserEventType,
    pub user_id: u64,
    pub email: String,
    pub username: String,
    pub timestamp: String,
}
```

**Constructor**:
```rust
UserCreatedEvent::new(user_id, email, username)
```

**JSON Format**:
```json
{
  "event_type": "user_created",
  "user_id": 1,
  "email": "user@example.com",
  "username": "username",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

## Helper Functions

### `parse_feed_event()`

Parse feed event from JSON string.

```rust
pub fn parse_feed_event(
    payload: &str
) -> Result<(FeedEventType, serde_json::Value), serde_json::Error>
```

**Returns**:
- `Ok((event_type, value))`: Event type (enum) and parsed JSON value
- `Err(e)`: Parse error (if JSON is invalid or event_type is missing/invalid)

**Implementation**:
- Uses serde deserialization directly for type safety
- Extracts `event_type` field and deserializes it to `FeedEventType` enum
- Returns the full JSON value for further processing

**Usage**:
```rust
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
            _ => {}
        }
    }
    Err(e) => {
        log::error!("Failed to parse event: {:?}", e);
    }
}
```

### `parse_user_event()`

Parse user event from JSON string.

```rust
pub fn parse_user_event(
    payload: &str
) -> Result<(UserEventType, serde_json::Value), serde_json::Error>
```

**Returns**:
- `Ok((event_type, value))`: Event type and parsed JSON value
- `Err(e)`: Parse error

## Usage Examples

### Sending Events (Producer)

```rust
use crate::kafka::{KafkaProducer, FeedCreatedEvent};

// Create event
let event = FeedCreatedEvent::new(
    feed_id,
    user_id,
    content.clone()
);

// Serialize to JSON
let event_json = serde_json::to_string(&event)?;

// Send to Kafka
kafka_producer
    .send_message("feed_events", &feed_id.to_string(), &event_json)
    .await?;
```

### Receiving Events (Consumer)

```rust
use crate::kafka::{parse_feed_event, FeedEventType};

// Parse event from payload
match parse_feed_event(payload_str) {
    Ok((event_type, event_data)) => {
        match event_type {
            FeedEventType::Liked => {
                let feed_id = event_data.get("feed_id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let user_id = event_data.get("user_id")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                
                // Process like event
                handle_feed_liked(feed_id, user_id).await;
            }
            FeedEventType::Commented => {
                // Process comment event
            }
            FeedEventType::Viewed => {
                // Process view event
            }
            FeedEventType::Created => {
                // Process created event
            }
        }
    }
    Err(e) => {
        log::error!("Failed to parse event: {:?}", e);
    }
}
```

## Benefits

### Type Safety

Using enum instead of string literals helps:
- **Compile-time checking**: Compiler will report error if wrong event type is used
- **IDE support**: Auto-complete and type hints
- **Refactoring**: Easy to rename or add new event types

### Maintainability

- **Centralized**: All event definitions in one place
- **Documentation**: Structs and enums self-document
- **Consistency**: Ensures consistent format

### Extensibility

Adding new event is very easy:

1. Add variant to enum:
```rust
pub enum FeedEventType {
    Created,
    Liked,
    Commented,
    Viewed,
    Deleted,  // New event
}
```

2. Create new struct:
```rust
pub struct FeedDeletedEvent {
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64,
    pub timestamp: String,
}
```

3. Update `as_str()` method to include new variant:
```rust
impl FeedEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedEventType::Created => "created",
            FeedEventType::Liked => "liked",
            FeedEventType::Commented => "commented",
            FeedEventType::Viewed => "viewed",
            FeedEventType::Deleted => "deleted",  // Add new variant
        }
    }
}
```

**Note**: No need to update `from_str()` - serde handles deserialization automatically!

## Best Practices

1. **Always use structs**: Do not create events with `json!` macro, always use structs
2. **Use constructors**: Use `::new()` methods to create events
3. **Type matching**: Use enum matching instead of string comparison
4. **Error handling**: Always handle parse errors when consuming events
5. **Versioning**: If event structure needs to change, consider versioning

## Serialization

All events implement `Serialize` and `Deserialize` from `serde`:
- Serialize: `serde_json::to_string(&event)`
- Deserialize: `serde_json::from_str::<EventType>(json_str)`

**Event Type Serialization**:
- Uses `#[serde(rename_all = "snake_case")]` on `FeedEventType` enum
- Serializes as lowercase snake_case: `"created"`, `"liked"`, `"commented"`, `"viewed"`
- Deserialization is automatic and type-safe - no manual string matching needed
- `parse_feed_event()` uses serde deserialization directly for better type safety

