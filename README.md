# Example Rust Web Service

A complete REST API service built with Rust featuring:

- REST API with Swagger documentation
- Kafka integration (producer and consumer)
- Job services for statistics calculation
- Backend services listening to Kafka events
- Databases: MySQL, MongoDB, Redis
- Authentication: Login, Sign up with JWT tokens
- Feed system: Create feed, like/unlike, comment
- Notification system: Notifications for likes/comments
- Top statistics: Top users, top feeds, top comments

## 🏗️ System Architecture

```
┌─────────────┐
│   Client    │
└──────┬──────┘
       │ HTTP/REST
       ▼
┌─────────────────────────────────┐
│      Actix-Web Server          │
│  ┌──────────────────────────┐  │
│  │   API Routes              │  │
│  │  - Auth (signup/login)   │  │
│  │  - Feed (CRUD, like)      │  │
│  │  - Comment               │  │
│  │  - Notification          │  │
│  │  - Top Stats             │  │
│  └──────────────────────────┘  │
└──────┬──────────────────┬───────┘
       │                  │
       ▼                  ▼
┌─────────────┐    ┌──────────────┐
│   MySQL     │    │   MongoDB    │
│  - Users    │    │  - Comments  │
│  - Feeds    │    │  - Notify    │
│  - Likes    │    │  - Views     │
└─────────────┘    └──────────────┘
       │                  │
       └──────────┬───────┘
                  ▼
         ┌─────────────┐
         │    Redis    │
         │  - Top Stats│
         └─────────────┘
                  │
                  ▼
         ┌─────────────┐
         │   Kafka     │
         │  - Events   │
         └──────┬──────┘
                │
                ▼
    ┌───────────────────────┐
    │  Kafka Consumers      │
    │  - Notification Svc   │
    │  - Job Handlers       │
    └───────────────────────┘
```

## 📊 Database Schema

### MySQL Database

#### Table: `users`
```sql
CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    username VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);
```

#### Table: `feeds`
```sql
CREATE TABLE feeds (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_user_id (user_id),
    INDEX idx_created_at (created_at)
);
```

#### Table: `feed_likes`
```sql
CREATE TABLE feed_likes (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    feed_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY unique_feed_user (feed_id, user_id),
    FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_feed_id (feed_id),
    INDEX idx_user_id (user_id)
);
```

### MongoDB Collections

#### Collection: `comments`
```json
{
  "_id": "uuid",
  "feed_id": 123,
  "user_id": 456,
  "content": "Comment text",
  "created_at": 1234567890
}
```

#### Collection: `notifications`
```json
{
  "_id": "uuid",
  "user_id": 456,
  "from_user_id": 789,
  "from_username": "username",
  "feed_id": 123,
  "notification_type": "like" | "comment",
  "content": "Display message",
  "created_at": 1234567890,
  "is_read": false
}
```

#### Collection: `feed_views`
```json
{
  "_id": "uuid",
  "feed_id": 123,
  "user_id": 456,
  "viewed_at": 1234567890
}
```

### Redis Keys

Using Redis Sorted Sets (ZSET) to store top statistics with pagination support:

**Top Statistics (Sorted Sets)**:
- `top:users_liked` - Sorted Set of top users with most likes (score = total_likes)
  - Value: `user_id` (string) - only stores ID, not JSON
  - Realtime update: When like event occurs, uses `ZINCRBY` to increment score for feed owner
  - Username is looked up from database when API is called
- `top:comments` - Sorted Set of top feeds with most comments (score = count)
  - Value: `feed_id` (string) - only stores ID, not JSON
  - Realtime update: When comment event occurs, uses `ZINCRBY` to increment score for feed
  - Feed info is looked up from database when API is called
- `top:feeds_viewed` - Sorted Set of top feeds with most views (score = count)
  - Value: `feed_id` (string) - only stores ID, not JSON
  - Realtime update: When view event occurs, uses `ZINCRBY` to increment score for feed
  - Feed info is looked up from database when API is called
