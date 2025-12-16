# Authentication Module

This module handles all authentication and authorization: JWT tokens, password hashing, and user extraction.

## Structure

```
auth/
├── mod.rs          # Module exports
├── jwt.rs          # JWT token creation & verification
├── password.rs     # Password hashing with bcrypt
└── extractor.rs    # AuthenticatedUser extractor
```

## JWT (`jwt.rs`)

### `Claims`

JWT claims structure containing user information.

```rust
pub struct Claims {
    pub sub: String,     // User ID (subject)
    pub email: String,   // User email
    pub exp: i64,        // Expiration timestamp
    pub iat: i64,        // Issued at timestamp
}
```

**Standard Claims**:
- `sub`: Subject (user ID)
- `exp`: Expiration time
- `iat`: Issued at time

**Custom Claims**:
- `email`: User email for display

### `Claims::new()`

Create new claims with expiration.

```rust
pub fn new(user_id: i64, email: String, expiration_hours: i64) -> Self
```

**Expiration**: Calculated from `Utc::now() + Duration::hours(expiration_hours)`.

### `create_token()`

Create JWT token from claims.

```rust
pub fn create_token(claims: &Claims, secret: &str) -> Result<String>
```

**Algorithm**: HS256 (HMAC SHA-256)

**Secret**: Retrieved from config `JWT_SECRET`.

### `verify_token()`

Verify and parse JWT token.

```rust
pub fn verify_token(token: &str, secret: &str) -> Result<Claims>
```

**Validation**:
- Verify signature
- Check expiration
- Validate algorithm

**Error**: Returns error if token is invalid or expired.

## Password Hashing (`password.rs`)

### `hash_password()`

Hash password with bcrypt.

```rust
pub fn hash_password(password: &str) -> Result<String>
```

**Algorithm**: bcrypt with `DEFAULT_COST` (10 rounds)

**Output**: Bcrypt hash string (format: `$2b$10$...`)

**Security**: 
- Automatically generates salt
- Cost factor 10 (balance between security and performance)

### `verify_password()`

Verify password with hash.

```rust
pub fn verify_password(password: &str, hash: &str) -> Result<bool>
```

**Return**: 
- `Ok(true)` if password is correct
- `Ok(false)` if password is wrong
- `Err` if there is error parsing hash

**Usage**: Used when logging in to verify password.

## AuthenticatedUser Extractor (`extractor.rs`)

### `AuthenticatedUser`

Actix-web extractor that automatically extracts user from JWT token.

```rust
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub email: String,
}
```

### Implementation

Implements `FromRequest` trait to automatically extract from HTTP request.

**Flow**:
1. Get `Authorization` header
2. Parse `Bearer <token>`
3. Verify token with JWT secret
4. Extract claims and create `AuthenticatedUser`
5. Inject into handler function

**Error**: Returns `401 Unauthorized` if:
- No Authorization header
- Token format is wrong
- Token is invalid or expired

### Usage

```rust
pub async fn some_handler(
    user: AuthenticatedUser,  // Automatically extracted
    // ... other params
) -> HttpResponse {
    let user_id = user.user_id;
    let email = user.email;
    // ...
}
```

**Optional**: Can use `Option<AuthenticatedUser>` for endpoints that don't require auth.

## Authentication Flow

### Signup Flow

1. Client sends `POST /api/auth/signup` with email, username, password
2. Server hashes password with `hash_password()`
3. Save user to database
4. Create JWT token with `create_token()`
5. Return token and user info

### Login Flow

1. Client sends `POST /api/auth/login` with email, password
2. Server finds user in database
3. Verify password with `verify_password()`
4. If correct, create JWT token
5. Return token and user info

### Protected Endpoints Flow

1. Client sends request with header: `Authorization: Bearer <token>`
2. `AuthenticatedUser` extractor automatically:
   - Parses header
   - Verifies token
   - Extracts user info
3. Handler function receives `AuthenticatedUser` with user_id and email
4. Handler uses user_id to query/update data

## Security Best Practices

1. **JWT Secret**:
   - Must be a long random string (at least 32 characters)
   - Do not commit to code
   - Use environment variable

2. **Token Expiration**:
   - Set reasonable expiration (24 hours default)
   - Client should refresh token before expiration

3. **Password**:
   - Never log password
   - Hash immediately when received
   - Do not return password hash in response

4. **HTTPS**:
   - Always use HTTPS in production
   - JWT token can be intercepted if using HTTP

5. **Token Storage**:
   - Client should store token in secure storage (localStorage or httpOnly cookie)
   - Do not store in plain text

## Error Handling

- **Invalid Token**: `401 Unauthorized`
- **Missing Token**: `401 Unauthorized`
- **Expired Token**: `401 Unauthorized` (from verify_token)
- **Invalid Password**: `401 Unauthorized` (from login handler)

## Future Enhancements

1. **Refresh Tokens**: Implement refresh token mechanism
2. **Token Blacklist**: Store revoked tokens in Redis
3. **Rate Limiting**: Limit number of login/signup attempts
4. **2FA**: Two-factor authentication
5. **OAuth**: Social login (Google, Facebook, etc.)

