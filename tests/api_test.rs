// Integration tests for API endpoints
// These tests can be run in CI/CD pipelines (e.g., GitHub Actions)
// Run with: cargo test --test api_test

use actix_web::{http::StatusCode, test, web, App};
use example_rust_web_service::{
    api, config::Config, db,
    kafka::KafkaProducer,
    models::{
        AuthResponse, FeedResponse,
    },
};
use serde_json::json;

/// Generate unique test identifier using nanoseconds for better uniqueness
fn generate_test_id() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string()
}

/// Helper function to create a test app
async fn create_test_app() -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let config = Config::from_env().expect("Failed to load configuration");
    let mysql_pool = db::create_mysql_pool(&config)
        .await
        .expect("Failed to create MySQL pool");
    let mongodb_db = db::create_mongodb_client(&config)
        .await
        .expect("Failed to create MongoDB client");
    let redis_client = db::create_redis_client(&config).expect("Failed to create Redis client");
    let kafka_producer = KafkaProducer::new(&config).expect("Failed to create Kafka producer");

    App::new()
        .app_data(web::Data::new(config))
        .app_data(web::Data::new(mysql_pool))
        .app_data(web::Data::new(mongodb_db))
        .app_data(web::Data::new(redis_client))
        .app_data(web::Data::new(kafka_producer))
        .service(
            web::scope("/api")
                .service(
                    web::scope("/auth")
                        .route("/signup", web::post().to(api::auth::signup))
                        .route("/login", web::post().to(api::auth::login)),
                )
                .service(
                    web::scope("/feed")
                        .route("", web::post().to(api::feed::create_feed))
                        .route("", web::get().to(api::feed::get_feeds))
                        .route("/{feed_id}/like", web::post().to(api::feed::like_feed))
                        .route("/{feed_id}/like", web::delete().to(api::feed::unlike_feed))
                        .route(
                            "/{feed_id}/comment",
                            web::post().to(api::feed::comment_feed),
                        )
                        .route(
                            "/{feed_id}/comments",
                            web::get().to(api::feed::get_comments),
                        )
                        .route("/{feed_id}/view", web::post().to(api::feed::view_feed)),
                )
                .service(
                    web::scope("/notify")
                        .route("", web::get().to(api::notify::get_notifications))
                        .route(
                            "/{notification_id}/read",
                            web::put().to(api::notify::mark_notification_read),
                        ),
                )
                .service(
                    web::scope("/top")
                        .route("/users-liked", web::get().to(api::top::get_top_users_liked))
                        .route(
                            "/feeds-commented",
                            web::get().to(api::top::get_top_comments),
                        )
                        .route(
                            "/feeds-viewed",
                            web::get().to(api::top::get_top_feeds_viewed),
                        )
                        .route("/feeds-liked", web::get().to(api::top::get_top_feeds_liked)),
                ),
        )
}

#[actix_web::test]
async fn test_signup() {
    let app = test::init_service(create_test_app().await).await;

    // Generate unique email for test
    let test_id = generate_test_id();
    let email = format!("test{}@example.com", test_id);
    let username = format!("testuser{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "Signup should return 201 CREATED"
    );

    let body: AuthResponse = test::read_body_json(resp).await;
    assert!(!body.token.is_empty(), "Token should not be empty");
    assert_eq!(body.user.email, email, "Email should match");
    assert_eq!(body.user.username, username, "Username should match");
}

#[actix_web::test]
async fn test_signup_duplicate_email() {
    let app = test::init_service(create_test_app().await).await;

    let test_id = generate_test_id();
    let email = format!("duplicate{}@example.com", test_id);
    let username = format!("user{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    // First signup
    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Try to signup again with same email
    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::CONFLICT,
        "Duplicate signup should return 409 CONFLICT"
    );
}

