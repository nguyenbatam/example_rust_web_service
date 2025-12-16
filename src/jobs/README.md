# Background Jobs

This module contains background jobs that run periodically to calculate and process data.

## Structure

```
jobs/
├── mod.rs          # Module exports
├── handlers.rs     # Kafka event handlers
└── top_stats.rs    # Top statistics calculation job
```

## Event Handlers (`handlers.rs`)

### `handle_user_created_event()`

Handles event when user is created.

```rust
pub fn handle_user_created_event(topic: String, key: String, payload: Vec<u8>)
```

**Process**:
1. Parse payload from bytes to string
2. Parse JSON
3. Log event data
4. Can add logic: send welcome email, create user profile, etc.

**Usage**: Called from Kafka consumer when receiving `user_created` event.

## Top Statistics Job (`top_stats.rs`)

Job calculates top statistics and stores in Redis.

### `calculate_top_stats()`

Main function that calculates all top statistics.

```rust
pub async fn calculate_top_stats(
    mysql_pool: &DbPool,
    mongo_db: &MongoDatabase,
    redis_client: &RedisClient,
) -> ()
```

**Process**:
1. Calculate 7 days ago timestamp
2. Call 4 functions to calculate (each function gets max 1000 items):
   - `calculate_top_users_liked()` - Top users whose feeds received most likes
   - `calculate_top_comments()` - Top feeds with most comments
   - `calculate_top_feeds_viewed()` - Top feeds with most views
   - `calculate_top_feeds_liked()` - Top feeds with most likes
3. Delete old data in Redis for each key (DEL command) before storing new data
4. Store in Redis Sorted Sets (ZSET) using `ZADD` command with:
   - **Score**: Number for sorting (total_likes, count)
   - **Value**: ID string (`user_id` or `feed_id`) - only stores ID, not JSON
   - **Keys**:
     - `top:users_liked` - stores `user_id`
     - `top:comments` (top feeds by comments) - stores `feed_id`
     - `top:feeds_viewed` - stores `feed_id`
     - `top:feeds_liked` - stores `feed_id`
   - Detailed information (username, content) is looked up from database when API is called
5. Log completion

**Schedule**: Runs every hour (3600 seconds)

**Redis Storage**:
- Uses `ZADD` to add items to Sorted Set
- Each item has score for sorting (descending order when querying)
- Only stores ID (`user_id` or `feed_id`) instead of JSON to optimize performance
- Uses `ZREVRANGE WITHSCORES` in API to get IDs and scores, then lookup information from database

**Realtime vs Background Job**:
- **Realtime Updates**: Consumer updates directly to Redis Sorted Sets when events occur (uses `ZINCRBY` to increment score)
  - Only stores ID in Redis, not JSON
  - Very fast and simple (O(log N))
- **Background Job**: Recalculates from scratch every hour to ensure accuracy and re-sorting
  - Only stores ID in Redis, not JSON
- Background job is still needed to:
  - Recalculate from scratch (ensure accuracy)
  - Re-sort in correct order
  - Handle edge cases (like unlike, delete feed, etc.)

### `calculate_top_users_liked()`

Calculates top users whose feeds received most likes in last 7 days.

```rust
async fn calculate_top_users_liked(
    pool: &DbPool,
    since: DateTime<Utc>,
) -> Vec<TopUser>
```

**Implementation**: Uses SeaORM raw SQL for complex aggregation query.

**SQL Query**:
```sql
SELECT 
    f.user_id,
    u.username,
    COUNT(fl.id) as total_likes
FROM feeds f
INNER JOIN feed_likes fl ON f.id = fl.feed_id
INNER JOIN users u ON f.user_id = u.id
WHERE fl.created_at >= ?
GROUP BY f.user_id, u.username
ORDER BY total_likes DESC
LIMIT 1000
```

**Result**: Top 1000 users (to support pagination)

### `calculate_top_comments()`

Calculates top comments in last 7 days.

```rust
async fn calculate_top_comments(
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    since: DateTime<Utc>,
) -> Vec<TopFeed>
```

