# API Routes

This module contains all REST API endpoints organized by domain.

## Structure

```
api/
├── mod.rs          # Export modules
├── auth.rs         # Authentication endpoints
├── feed.rs         # Feed endpoints
├── notify.rs       # Notification endpoints
└── top.rs          # Top statistics endpoints
```

## Authentication API (`auth.rs`)

### `POST /api/auth/signup`

Register a new user.

**Handler**: `signup()`

**Request Body**:
```json
{
  "email": "user@example.com",
  "username": "username",
  "password": "password123"
}
```

**Process**:
1. Check if user already exists (email or username) using SeaORM
2. Hash password
3. Insert into database using SeaORM
4. Create JWT token
5. Publish `user_created` event to Kafka
6. Return token and user info

**Response**:
- `200 OK`: Success with token
- `409 Conflict`: User already exists
- `500 Internal Server Error`: Database error

### `POST /api/auth/login`

Login user.

**Handler**: `login()`

**Request Body**:
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**Process**:
1. Find user by email using SeaORM
2. Verify password
3. Create JWT token
4. Return token and user info

**Response**:
- `200 OK`: Success with token
- `401 Unauthorized`: Invalid credentials
- `404 Not Found`: User does not exist

## Feed API (`feed.rs`)

All endpoints require JWT authentication (except `GET /api/feed` which is optional).

### `POST /api/feed`

Create a new feed.

**Handler**: `create_feed()`

**Auth**: Required (`AuthenticatedUser`)

**Request Body**:
```json
{
  "content": "Feed content here"
}
```

**Process**:
1. Extract user_id from JWT token
2. Insert feed into database using SeaORM
3. Publish `created` event to Kafka (event_type: "created")
4. Return feed with metadata

**Response**: `FeedResponse` with `like_count=0`, `comment_count=0`, `is_liked=false`

### `GET /api/feed`

Get list of feeds.

**Handler**: `get_feeds()`

**Auth**: Optional (`Option<AuthenticatedUser>`)

**Query Parameters**:
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 20): Number of feeds per page

**Process**:
1. Calculate offset from `page` and `limit` parameters
2. Query feeds from database using SeaORM (ORDER BY created_at DESC, with LIMIT and OFFSET)
3. For each feed:
   - Count likes from `feed_likes` table using SeaORM (find all and count length)
   - Count comments from MongoDB collection `comments` using `count_documents`
   - Check if user has liked using SeaORM (if authenticated) - single query per feed
4. Return list of `FeedResponse`

**Response**: Array of `FeedResponse`

### `POST /api/feed/{feed_id}/like`

Like a feed.

**Handler**: `like_feed()`

**Auth**: Required

**Process**:
1. Check if already liked using SeaORM (SELECT query to prevent duplicate)
2. Verify feed exists (SELECT query)
3. Insert into `feed_likes` table using SeaORM (INSERT query)
4. Publish `liked` event to Kafka (event_type: "liked")
5. Notification service will create notification (async, does not block API response)
6. Feed owner is retrieved by Kafka consumer when processing event (async)

**Response**:
- `200 OK`: Success with message "Feed liked"
- `200 OK`: "Already liked" if already liked
- `400 Bad Request`: If unique constraint violation (race condition)
- `404 Not Found`: If feed does not exist

### `DELETE /api/feed/{feed_id}/like`

Unlike a feed.

**Handler**: `unlike_feed()`

**Auth**: Required

**Process**:
1. Delete from `feed_likes` table using SeaORM
2. Return success

**Note**: Does not publish event when unliking.

### `POST /api/feed/{feed_id}/comment`

Comment on a feed.

**Handler**: `comment_feed()`

**Auth**: Required

**Request Body**:
```json
{
  "content": "Comment text"
}
```

**Process**:
1. Insert comment into MongoDB
2. Publish `commented` event to Kafka (event_type: "commented", minimal data)
3. Notification service will lookup username and feed_owner_id when processing event

**Response**: `CommentResponse` (without username - consumer can lookup later if needed)

