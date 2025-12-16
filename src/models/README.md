# Models

This module defines all data models and DTOs (Data Transfer Objects) for the API.

## Structure

```
models/
├── mod.rs          # Export models
├── user.rs         # User models & auth DTOs
└── feed.rs         # Feed, Comment, Notification models
```

## User Models (`user.rs`)

### `User`

Model representing a user in the database.

```rust
pub struct User {
    pub id: Option<i64>,              // None when inserting new
    pub email: String,
    pub username: String,
    pub password_hash: String,        // Not serialized in response
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

**Database mapping**: Used for API responses. Database operations use SeaORM entities from `src/entities/` module.

**Serialization**: `password_hash` is skipped when serializing (not sent to client).

### `SignupRequest`

DTO for signup request.

```rust
pub struct SignupRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}
```

**Validation**: Should validate email format, password strength at API layer.

### `LoginRequest`

DTO for login request.

```rust
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
```

### `AuthResponse`

Response after successful signup/login.

```rust
pub struct AuthResponse {
    pub token: String,        // JWT token
    pub user: UserResponse,    // User info (without password)
}
```

### `UserResponse`

User info returned to client (without sensitive data).

```rust
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub username: String,
}
```

**Conversion**: Has `From<User>` implementation for easy conversion.

## Feed Models (`feed.rs`)

### `Feed`

Model representing a feed post.

```rust
pub struct Feed {
    pub id: Option<i64>,
    pub user_id: i64,
    pub content: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}
```

**Database**: Used for API responses. Database operations use SeaORM entities from `src/entities/` module.

### `CreateFeedRequest`

DTO to create a new feed.

```rust
pub struct CreateFeedRequest {
    pub content: String,
}
```

### `FeedResponse`

Response when returning feed (with additional metadata).

```rust
pub struct FeedResponse {
    pub id: i64,
    pub user_id: i64,
    pub content: String,
    pub like_count: i64,        // Number of likes
    pub comment_count: i64,     // Number of comments
    pub is_liked: bool,          // Whether current user has liked
    pub created_at: DateTime<Utc>,
}
```

**Note**: `like_count`, `comment_count`, `is_liked` are calculated when querying, not stored in database.

### `Comment`

Model for comment (stored in MongoDB).

```rust
pub struct Comment {
    pub id: Option<String>,           // MongoDB _id
    pub feed_id: i64,
    pub user_id: i64,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
```

**Serialization**: 
- `id` is mapped to `_id` in MongoDB
- `created_at` is serialized as timestamp (seconds)

**Database**: Collection `comments` in MongoDB.

### `CommentRequest`

DTO to create a comment.

```rust
pub struct CommentRequest {
    pub content: String,
}
```

### `CommentResponse`

Response when returning comment (without username for performance optimization).

```rust
pub struct CommentResponse {
    pub id: String,
    pub feed_id: i64,
    pub user_id: i64,
    pub content: String,
    pub created_at: DateTime<Utc>,
}
```

**Note**: Does not include `username` field to optimize API response time. Username can be looked up from database by `user_id` if needed by consumer.

### `NotificationType`

Enum defining notification types.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Like,
    Comment,
}
```

**Serialization**: Enum is serialized as lowercase string ("like", "comment") when stored in MongoDB or returned via API.

**Benefits**:
- Type-safe: Cannot create notification with invalid type
- Easy to extend: Just add new variant to enum
- Compile-time checking: Rust compiler will report error if wrong type is used

### `Notification`

Model for notification (stored in MongoDB).

```rust
pub struct Notification {
    pub id: Option<String>,           // MongoDB _id
    pub user_id: i64,                  // User receiving notification
    pub from_user_id: i64,             // User performing action
    pub from_username: String,
    pub feed_id: i64,
    pub notification_type: NotificationType,  // Enum: Like or Comment
    pub content: String,               // Message displayed to user (always has value)
    pub created_at: DateTime<Utc>,
    pub is_read: bool,
}
```

**Database**: Collection `notifications` in MongoDB.

**Notification Types**:
- `NotificationType::Like`: When someone likes your feed
  - `content`: "{username} liked your feed" (e.g., "John liked your feed")
- `NotificationType::Comment`: When someone comments on your feed
  - `content`: Comment content

**Content Field**:
- Always has value (not `Option<String>`)
- For like notification: English message like "{username} liked your feed"
- For comment notification: Actual comment content
- Frontend can display directly without null check

### `NotificationResponse`

Response when returning notification.

```rust
pub struct NotificationResponse {
    pub id: String,
    pub from_user_id: i64,
    pub from_username: String,
    pub feed_id: i64,
    pub notification_type: NotificationType,  // Enum: Like or Comment
    pub content: String,               // Message displayed (always has value)
    pub created_at: DateTime<Utc>,
    pub is_read: bool,
}
```

### `FeedView`

Model to track feed views (stored in MongoDB).

```rust
pub struct FeedView {
    pub id: Option<String>,
    pub feed_id: i64,
    pub user_id: i64,                  // 0 if anonymous
    pub viewed_at: DateTime<Utc>,
}
```

**Database**: Collection `feed_views` in MongoDB.

**Usage**: Inserted each time there is a view (can be duplicate).

### Top Statistics Models

#### `TopUser`

Top user with most likes.

```rust
pub struct TopUser {
    pub user_id: i64,
    pub username: String,
    pub total_likes: i64,
}
```

#### `TopFeed`

Top feed (used for both viewed and liked).

```rust
pub struct TopFeed {
    pub feed_id: i64,
    pub user_id: i64,
    pub username: String,
    pub content: String,
    pub count: i64,             // Number of views or likes
}
```

## Serialization

All models implement `Serialize` and `Deserialize` from `serde`:

- **Request DTOs**: Only `Deserialize` (receive from client)
- **Response DTOs**: Only `Serialize` (send to client)
- **Database Models**: Both (for query and insert)

## OpenAPI/Swagger

All models have `#[derive(ToSchema)]` to automatically generate OpenAPI schema for Swagger UI.

## Best Practices

1. **Separation of Concerns**:
   - Database models (`User`, `Feed`) are separate from DTOs
   - Request DTOs are separate from Response DTOs

2. **Security**:
   - Never serialize sensitive data (password_hash)
   - User ID is extracted from JWT token, do not trust from client

3. **Type Safety**:
   - Use `Option<T>` for nullable fields
   - Use specific types instead of `String` when possible

4. **Validation**:
   - Should validate at API layer before converting to models
   - Email format, password strength, content length, etc.