- `top:feeds_liked` - Sorted Set of top feeds with most likes (score = count)
  - Value: `feed_id` (string) - only stores ID, not JSON
  - Realtime update: When like event occurs, uses `ZINCRBY` to increment score for feed
  - Feed info is looked up from database when API is called

Each item in Sorted Set:
- **Score**: Number for sorting (total_likes, count)
- **Value**: ID string (`user_id` or `feed_id`) - not storing JSON for optimal update performance
- **Lookup**: Detailed information (username, content, etc.) is looked up from database when API is called

Using `ZREVRANGE` to query by range with pagination.

## 🗂️ Project Structure

```
src/
├── main.rs                 # Entry point - initialize server, databases, Kafka
├── config.rs              # Configuration management from .env
│
├── db/                    # Database connections
│   ├── mod.rs            # Module exports
│   ├── mysql.rs          # MySQL connection & table creation
│   ├── mongodb.rs        # MongoDB connection
│   └── redis.rs          # Redis connection
│
├── entities/             # SeaORM database entities
│   ├── mod.rs
│   ├── user.rs           # User entity for MySQL
│   ├── feed.rs           # Feed entity for MySQL
│   └── feed_like.rs      # FeedLike entity for MySQL
│
├── models/               # Data models & DTOs
│   ├── mod.rs
│   ├── user.rs           # User model, SignupRequest, LoginRequest
│   └── feed.rs           # Feed, Comment, Notification, TopUser, TopFeed models
│
├── auth/                  # Authentication & Authorization
│   ├── mod.rs
│   ├── jwt.rs            # JWT token creation & verification
│   ├── password.rs       # Password hashing with bcrypt
│   └── extractor.rs      # AuthenticatedUser extractor for Actix
│
├── api/                   # REST API endpoints
│   ├── mod.rs
│   ├── auth.rs           # POST /api/auth/signup, /api/auth/login
│   ├── feed.rs           # Feed CRUD, like, comment, view
│   ├── notify.rs         # GET /api/notify, PUT /api/notify/{id}/read
│   └── top.rs            # GET /api/top - Top statistics
│
├── kafka/                 # Kafka integration
│   ├── mod.rs
│   ├── producer.rs       # Kafka producer to send events
│   └── consumer.rs       # Kafka consumer to receive events
│
├── services/              # Business logic services
│   ├── mod.rs
│   └── notification.rs   # Service handling notification creation from Kafka events
│
└── jobs/                  # Background jobs
    ├── mod.rs
    ├── handlers.rs        # Event handlers for Kafka messages
    └── top_stats.rs       # Job calculating top statistics and storing in Redis

tests/
└── api_test.rs           # Integration tests for all API endpoints
```

## 📡 API Endpoints

### Authentication

#### `POST /api/auth/signup`
Register a new user.