**Optimization**: 
- Does not query username from database (reduces 1 query)
- Does not query feed_owner from database (reduces 1 query)
- Consumer will enrich data when processing Kafka event

### `GET /api/feed/{feed_id}/comments`

Get list of comments for a feed.

**Handler**: `get_comments()`

**Auth**: Not required

**Process**:
1. Query comments from MongoDB (filter by feed_id)
2. Return list of `CommentResponse` (without username)

**Response**: Array of `CommentResponse` (without username - consumer can lookup later if needed)

**Optimization**: 
- Does not query username from MySQL for each comment (reduces N queries)
- Consumer can batch lookup usernames if needed

### `POST /api/feed/{feed_id}/view`

Track feed view.

**Handler**: `view_feed()`

**Auth**: Optional

**Process**:
1. Insert `FeedView` into MongoDB
2. Publish `FeedViewedEvent` to Kafka topic `feed_events`
3. Return success

**Event-Driven**:
- Event is published for consumer to process async
- Consumer will update `top:feeds_viewed` in Redis
- Reduces load on API handler (does not block request)

**Note**: 
- Each view is tracked separately (can be duplicate)
- Used to calculate top feeds viewed
- User_id = 0 if anonymous
- Event format: `{"event_type": "viewed", "feed_id": 1, "user_id": 2, "timestamp": "..."}`

## Notification API (`notify.rs`)

All endpoints require JWT authentication.

### `GET /api/notify`

Get list of notifications for current user.

**Handler**: `get_notifications()`

**Query Parameters**:
- `limit` (optional, default: 50): Number of notifications

**Process**:
1. Extract user_id from JWT
2. Query notifications from MongoDB (filter by user_id)
3. Sort by created_at DESC
4. Limit results
5. Return list of `NotificationResponse`

**Response**: Array of `NotificationResponse`

### `PUT /api/notify/{notification_id}/read`

Mark notification as read.

**Handler**: `mark_notification_read()`

**Process**:
1. Extract user_id from JWT
2. Update notification in MongoDB (set is_read = true)
3. Filter by notification_id and user_id (security)

**Response**: `200 OK` with message

## Top Statistics API (`top.rs`)

All endpoints do not require authentication (public). Supports pagination with `page` and `limit` parameters.

**Data Storage**: Uses Redis Sorted Sets (ZSET) to store data, allowing efficient pagination and range queries.

### `GET /api/top/users-liked`

Get top users with most likes.

**Handler**: `get_top_users_liked()`

**Auth**: Not required

**Query Parameters**:
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Number of items per page

**Process**:
1. Calculate range based on `page` and `limit`
2. Use `ZREVRANGE WITHSCORES` to get `user_id` and scores from Redis Sorted Set (descending order)
3. Batch lookup usernames from database using SeaORM based on `user_id`
4. Create `TopUser` objects with information from database
5. Return `Vec<TopUser>`

**Response**: Array of `TopUser`
```json
[
  {
    "user_id": 1,
    "username": "user1",
    "total_likes": 100
  }
]
```

**Example**: `GET /api/top/users-liked?page=1&limit=10`

### `GET /api/top/feeds-commented`

Get top feeds with most comments (rank feeds by number of comments).

**Handler**: `get_top_comments()`

**Auth**: Not required

**Query Parameters**:
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Number of items per page

**Process**:
1. Calculate range based on `page` and `limit`
2. Use `ZREVRANGE WITHSCORES` to get `feed_id` and scores from Redis Sorted Set
3. Batch lookup feed info (user_id, username, content) from database using SeaORM based on `feed_id`
4. Create `TopFeed` objects with information from database
5. Return `Vec<TopFeed>`

**Response**: Array of `TopFeed` (feeds with most comments)
```json
[
  {
    "feed_id": 1,
    "user_id": 1,
    "username": "feed_owner_username",
    "content": "Feed content",
    "count": 25
  }
]
```

**Note**: `count` = number of comments for that feed

**Example**: `GET /api/top/feeds-commented?page=2&limit=20`

