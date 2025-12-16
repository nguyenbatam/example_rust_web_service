# Database Layer

This module manages all database connections: MySQL, MongoDB, and Redis.

## Structure

```
db/
├── mod.rs          # Module exports
├── mysql.rs        # MySQL connection & schema
├── mongodb.rs      # MongoDB connection
└── redis.rs        # Redis connection
```

## MySQL (`mysql.rs`)

### Functionality

Manages MySQL database connection using SeaORM and automatically creates necessary tables on startup.

### Functions

#### `create_mysql_pool(config: &Config) -> Result<DbPool>`

Creates MySQL database connection using SeaORM and automatically creates tables if they don't exist (using raw SQL statements):

1. **users**: Stores user information
   - `id`: Primary key, auto increment
   - `email`: Unique, no duplicates
   - `username`: Unique, no duplicates
   - `password_hash`: Hashed password
   - `created_at`, `updated_at`: Timestamps

2. **feeds**: Stores feed posts
   - `id`: Primary key
   - `user_id`: Foreign key to users
   - `content`: Feed content
   - Indexes: `user_id`, `created_at` for fast queries

3. **feed_likes**: Stores feed likes
   - `id`: Primary key
   - `feed_id`: Foreign key to feeds
   - `user_id`: Foreign key to users
   - Unique constraint: `(feed_id, user_id)` - each user can only like once
   - Indexes: `feed_id`, `user_id`

### Usage

```rust
use db::create_mysql_pool;

let db = create_mysql_pool(&config).await?;
// Tables are automatically created if they don't exist
// Use SeaORM entities to query
use crate::entities::user;
let user = user::Entity::find_by_id(1).one(&db).await?;
```

**Note**: In production, consider using SeaORM migrations (`sea-orm-migration`) instead of raw SQL for schema management.

### Connection String

Format: `mysql://user:password@host:port/database`

Created from config via `config.mysql_url()`.

## MongoDB (`mongodb.rs`)

### Functionality

Manages MongoDB client connection and database.

### Functions

#### `create_mongodb_client(config: &Config) -> Result<Database>`

Creates MongoDB client and returns database instance.

### Collections

1. **comments**: Stores feed comments
   - `_id`: MongoDB ObjectId or custom string
   - `feed_id`: Feed ID
   - `user_id`: User ID who commented
   - `content`: Comment content
   - `created_at`: Timestamp

2. **notifications**: Stores notifications
   - `_id`: MongoDB ObjectId
   - `user_id`: User receiving notification
   - `from_user_id`: User performing action
   - `from_username`: Username for display
   - `feed_id`: Related feed ID
   - `notification_type`: Enum `NotificationType` ("like" or "comment" when serialized)
   - `content`: Message displayed to user (always has value)
     - For like: "{username} liked your feed" (e.g., "John liked your feed")
     - For comment: Actual comment content
   - `created_at`: Timestamp
   - `is_read`: Whether read

3. **feed_views**: Tracks feed views
   - `_id`: MongoDB ObjectId
   - `feed_id`: Feed ID
   - `user_id`: User ID who viewed (0 if anonymous)
   - `viewed_at`: Timestamp

### Usage

```rust
use db::create_mongodb_client;

let db = create_mongodb_client(&config).await?;
let collection = db.collection::<Comment>("comments");
```

### Connection String

Format: `mongodb://host:port` or `mongodb://user:password@host:port`

Retrieved from `config.mongodb.uri`.

## Redis (`redis.rs`)

### Functionality

Manages Redis client connection.

### Functions

#### `create_redis_client(config: &Config) -> Result<RedisClient>`

Creates Redis client.

### Keys Used

**Top Statistics (Sorted Sets - ZSET)**:

1. **top:users_liked**: Sorted Set of top users with most likes
   - **Member**: `user_id` (string) - only stores ID, not JSON
   - **Score**: `total_likes` (number of likes)
   - Username is looked up from database when API is called

2. **top:comments**: Sorted Set of top feeds with most comments
   - **Member**: `feed_id` (string) - only stores ID, not JSON
   - **Score**: `count` (number of comments)
   - Feed info is looked up from database when API is called

3. **top:feeds_viewed**: Sorted Set of top feeds with most views
   - **Member**: `feed_id` (string) - only stores ID, not JSON
   - **Score**: `count` (number of views)
   - Feed info is looked up from database when API is called

4. **top:feeds_liked**: Sorted Set of top feeds with most likes
   - **Member**: `feed_id` (string) - only stores ID, not JSON
   - **Score**: `count` (number of likes)
   - Feed info is looked up from database when API is called

**Note**: 
- Only stores ID instead of JSON to optimize performance when updating (uses `ZINCRBY` instead of parse/serialize JSON)
- Detailed information is looked up from database when needed (when API is called)

### Usage

```rust
use db::create_redis_client;

let client = create_redis_client(&config)?;
let mut conn = client.get_async_connection().await?;
let value: Option<String> = redis::cmd("GET")
    .arg("top:users_liked")
    .query_async(&mut conn)
    .await?;
```

### Connection String

Format: `redis://host:port` or `redis://:password@host:port`

Created from config via `config.redis_url()`.

## Best Practices

1. **ORM Usage**: 
   - MySQL uses SeaORM for type-safe queries
   - Entities are defined in `src/entities/` module
   - Use SeaORM query builder for complex queries
   - Raw SQL can be used for complex aggregations

2. **Error Handling**:
   - All functions return `Result` to handle errors
   - Uses `anyhow::Error` for error type

3. **Initialization**:
   - All connections are created in `main.rs` on startup
   - Shared via `web::Data` in Actix

4. **Indexes**:
   - MySQL tables have indexes for frequently queried columns
   - MongoDB collections can create indexes if needed

5. **Transactions**:
   - MySQL supports transactions via SeaORM
   - MongoDB supports transactions (requires replica set)

## Migration

Currently schema is created automatically on startup. For production, should:

1. Use SeaORM migrations (`sea-orm-migration`) for MySQL
2. Create indexes for MongoDB collections
3. Backup strategy for all 3 databases