**Request:**
```json
{
  "email": "user@example.com",
  "username": "username",
  "password": "password123"
}
```

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "user": {
    "id": 1,
    "email": "user@example.com",
    "username": "username"
  }
}
```

#### `POST /api/auth/login`
Login and receive JWT token.

**Request:**
```json
{
  "email": "user@example.com",
  "password": "password123"
}
```

**Response:** Same as signup

### Feed Endpoints

**Note:** 
- `GET /api/feed` does not require authentication (optional)
- Other endpoints require JWT token in header: `Authorization: Bearer <token>`
- `comment_count` is retrieved directly from MongoDB collection `comments` by counting documents with matching `feed_id`

#### `POST /api/feed`
Create a new feed.

**Request:**
```json
{
  "content": "Feed content here"
}
```

**Response:**
```json
{
  "id": 1,
  "user_id": 1,
  "content": "Feed content here",
  "like_count": 0,
  "comment_count": 0,
  "is_liked": false,
  "created_at": "2024-01-01T00:00:00Z"
}
```

#### `GET /api/feed?limit=20&offset=0`
Get list of feeds (authentication optional).

**Query Parameters:**
- `limit` (optional): Number of feeds (default: 20)
- `offset` (optional): Offset (default: 0)

**Response:**
```json
[
  {
    "id": 1,
    "user_id": 1,
    "content": "Feed content here",
    "like_count": 5,
    "comment_count": 3,
    "is_liked": false,
    "created_at": "2024-01-01T00:00:00Z"
  }
]
```

**Note:**
- `like_count`: Retrieved from MySQL table `feed_likes`
- `comment_count`: Retrieved from MongoDB collection `comments` (count comments by `feed_id`)
- `is_liked`: Only has value if user is logged in (has JWT token)

#### `POST /api/feed/{feed_id}/like`
Like a feed.

**Response:**
```json
{
  "message": "Feed liked"
}
```

**Note:**
- Uses direct `INSERT` with UNIQUE constraint to avoid duplicates
- If already liked, returns `"Already liked"` (no duplicate created)
- Only performs 1 database query (INSERT), no SELECT needed to check first
- Feed owner is retrieved by Kafka consumer after receiving event (async processing)

#### `DELETE /api/feed/{feed_id}/like`
Unlike a feed.

#### `POST /api/feed/{feed_id}/comment`
Comment on a feed.

**Request:**
```json
{
  "content": "Comment text"
}
```

**Response:**
```json
{
  "id": "uuid",
  "feed_id": 1,
  "user_id": 2,
  "content": "Comment text",
  "created_at": "2024-01-01T00:00:00Z"
}
```

**Note:**
- Response does not contain `username` for performance optimization
- Does not query username and feed_owner from database
- Consumer can lookup username later if needed (from `user_id`)

#### `GET /api/feed/{feed_id}/comments`
Get list of comments for a feed.

**Response:**
```json
[
  {
    "id": "uuid",
    "feed_id": 1,
    "user_id": 2,
    "content": "Comment text",
    "created_at": "2024-01-01T00:00:00Z"
  }
]
```

**Note:**
- Response does not contain `username` for performance optimization
- Does not query username from MySQL for each comment
- Consumer can batch lookup usernames if needed

#### `POST /api/feed/{feed_id}/view`
Track feed view (saved to MongoDB).

### Notification Endpoints

Requires JWT token.

#### `GET /api/notify?limit=50`
Get list of notifications for current user.

**Query Parameters:**
- `limit` (optional): Number of notifications (default: 50)

**Response:**
```json
[
  {
    "id": "uuid",
    "from_user_id": 2,
    "from_username": "user2",
    "feed_id": 1,
    "notification_type": "like",
    "content": "user2 liked your feed",
    "created_at": 1234567890,
    "is_read": false
  }
]
```

#### `PUT /api/notify/{notification_id}/read`
Mark notification as read.

### Top Statistics

All endpoints do not require authentication (public). Frontend can call each endpoint separately. Supports pagination with `page` and `limit` parameters.

**Data Storage**: Uses Redis Sorted Sets (ZSET) to store data, allowing efficient pagination and range queries.

#### `GET /api/top/users-liked`
Get top users with most likes.

**Query Parameters:**
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Items per page

**Response:**
```json
[
  {
    "user_id": 1,
    "username": "user1",
    "total_likes": 100
  }
]
```

**Example:** `GET /api/top/users-liked?page=1&limit=10`

#### `GET /api/top/feeds-commented`
Get top feeds with most comments (ranked by comment count).

**Query Parameters:**
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Items per page

**Response:**
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

**Note:** `count` = number of comments for that feed

**Example:** `GET /api/top/feeds-commented?page=2&limit=20`

#### `GET /api/top/feeds-viewed`
Get top feeds with most views.

**Query Parameters:**
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Items per page

**Response:**
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

**Example:** `GET /api/top/feeds-viewed?page=1&limit=5`

#### `GET /api/top/feeds-liked`
Get top feeds with most likes.

**Query Parameters:**
- `page` (optional, default: 1): Page number
- `limit` (optional, default: 10): Items per page

**Response:** Same as `feeds-viewed` (Array of `TopFeed`)

**Example:** `GET /api/top/feeds-liked?page=1&limit=10`

**Note:**
- **Realtime Updates**: Data is updated in realtime when events occur (using `ZINCRBY`):
  - `top:users_liked` - Updated when like event occurs (increment score for feed owner)
  - `top:feeds_liked` - Updated when like event occurs (increment score for feed)
  - `top:comments` - Updated when comment event occurs (increment score for feed)
  - `top:feeds_viewed` - Updated when view event occurs (increment score for feed)
- **Storage**: Only stores ID (`user_id` or `feed_id`) in Redis, not JSON
  - Optimizes performance when updating (no need to parse/serialize JSON)
  - Detailed information (username, content) is looked up from database when API is called
- **Background Job**: Still runs every hour to ensure accuracy and re-sorting
- Uses Redis Sorted Sets (ZSET) for storage, allowing efficient pagination
- Background job calculates and stores up to 1000 items for each type of top stats
- Realtime updates use `ZINCRBY` - very fast (O(log N)) and simple

## 🔄 Kafka Events

### Topics

- `user_events` - Events related to users
- `feed_events` - Events related to feeds (created, liked, commented)

### Event Types

#### `user_created`
Published when user successfully registers.

```json
{
  "event_type": "user_created",
  "user_id": 1,
  "email": "user@example.com",
  "username": "username",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### `created`
Published when feed is created.

```json
{
  "event_type": "created",
  "feed_id": 1,
  "user_id": 1,
  "content": "Feed content",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

#### `liked`
Published when feed is liked.

```json
{
  "event_type": "liked",
  "feed_id": 1,
  "user_id": 2,
  "timestamp": "2024-01-01T00:00:00Z"
}
```

**Note:** `feed_owner_id` is not sent in event. Consumer will query from database when processing event to reduce load on API handler.

#### `commented`
Published when new comment is added.

```json
{
  "event_type": "commented",
  "feed_id": 1,
  "user_id": 2,
  "comment_id": "uuid",
  "content": "Comment text",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

**Note:** `username` and `feed_owner_id` are not sent in event. Consumer will query from database when processing event to:
- Reduce load on API handler (no need for additional queries)
- Optimize API response time
- Consumer can batch lookup or cache username/feed_owner if needed

#### `viewed`
Published when feed is viewed.

```json
{
  "event_type": "viewed",
  "feed_id": 1,
  "user_id": 2,
  "timestamp": "2024-01-01T00:00:00Z"
}
```

**Note:** 
- `user_id` = 0 if user is anonymous (not logged in)
- Event is published from `view_feed()` API handler
- Consumer will update `top:feeds_viewed` in Redis to calculate top feeds viewed

### Event Processing

1. **Notification Service**: Listens to `feed_events` and processes events:
   - When receiving `liked` event (event_type: "liked"): 
     - Query `feed_owner_id` from database based on `feed_id`
     - Query `username` from database based on `user_id`
     - Update `top:users_liked` and `top:feeds_liked` in Redis
     - Create notification and update top stats
   - When receiving `commented` event (event_type: "commented"): 
     - **Update `top:comments` in Redis first** (increment score for feed)
     - Query `feed_owner_id` from database based on `feed_id`
     - Query `username` from database based on `user_id`
     - Create notification
   - When receiving `viewed` event (event_type: "viewed"):
     - Update `top:feeds_viewed` in Redis (increment score for feed)
   - Processing is async, does not block API response
   - Can cache username and feed_owner to reduce database queries
   - Event types are deserialized using serde for type safety
2. **Job Handlers**: Process other events if needed

## ⚙️ Background Jobs

### Top Statistics Job

Job runs every hour to calculate:

1. **Top Users Liked**: Users whose feeds received most likes in last 7 days (max 1000 items)
2. **Top Feeds by Comments**: Feeds with most comments in last 7 days (max 1000 items)
3. **Top Feeds Viewed**: Feeds with most views in last 7 days (max 1000 items)
4. **Top Feeds Liked**: Feeds with most likes in last 7 days (max 1000 items)

**Process**:
1. Calculate from scratch based on data from last 7 days
2. Store in Redis Sorted Sets (ZSET) with:
   - **Score**: Number for sorting (total_likes, count)
   - **Value**: ID string (`user_id` or `feed_id`) - not storing JSON
   - **TTL**: Unlimited (updated every hour by clearing and re-adding)

**Benefits of Sorted Sets:**
- Efficient pagination support with `ZREVRANGE`
- Only queries needed range, no need to parse all data
- Better performance compared to storing JSON array
- Can scale to thousands of items

**Realtime vs Background Job**:
- **Realtime Updates**: Consumer updates directly to Redis Sorted Sets when events occur (only increment score)
- **Background Job**: Recalculates from scratch every hour to ensure accuracy and re-sorting
- Background job is still needed to:
  - Recalculate from scratch (ensure accuracy)
  - Re-sort in correct order
  - Handle edge cases (like unlike, delete feed, etc.)

## ⚡ Performance Optimizations

### Database Query Optimization

1. **Like Feed Endpoint** (`POST /api/feed/{feed_id}/like`):
   - Checks if already liked (SELECT query)
   - Verifies feed exists (SELECT query)
   - Inserts like (INSERT query)
   - Uses UNIQUE constraint as fallback to handle race conditions
   - Feed owner is retrieved by Kafka consumer (async), does not block API response
   - Event is published to Kafka for async notification processing

2. **Comment Feed Endpoint** (`POST /api/feed/{feed_id}/comment`):
   - **Before:** 3 queries (SELECT username, INSERT comment, SELECT feed_owner)
   - **After:** 1 query (INSERT comment to MongoDB)
   - Does not query username from MySQL
   - Does not query feed_owner from MySQL
   - Username and feed_owner_id are looked up by Kafka consumer (async)
   - Response does not contain username to reduce payload size

3. **Get Comments Endpoint** (`GET /api/feed/{feed_id}/comments`):
   - **Before:** 1 query MongoDB + N queries MySQL (1 query per comment to get username)
   - **After:** 1 query MongoDB
   - Does not query username for each comment
   - Consumer can batch lookup usernames if needed

4. **Get Feeds Endpoint** (`GET /api/feed`):
   - `comment_count` is retrieved directly from MongoDB collection `comments`
   - Uses `count_documents` to count comments by `feed_id`
   - `like_count` is retrieved from MySQL with COUNT query

5. **Top Statistics API** (`GET /api/top/*`):
   - **Before:** Stored entire JSON array, had to parse all data each request
   - **After:** Uses Redis Sorted Sets (ZSET) with pagination
   - Only queries needed range with `ZREVRANGE` (O(log(N) + M) complexity)
   - No need to parse all data, only parse items in page
   - Efficient pagination support with `page` and `limit` parameters
   - Can scale to thousands of items without affecting performance

6. **Async Processing**:
   - Feed owner and username lookup moved to Kafka consumer
   - Reduces database load in request path
   - Improves response time for API endpoints
   - Consumer can batch process and cache data

## 🔐 Authentication Flow

1. User registers/logs in → Receives JWT token
2. Client sends request with header: `Authorization: Bearer <token>`
3. `AuthenticatedUser` extractor automatically:
   - Parses token from header
   - Verifies token with JWT secret
   - Extracts user_id and email from claims
   - Injects into handler function

## 📦 Main Dependencies

- **actix-web**: Web framework
- **sea-orm**: MySQL ORM (type-safe database operations)
- **mongodb**: MongoDB driver with serde
- **redis**: Redis client
- **rdkafka**: Kafka client
- **jsonwebtoken**: JWT handling
- **bcrypt**: Password hashing
- **utoipa**: OpenAPI/Swagger generation

## 🚀 Setup & Run

### Prerequisites

- Rust (latest stable)
- MySQL server
- MongoDB server
- Redis server
- Kafka server

### Installation

1. Clone repository
2. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```
3. Configure `.env` with database and service information
4. Run service:
   ```bash
   cargo run
   ```

Service will run at `http://localhost:8080` (or according to config).

### Swagger UI

Visit `http://localhost:8080/api/docs` to view and test API.

### Kafka UI (AKHQ)

When running with `docker-compose`, Kafka UI (AKHQ) is available to monitor Kafka topics and messages.

Visit `http://localhost:9000` to access Kafka UI web interface.

**Features:**
- View all Kafka topics
- Browse messages in topics
- View consumer groups
- Monitor topic partitions and offsets
- Inspect message payloads (JSON format)
- Modern and intuitive UI
- Real-time updates

**Usage:**
1. Start services: `docker-compose up -d`
2. Open browser: `http://localhost:9000`
3. Select a topic (e.g., `feed_events`, `user_events`) to view messages
4. Click on a message to see its content
5. View consumer groups and their lag

## 🔧 Development

```bash
# Build
cargo build

# Run
cargo run

# Check errors
cargo check

# Run tests
cargo test

# Run API integration tests
cargo test --test api_test

# Run tests with output
cargo test --test api_test -- --nocapture
```

## 🧪 Testing

### Integration Tests

The project includes comprehensive integration tests for all API endpoints located in `tests/api_test.rs`.

**Test Coverage:**
- ✅ Authentication (signup, login, duplicate email, invalid credentials)
- ✅ Feed operations (create, get, like, unlike, comment, view)
- ✅ Comments retrieval
- ✅ Top statistics endpoints
- ✅ Pagination support
- ✅ Unauthorized access handling
- ✅ Edge cases (like twice, non-existent feed, wrong password)

**Running Tests:**

```bash
# Run all tests
cargo test

# Run only API integration tests
cargo test --test api_test

# Run with detailed output
cargo test --test api_test -- --nocapture
```

**Test Requirements:**
- MySQL database (test database)
- MongoDB instance
- Redis server
- Kafka broker (optional, tests will continue if Kafka is unavailable)

**Note:** Tests use unique identifiers (nanoseconds) to avoid conflicts when running in parallel.

### CI/CD Integration

The project includes GitHub Actions workflow (`.github/workflows/ci.yml`) that automatically:
- Sets up test environment with all required services (MySQL, MongoDB, Redis, Kafka)
- Runs API integration tests on every push and pull request
- Caches Cargo dependencies for faster builds

**Workflow triggers:**
- Push to `main`, `master`, or `develop` branches
- Pull requests to `main`, `master`, or `develop` branches

**CI Services:**
- MySQL 8.0
- MongoDB 7.0
- Redis 7-alpine
- Kafka with Zookeeper

## 📝 Environment Variables

See `.env.example` for all environment variables:

- `SERVER_HOST` / `SERVER_PORT` - Server address
- `JWT_SECRET` - JWT secret key
- `JWT_EXPIRATION_HOURS` - Token expiration
- `MYSQL_*` - MySQL connection
- `MONGODB_URI` / `MONGODB_DATABASE` - MongoDB
- `REDIS_*` - Redis connection
- `KAFKA_BROKERS` / `KAFKA_GROUP_ID` - Kafka

## 📚 Module Details

See README files in each folder for details:
- [Database Layer](./src/db/README.md)
- [Models](./src/models/README.md)
- [Authentication](./src/auth/README.md)
- [API Routes](./src/api/README.md)
- [Kafka Integration](./src/kafka/README.md)
- [Services](./src/services/README.md)
- [Jobs](./src/jobs/README.md)

## 📄 License

MIT
