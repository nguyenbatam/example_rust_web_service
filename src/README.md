# Source Code Overview

Overview of source code structure and how modules work together.

## Entry Point (`main.rs`)

This file is the starting point of the application, responsible for:

1. **Initialization**:
   - Load configuration from `.env`
   - Initialize logger
   - Create connections to all databases (MySQL, MongoDB, Redis)
   - Create Kafka producer and consumers

2. **Background Services**:
   - Start Kafka consumers to listen for events
   - Start background jobs (top statistics calculation)

3. **HTTP Server**:
   - Setup Actix-web server
   - Register all API routes
   - Setup Swagger UI
   - Start listening on configured port

### Flow on startup

```
1. Load Config (.env)
   ↓
2. Initialize Databases
   ├── MySQL Pool
   ├── MongoDB Client
   └── Redis Client
   ↓
3. Initialize Kafka
   ├── Producer
   └── Consumers
   ↓
4. Start Background Jobs
   ├── Top Stats Job (every hour)
   └── Kafka Event Handlers
   ↓
5. Start HTTP Server
   ├── Register Routes
   ├── Setup Swagger
   └── Listen on Port
```

## Configuration (`config.rs`)

Manages all configuration from environment variables using `dotenv`.

### Config Structure

```rust
pub struct Config {
    pub server: ServerConfig,    // Host, port
    pub jwt: JwtConfig,         // Secret, expiration_hours
    pub mysql: MysqlConfig,     // Host, port, user, password, database
    pub mongodb: MongodbConfig, // URI, database
    pub redis: RedisConfig,     // Host, port, password (optional)
    pub kafka: KafkaConfig,     // Brokers, group_id
}
```

### Helper Methods

- `from_env()`: Load configuration from environment variables (with defaults)
- `mysql_url()`: Create MySQL connection string (`mysql://user:password@host:port/database`)
- `redis_url()`: Create Redis connection string (with or without password)

### Environment Variables

All configuration is loaded from `.env` file or environment variables:
- `SERVER_HOST`, `SERVER_PORT`
- `JWT_SECRET`, `JWT_EXPIRATION_HOURS`
- `MYSQL_HOST`, `MYSQL_PORT`, `MYSQL_USER`, `MYSQL_PASSWORD`, `MYSQL_DATABASE`
- `MONGODB_URI`, `MONGODB_DATABASE`
- `REDIS_HOST`, `REDIS_PORT`, `REDIS_PASSWORD` (optional)
- `KAFKA_BROKERS`, `KAFKA_GROUP_ID`

## Module Dependencies

```
main.rs
├── config.rs          (Configuration)
├── db/                (Database connections)
│   ├── mysql.rs
│   ├── mongodb.rs
│   └── redis.rs
├── models/            (Data models)
│   ├── user.rs
│   └── feed.rs
├── auth/              (Authentication)
│   ├── jwt.rs
│   ├── password.rs
│   └── extractor.rs
├── api/               (API routes)
│   ├── auth.rs
│   ├── feed.rs
│   ├── notify.rs
│   └── top.rs
├── kafka/             (Kafka integration)
│   ├── producer.rs
│   └── consumer.rs
├── services/          (Business services)
│   └── notification.rs
└── jobs/              (Background jobs)
    ├── handlers.rs
    └── top_stats.rs
```

## Data Flow

### Request Flow

```
HTTP Request
    ↓
Actix Router
    ↓
AuthenticatedUser Extractor (if protected)
    ↓
API Handler
    ├── Validate Input
    ├── Query Database
    │   ├── MySQL (users, feeds, likes)
    │   └── MongoDB (comments, notifications)
    ├── Business Logic
    ├── Publish Kafka Event (if needed)
    └── Return Response
```

### Event Flow

```
API Action (like, comment, etc.)
    ↓
Publish Kafka Event
    ↓
Kafka Consumer Receives
    ↓
Event Handler
    ├── Notification Service (create notification)
    └── Job Handlers (other processing)
```

### Background Job Flow

```
Timer (every hour)
    ↓
calculate_top_stats()
    ├── Query MySQL (users, feeds, likes)
    ├── Query MongoDB (comments, views)
    ├── Calculate Top 10
    ├── Serialize to JSON
    └── Store in Redis
```

## Error Handling Strategy

1. **API Level**: 
   - Return appropriate HTTP status codes
   - Log errors
   - Don't expose internal errors

2. **Database Level**:
   - Use Result types
   - Handle connection errors gracefully
   - Retry logic if needed

3. **Kafka Level**:
   - Non-blocking producer errors
   - Graceful consumer error handling
   - Log all errors

4. **Job Level**:
   - Don't crash on errors
   - Log and continue
   - Alert on repeated failures

## Testing Strategy

### Unit Tests
- Test individual functions
- Mock dependencies
- Test error cases

### Integration Tests
- Test API endpoints
- Test database operations
- Test Kafka integration

### E2E Tests
- Test full flows
- Test with real databases (test environment)

## Performance Considerations

1. **Database**:
   - Connection pooling
   - Indexes on frequently queried columns
   - Query optimization

2. **Caching**:
   - Redis for top statistics
   - Consider caching frequently accessed data

3. **Async**:
   - All I/O operations are async
   - Non-blocking Kafka operations
   - Background job processing

4. **Scalability**:
   - Stateless API (can scale horizontally)
   - Shared Redis cache
   - Kafka for event distribution

## Security Considerations

1. **Authentication**:
   - JWT tokens with expiration
   - Secure password hashing (bcrypt)
   - Token validation on every request

2. **Input Validation**:
   - Validate all user inputs
   - SQL injection prevention (parameterized queries)
   - XSS prevention

3. **Secrets**:
   - Never commit secrets
   - Use environment variables
   - Rotate secrets regularly

4. **HTTPS**:
   - Always use HTTPS in production
   - Secure cookie settings

## Monitoring & Logging

1. **Logging**:
   - Use structured logging
   - Log levels: info, warn, error
   - Log important events

2. **Metrics**:
   - Request count
   - Response times
   - Error rates
   - Database query times

3. **Health Checks**:
   - Database connectivity
   - Kafka connectivity
   - Redis connectivity

## Deployment Considerations

1. **Environment**:
   - Separate dev, staging, production
   - Different configs per environment

2. **Database Migrations**:
   - Use migration tools
   - Backup before migration

3. **Rolling Updates**:
   - Zero-downtime deployment
   - Health checks
   - Graceful shutdown

4. **Scaling**:
   - Horizontal scaling (multiple instances)
   - Load balancer
   - Database read replicas if needed

## Future Improvements

1. **Features**:
   - Real-time notifications (WebSocket)
   - File uploads
   - Search functionality
   - Pagination improvements

2. **Performance**:
   - Database query optimization
   - Caching strategy
   - CDN for static assets

3. **Reliability**:
   - Circuit breakers
   - Retry mechanisms
   - Dead letter queues

4. **Observability**:
   - Distributed tracing
   - APM integration
   - Better metrics

