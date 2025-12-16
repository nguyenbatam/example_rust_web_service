use chrono::Utc;
use serde::de::Error;
use serde::{Deserialize, Serialize};

/// Enum defining event types related to Feed
/// Serializes/deserializes as snake_case: "created", "liked", "commented", "viewed"
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedEventType {
    Created,
    Liked,
    Commented,
    Viewed,
}

/// Enum defining event types related to User
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserEventType {
    UserCreated,
}

/// Event when a feed is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedCreatedEvent {
    #[serde(rename = "event_type")]
    pub event_type: FeedEventType,
    pub feed_id: u64,
    pub user_id: i64,
    pub content: String,
    pub timestamp: String,
}

impl FeedCreatedEvent {
    pub fn new(feed_id: u64, user_id: i64, content: String) -> Self {
        Self {
            event_type: FeedEventType::Created,
            feed_id,
            user_id,
            content,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// Event when a feed is liked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedLikedEvent {
    #[serde(rename = "event_type")]
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64,
    pub timestamp: String,
}

impl FeedLikedEvent {
    pub fn new(feed_id: i64, user_id: i64) -> Self {
        Self {
            event_type: FeedEventType::Liked,
            feed_id,
            user_id,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// Event when a feed is commented
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedCommentedEvent {
    #[serde(rename = "event_type")]
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64,
    pub comment_id: String,
    pub content: String,
    pub timestamp: String,
}

impl FeedCommentedEvent {
    pub fn new(feed_id: i64, user_id: i64, comment_id: String, content: String) -> Self {
        Self {
            event_type: FeedEventType::Commented,
            feed_id,
            user_id,
            comment_id,
            content,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// Event when a feed is viewed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedViewedEvent {
    #[serde(rename = "event_type")]
    pub event_type: FeedEventType,
    pub feed_id: i64,
    pub user_id: i64, // 0 if anonymous
    pub timestamp: String,
}

impl FeedViewedEvent {
    pub fn new(feed_id: i64, user_id: i64) -> Self {
        Self {
            event_type: FeedEventType::Viewed,
            feed_id,
            user_id,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// Event when a user is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCreatedEvent {
    #[serde(rename = "event_type")]
    pub event_type: UserEventType,
    pub user_id: u64,
    pub email: String,
    pub username: String,
    pub timestamp: String,
}

impl UserCreatedEvent {
    pub fn new(user_id: u64, email: String, username: String) -> Self {
        Self {
            event_type: UserEventType::UserCreated,
            user_id,
            email,
            username,
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

/// Helper function to parse event from JSON string
/// Uses serde deserialization directly for type safety
pub fn parse_feed_event(
    payload: &str,
) -> Result<(FeedEventType, serde_json::Value), serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(payload)?;

    // Extract and deserialize event_type directly using serde
    let event_type = value
        .get("event_type")
        .ok_or_else(|| serde_json::Error::custom("Missing event_type field"))?
        .clone();

    let event_type: FeedEventType = serde_json::from_value(event_type).map_err(|e| {
        log::warn!("Failed to deserialize event_type: {:?}", e);
        e
    })?;

    Ok((event_type, value))
}
