# Services

This module contains business logic services, especially services that process events from Kafka.

## Structure

```
services/
├── mod.rs              # Module exports
└── notification.rs     # Notification service
```

## Notification Service (`notification.rs`)

This service listens to Kafka events and automatically creates notifications for users.

### `handle_feed_liked_event()`

Handles event when feed is liked.

```rust
pub async fn handle_feed_liked_event(
    event_data: &Value,
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    redis_client: &RedisClient,
) -> ()
```

**Process**:
1. Extract `user_id`, `feed_id` from event
2. Get feed owner info from database using SeaORM (`feed_owner_id`, `feed_owner_username`)
3. **Update `top:users_liked` in realtime** - Increment score for feed owner
4. **Update `top:feeds_liked` in realtime** - Increment score for feed
5. Check if user likes their own feed → skip (no notification)
6. Get username from MySQL using SeaORM
7. Create `Notification` with:
   - `user_id`: feed_owner_id (recipient)
   - `from_user_id`: user_id (who liked)
   - `notification_type`: `NotificationType::Like`
   - `content`: "{username} liked your feed" (e.g., "John liked your feed")
8. Insert into MongoDB collection `notifications`
9. Log success/error

**Event Data**:
```json
{
  "event_type": "liked",
  "feed_id": 1,
  "user_id": 2,
  "feed_owner_id": 1,
  "timestamp": "..."
}
```

### `handle_feed_commented_event()`

Handles event when feed has new comment.

```rust
pub async fn handle_feed_commented_event(
    event_data: &Value,
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    redis_client: &RedisClient,
) -> ()
```

**Process**:
1. Extract `user_id`, `feed_id`, `content` from event
2. **Update `top:comments` in realtime** - Increment score for feed (top feeds by comments)
3. Get feed owner info from database using SeaORM (`feed_owner_id`)
4. Check if user comments their own feed → skip (no notification)
5. Get username from MySQL using SeaORM
6. Create `Notification` with:
   - `user_id`: feed_owner_id (recipient)
   - `from_user_id`: user_id (who commented)
   - `notification_type`: `NotificationType::Comment`
   - `content`: Actual comment content
7. Insert into MongoDB
8. Log success/error

**Event Data**:
```json
{
  "event_type": "commented",
  "feed_id": 1,
  "user_id": 2,
  "comment_id": "uuid",
  "content": "Comment text",
  "timestamp": "..."
}
```

**Note**: 
- `feed_owner_id` and `username` are **not** included in event data
- They are looked up from database when processing event to reduce load on API handler
- This optimization allows API to respond faster (no additional queries)

### `handle_feed_viewed_event()`

Handles event when feed is viewed.

```rust
pub async fn handle_feed_viewed_event(
    event_data: &Value,
    redis_client: &RedisClient,
) -> ()
```

**Process**:
1. Extract `feed_id` from event
2. **Update `top:feeds_viewed` in realtime** - Increment score for viewed feed
3. Log success/error

**Event Data**:
```json
{
  "event_type": "viewed",
  "feed_id": 1,
  "user_id": 2,
  "timestamp": "2024-01-01T00:00:00Z"
}
```

**Note**: 
- `user_id` = 0 if user is anonymous (not logged in)
- Event is published from `view_feed()` API handler
- Consumer will process async to update Redis top feeds viewed

## Integration with Kafka Consumer

Service is called from Kafka consumer in `main.rs`:

```rust
use crate::kafka::{parse_feed_event, FeedEventType};

kafka_consumer.start_consuming(move |topic, key, payload| {
    if topic == "feed_events" {
        match std::str::from_utf8(&payload) {
            Ok(payload_str) => {
                match parse_feed_event(payload_str) {
                    Ok((event_type, event_data)) => {
                        let mysql_pool = mysql_pool_clone.clone();
                        let mongo_db = mongo_db_clone.clone();
                        
                        tokio::spawn(async move {
                            match event_type {
                                FeedEventType::Liked => {
                                    handle_feed_liked_event(&event_data, &mongo_db, &mysql_pool).await;
                                }
                                FeedEventType::Commented => {
                                    handle_feed_commented_event(&event_data, &mongo_db, &mysql_pool, &redis_client).await;
                                }
                                FeedEventType::Viewed => {
                                    handle_feed_viewed_event(&event_data, &redis_client).await;
                                }
                                FeedEventType::Created => {
                                    // Handle created event if needed
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to parse feed event: {:?}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to decode feed event: {:?}", e);
            }
        }
    }
});
```

**Note**: Uses `parse_feed_event()` and `FeedEventType` enum for type-safe event parsing. See more in [kafka/EVENTS.md](../kafka/EVENTS.md).

**Async Processing**: Each event is processed in separate tokio task to avoid blocking consumer.

## Error Handling

- **Logging**: All errors are logged
- **Non-blocking**: Errors don't crash consumer
- **Graceful Degradation**: If notification creation fails, event is still processed

## Best Practices

1. **Idempotency**:
   - Check for duplicate notifications if needed
   - Use unique constraint in MongoDB

2. **Performance**:
   - Process async to avoid blocking Kafka consumer
   - Batch inserts if there are many notifications

3. **Data Consistency**:
   - Get username from MySQL using SeaORM (source of truth)
   - Validate event data before processing

4. **Monitoring**:
   - Log number of notifications created
   - Monitor error rate
   - Alert if there are many failures

## Realtime Top Statistics Updates

This service is also responsible for updating realtime top statistics in Redis when events occur:

### Realtime Updates

1. **`update_top_users_liked_realtime()`**:
   - Called when like event occurs
   - Updates `top:users_liked` - Increments score for feed owner
   - Logic: Uses `ZINCRBY top:users_liked 1 user_id` - simple and fast (O(log N))
   - Only stores `user_id` in Redis, not JSON

2. **`update_top_feeds_liked_realtime()`**:
   - Called when like event occurs
   - Updates `top:feeds_liked` - Increments score for liked feed
   - Logic: Uses `ZINCRBY top:feeds_liked 1 feed_id` - simple and fast (O(log N))
   - Only stores `feed_id` in Redis, not JSON

3. **`update_top_feeds_commented_realtime()`**:
   - Called when comment event occurs
   - Updates `top:comments` - Increments score for commented feed (top feeds by comments)
   - Logic: Uses `ZINCRBY top:comments 1 feed_id` - simple and fast (O(log N))
   - Only stores `feed_id` in Redis, not JSON

4. **`update_top_feeds_viewed_realtime()`**:
   - Called from `handle_feed_viewed_event()` when receiving `viewed` event (event_type: "viewed")
   - Updates `top:feeds_viewed` - Increments score for viewed feed
   - Logic: Uses `ZINCRBY top:feeds_viewed 1 feed_id` - simple and fast (O(log N))
   - Only stores `feed_id` in Redis, not JSON

**Note**: 
- Realtime updates use `ZINCRBY` - very fast and simple (O(log N))
- Only stores ID in Redis, not JSON to avoid parse/serialize overhead
- Detailed information (username, content) is looked up from database when API is called
- Background job still runs every hour to ensure accuracy and re-sorting
- If entry doesn't exist, `ZINCRBY` will automatically create new one with score = 1

## Future Enhancements

1. **Notification Preferences**: Users can disable certain types of notifications
2. **Batch Processing**: Process multiple events at once
3. **Notification Templates**: Customize notification messages
4. **Push Notifications**: Send push notifications (FCM, APNS)
5. **Email Notifications**: Send email for important notifications
6. **Notification Aggregation**: Aggregate multiple notifications of same type
