use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct Feed {
    pub id: Option<i64>,
    pub user_id: i64,
    pub content: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFeedRequest {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeedResponse {
    pub id: i64,
    pub user_id: i64,
    pub content: String,
    pub like_count: i64,
    pub comment_count: i64,
    pub is_liked: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CommentRequest {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Comment {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub feed_id: i64,
    pub user_id: i64,
    pub content: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CommentResponse {
    pub id: String,
    pub feed_id: i64,
    pub user_id: i64,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Like,
    Comment,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Notification {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub user_id: i64,      // User receiving notification
    pub from_user_id: i64, // User performing action
    pub from_username: String,
    pub feed_id: i64,
    pub notification_type: NotificationType,
    pub content: String, // Message displayed to user (e.g., "John liked your feed" or comment content)
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_read: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationResponse {
    pub id: String,
    pub from_user_id: i64,
    pub from_username: String,
    pub feed_id: i64,
    pub notification_type: NotificationType,
    pub content: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_read: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FeedView {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub feed_id: i64,
    pub user_id: i64,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub viewed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TopUser {
    pub user_id: i64,
    pub username: String,
    pub total_likes: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TopFeed {
    pub feed_id: i64,
    pub user_id: i64,
    pub username: String,
    pub content: String,
    pub count: i64,
}