#[actix_web::test]
async fn test_login() {
    let app = test::init_service(create_test_app().await).await;

    // First create a user
    let test_id = generate_test_id();
    let email = format!("login{}@example.com", test_id);
    let username = format!("loginuser{}", test_id);
    let password = "password123".to_string();

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": password
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Now try to login
    let login_req = json!({
        "email": email,
        "password": password
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(&login_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Login should return 200 OK"
    );

    let body: AuthResponse = test::read_body_json(resp).await;
    assert!(!body.token.is_empty(), "Token should not be empty");
    assert_eq!(body.user.email, email, "Email should match");
}

#[actix_web::test]
async fn test_login_invalid_credentials() {
    let app = test::init_service(create_test_app().await).await;

    let login_req = json!({
        "email": "nonexistent@example.com",
        "password": "wrongpassword"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(&login_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_create_feed() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("feeduser{}@example.com", test_id);
    let username = format!("feeduser{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Test feed content"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Create feed should return 200 OK"
    );

    let feed: FeedResponse = test::read_body_json(resp).await;
    assert_eq!(feed.content, "Test feed content", "Feed content should match");
    assert_eq!(feed.like_count, 0, "New feed should have 0 likes");
    assert_eq!(feed.comment_count, 0, "New feed should have 0 comments");
    assert_eq!(feed.is_liked, false, "New feed should not be liked");
}

#[actix_web::test]
async fn test_create_feed_unauthorized() {
    let app = test::init_service(create_test_app().await).await;

    let feed_req = json!({
        "content": "Test feed content"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn test_get_feeds() {
    let app = test::init_service(create_test_app().await).await;

    // Get feeds without authentication (should work)
    let req = test::TestRequest::get()
        .uri("/api/feed")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get feeds should return 200 OK"
    );

    let _feeds: Vec<FeedResponse> = test::read_body_json(resp).await;
    // Should return an array (can be empty)
    // Type check verifies it's a Vec<FeedResponse>
}

#[actix_web::test]
async fn test_like_feed() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("likeuser{}@example.com", test_id);
    let username = format!("likeuser{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Feed to like"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let feed: FeedResponse = test::read_body_json(resp).await;
    let feed_id = feed.id;

    // Like the feed
    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/like", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Like feed should return 200 OK"
    );
}

#[actix_web::test]
async fn test_comment_feed() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("commentuser{}@example.com", test_id);
    let username = format!("commentuser{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Feed to comment"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let feed: FeedResponse = test::read_body_json(resp).await;
    let feed_id = feed.id;

    // Comment on the feed
    let comment_req = json!({
        "content": "This is a test comment"
    });

    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/comment", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&comment_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Comment feed should return 200 OK"
    );
}

#[actix_web::test]
async fn test_view_feed() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("viewuser{}@example.com", test_id);
    let username = format!("viewuser{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Feed to view"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let feed: FeedResponse = test::read_body_json(resp).await;
    let feed_id = feed.id;

    // View the feed (no auth required)
    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/view", feed_id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "View feed should return 200 OK"
    );
}

#[actix_web::test]
async fn test_get_top_feeds_liked() {
    let app = test::init_service(create_test_app().await).await;

    let req = test::TestRequest::get()
        .uri("/api/top/feeds-liked")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get top feeds liked should return 200 OK"
    );
}

#[actix_web::test]
async fn test_get_top_users_liked() {
    let app = test::init_service(create_test_app().await).await;

    let req = test::TestRequest::get()
        .uri("/api/top/users-liked")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get top users liked should return 200 OK"
    );
}

#[actix_web::test]
async fn test_get_top_feeds_commented() {
    let app = test::init_service(create_test_app().await).await;

    let req = test::TestRequest::get()
        .uri("/api/top/feeds-commented")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get top feeds commented should return 200 OK"
    );
}

#[actix_web::test]
async fn test_get_top_feeds_viewed() {
    let app = test::init_service(create_test_app().await).await;

    let req = test::TestRequest::get()
        .uri("/api/top/feeds-viewed")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get top feeds viewed should return 200 OK"
    );
}

#[actix_web::test]
async fn test_unlike_feed() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("unlikeuser{}@example.com", test_id);
    let username = format!("unlikeuser{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Feed to unlike"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let feed: FeedResponse = test::read_body_json(resp).await;
    let feed_id = feed.id;

    // Like the feed first
    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/like", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Like feed should return 200 OK"
    );

    // Unlike the feed
    let req = test::TestRequest::delete()
        .uri(&format!("/api/feed/{}/like", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Unlike feed should return 200 OK"
    );
}

#[actix_web::test]
async fn test_get_comments() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("commentget{}@example.com", test_id);
    let username = format!("commentget{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Feed for comments"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let feed: FeedResponse = test::read_body_json(resp).await;
    let feed_id = feed.id;

    // Add a comment
    let comment_req = json!({
        "content": "Test comment"
    });

    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/comment", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&comment_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Add comment should return 200 OK"
    );

    // Get comments
    let req = test::TestRequest::get()
        .uri(&format!("/api/feed/{}/comments", feed_id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get comments should return 200 OK"
    );

    let comments: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert!(comments.len() > 0, "Comments list should not be empty");
}

#[actix_web::test]
async fn test_get_feeds_with_pagination() {
    let app = test::init_service(create_test_app().await).await;

    // Test pagination parameters
    let req = test::TestRequest::get()
        .uri("/api/feed?page=1&limit=10")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Get feeds with pagination should return 200 OK"
    );

    let feeds: Vec<FeedResponse> = test::read_body_json(resp).await;
    assert!(
        feeds.len() <= 10,
        "Feeds with limit=10 should return at most 10 items"
    );
}

#[actix_web::test]
async fn test_like_feed_twice() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("liketwice{}@example.com", test_id);
    let username = format!("liketwice{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Create feed
    let feed_req = json!({
        "content": "Feed to like twice"
    });

    let req = test::TestRequest::post()
        .uri("/api/feed")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(&feed_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let feed: FeedResponse = test::read_body_json(resp).await;
    let feed_id = feed.id;

    // Like the feed first time
    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/like", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "First like should return 200 OK"
    );

    // Try to like again (should return "Already liked")
    let req = test::TestRequest::post()
        .uri(&format!("/api/feed/{}/like", feed_id))
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Second like should return 200 OK (already liked)"
    );
}

#[actix_web::test]
async fn test_like_nonexistent_feed() {
    let app = test::init_service(create_test_app().await).await;

    // Create user and get token
    let test_id = generate_test_id();
    let email = format!("likenonex{}@example.com", test_id);
    let username = format!("likenonex{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "password123"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    let body: AuthResponse = test::read_body_json(resp).await;
    let token = body.token;

    // Try to like a non-existent feed
    let req = test::TestRequest::post()
        .uri("/api/feed/999999/like")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_login_wrong_password() {
    let app = test::init_service(create_test_app().await).await;

    // Create user first
    let test_id = generate_test_id();
    let email = format!("wrongpass{}@example.com", test_id);
    let username = format!("wrongpass{}", test_id);

    let signup_req = json!({
        "email": email,
        "username": username,
        "password": "correctpassword"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/signup")
        .set_json(&signup_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Try to login with wrong password
    let login_req = json!({
        "email": email,
        "password": "wrongpassword"
    });

    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(&login_req)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