**Process**:
1. Query comments from MongoDB (filter by created_at >= since)
2. Count comments per feed_id
3. Get top feeds by comment count
4. Get feed info and username from MySQL using SeaORM
5. Return `TopFeed` list (with count as number of comments)

**Note**: This function returns top feeds with most comments, not top comments. Each item is a feed with comment count.

### `calculate_top_feeds_viewed()`

Calculates top feeds with most views in last 7 days.

```rust
async fn calculate_top_feeds_viewed(
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    since: DateTime<Utc>,
) -> Vec<TopFeed>
```

**Process**:
1. Query `feed_views` from MongoDB (filter by viewed_at >= since)
2. Count views per feed_id
3. Sort by count DESC
4. Get top 1000 feeds
5. Get feed info (user_id, content) from MySQL using SeaORM
6. Get username from MySQL using SeaORM
7. Return `TopFeed` list

### `calculate_top_feeds_liked()`

Calculates top feeds with most likes in last 7 days.

```rust
async fn calculate_top_feeds_liked(
    pool: &DbPool,
    since: DateTime<Utc>,
) -> Vec<TopFeed>
```

**Implementation**: Uses SeaORM raw SQL for complex aggregation query.

**SQL Query**:
```sql
SELECT 
    f.id,
    f.user_id,
    u.username,
    f.content,
    COUNT(fl.id) as like_count
FROM feeds f
INNER JOIN feed_likes fl ON f.id = fl.feed_id
INNER JOIN users u ON f.user_id = u.id
WHERE fl.created_at >= ?
GROUP BY f.id, f.user_id, u.username, f.content
ORDER BY like_count DESC
LIMIT 1000
```

**Result**: Top 1000 feeds (to support pagination)

## Job Scheduling

Job is scheduled in `main.rs`:

```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));
    loop {
        interval.tick().await;
        calculate_top_stats(&mysql_pool, &mongodb_db, &redis_client).await;
    }
});
```

**Schedule**:
- Runs every hour (3600 seconds)
- Runs first time after 5 seconds (to wait for server startup)

## Data Flow

```
┌─────────────┐
│   MySQL     │──┐
│   MongoDB   │──┼──► calculate_top_stats()
│             │  │
└─────────────┘  │
                 ▼
         ┌──────────────┐
         │  Calculate  │
         │  Top Stats  │
         │ (max 1000)  │
         └──────┬───────┘
                ▼
         ┌──────────────┐
         │   Store ID   │
         │  + Score     │
         └──────┬───────┘
                ▼
         ┌──────────────┐
         │    Redis    │
         │ Sorted Sets  │
         │   (ZSET)     │
         └──────┬───────┘
                ▼
         ┌──────────────┐
         │  API Client  │
         │ GET /api/top │
         │ + pagination │
         └──────────────┘
```

## Performance Considerations

1. **Time Range**: Only calculates last 7 days to reduce data processing
2. **Limits**: Gets top 1000 items to support pagination (instead of just 10)
3. **Caching**: Results are cached in Redis Sorted Sets, not recalculated on each request
4. **Pagination**: Uses `ZREVRANGE` to only query needed range, doesn't parse all data
5. **Async**: All operations are async to avoid blocking
6. **Storage**: Redis Sorted Sets allow efficient range queries (O(log(N) + M) complexity)

## Error Handling

- **Logging**: All errors are logged
- **Graceful**: Errors don't crash job, continues with available data
- **Fallback**: If calculation fails, Redis still keeps old data

## Best Practices

1. **Idempotency**: Job can run multiple times without side effects
2. **Monitoring**: Log execution time and errors
3. **Alerting**: Alert if job fails multiple times
4. **Optimization**: 
   - Use indexes in database
   - Consider materialized views if needed
   - Cache intermediate results if needed

## Future Enhancements

1. **More Statistics**:
   - Top users by followers
   - Trending feeds
   - Most active users

2. **Time Ranges**:
   - Top today, this week, this month
   - Historical trends

3. **Real-time Updates**:
   - Update Redis when new event occurs (instead of waiting for job)
   - Use Redis sorted sets

4. **Analytics**:
   - User engagement metrics
   - Content performance
   - Growth metrics