### `GET /api/top/feeds-viewed`

Get top feeds with most views.

**Handler**: `get_top_feeds_viewed()`

**Auth**: Not required

**Query Parameters**:
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Number of items per page

**Process**:
1. Calculate range based on `page` and `limit`
2. Use `ZREVRANGE WITHSCORES` to get `feed_id` and scores from Redis Sorted Set
3. Batch lookup feed info (user_id, username, content) from database using SeaORM based on `feed_id`
4. Create `TopFeed` objects with information from database
5. Return `Vec<TopFeed>`

**Response**: Array of `TopFeed`
```json
[
  {
    "feed_id": 1,
    "user_id": 1,
    "username": "user1",
    "content": "Feed content",
    "count": 500
  }
]
```

**Example**: `GET /api/top/feeds-viewed?page=1&limit=5`

### `GET /api/top/feeds-liked`

Get top feeds with most likes.

**Handler**: `get_top_feeds_liked()`

**Auth**: Not required

**Query Parameters**:
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Number of items per page

**Process**:
1. Calculate range based on `page` and `limit`
2. Use `ZREVRANGE WITHSCORES` to get `feed_id` and scores from Redis Sorted Set
3. Batch lookup feed info (user_id, username, content) from database using SeaORM based on `feed_id`
4. Create `TopFeed` objects with information from database
5. Return `Vec<TopFeed>`

**Response**: Array of `TopFeed` (similar to `feeds-viewed`)

**Example**: `GET /api/top/feeds-liked?page=1&limit=10`

**Note**: 
- **Realtime Updates**: Data is updated in realtime when events occur (like, comment, view)
  - `top:users_liked` - Updated when like event occurs (uses `ZINCRBY`)
  - `top:feeds_liked` - Updated when like event occurs (uses `ZINCRBY`)
  - `top:comments` - Updated when comment event occurs (uses `ZINCRBY`)
  - `top:feeds_viewed` - Updated when view event occurs (uses `ZINCRBY`)
- **Storage**: Only stores ID (`user_id` or `feed_id`) in Redis, does not store JSON
  - Optimizes performance when updating (no need to parse/serialize JSON)
  - Detailed information (username, content) is looked up from database when API is called
- **Background Job**: Still runs every hour to ensure accuracy and re-sorting
- Frontend can call each endpoint separately instead of calling one endpoint that returns all
- Uses Redis Sorted Sets (ZSET) for storage, allowing efficient pagination
- Each item is stored with score as number for sorting (total_likes, count)
- Background job calculates and stores maximum 1000 items for each top stats type
- Realtime updates use `ZINCRBY` - very fast (O(log N)) and simple

## Error Handling

All handlers use `ActixResult<HttpResponse>`:

- **Success**: `Ok(HttpResponse::Ok().json(data))`
- **Error**: `Ok(HttpResponse::Error().json({"error": "message"}))`

**Error Types**:
- `400 Bad Request`: Invalid input
- `401 Unauthorized`: Authentication required/failed
- `404 Not Found`: Resource not found
- `409 Conflict`: Resource conflict (duplicate)
- `500 Internal Server Error`: Server error

## OpenAPI/Swagger

All endpoints have `#[utoipa::path(...)]` attributes to generate OpenAPI docs:

- Path, method
- Request body schema
- Response schemas
- Tags for grouping

Swagger UI available at `/api/docs`.

## Best Practices

1. **Authentication**:
   - Use `AuthenticatedUser` extractor for protected endpoints
   - Optional auth with `Option<AuthenticatedUser>`

2. **Error Handling**:
   - Always log errors
   - Do not expose internal errors to client
   - Return appropriate HTTP status codes

3. **Validation**:
   - Validate input data (email format, content length, etc.)
   - Return clear error messages

4. **Performance**:
   - Use indexes for database queries
   - Consider caching for frequently accessed data
   - Pagination for list endpoints

5. **Security**:
   - Never trust client input
   - Always validate user_id from token
   - SeaORM automatically uses parameterized queries (SQL injection prevention)

