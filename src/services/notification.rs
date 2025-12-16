use crate::db::DbPool;
use crate::entities::{feed, user};
use crate::models::{Notification, NotificationType};
use chrono::Utc;
use log::{error, info};
use mongodb::Database as MongoDatabase;
use redis::Client as RedisClient;
use sea_orm::EntityTrait;
use serde_json::Value;
use uuid::Uuid;

pub async fn handle_feed_liked_event(
    event_data: &Value,
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    redis_client: &RedisClient,
) {
    if let (Some(user_id), Some(feed_id)) = (
        event_data.get("user_id").and_then(|v| v.as_i64()),
        event_data.get("feed_id").and_then(|v| v.as_i64()),
    ) {
        // Get feed owner info using SeaORM
        let feed_owner_info =
            if let Ok(Some(feed_model)) = feed::Entity::find_by_id(feed_id).one(mysql_pool).await {
                if let Ok(Some(user_model)) = user::Entity::find_by_id(feed_model.user_id)
                    .one(mysql_pool)
                    .await
                {
                    Some((feed_model.user_id, user_model.username))
                } else {
                    None
                }
            } else {
                None
            };

        let (feed_owner_id, feed_owner_username) = match feed_owner_info {
            Some((owner_id, username)) => (owner_id, username),
            None => {
                error!("Feed {} not found when processing like event", feed_id);
                return;
            }
        };

        update_top_users_liked_realtime(redis_client, feed_owner_id, &feed_owner_username).await;
        update_top_feeds_liked_realtime(
            redis_client,
            feed_id,
            feed_owner_id,
            &feed_owner_username,
            mysql_pool,
        )
        .await;

        if feed_owner_id == user_id {
            return;
        }

        // Get username using SeaORM
        let username: Option<String> = user::Entity::find_by_id(user_id)
            .one(mysql_pool)
            .await
            .ok()
            .flatten()
            .map(|user_model| user_model.username);

        if let Some(username) = username {
            let content = format!("{} liked your feed", username);
            let notification = Notification {
                id: Some(Uuid::new_v4().to_string()),
                user_id: feed_owner_id,
                from_user_id: user_id,
                from_username: username,
                feed_id,
                notification_type: NotificationType::Like,
                content,
                created_at: Utc::now(),
                is_read: false,
            };

            let collection = mongo_db.collection::<Notification>("notifications");
            if let Err(e) = collection.insert_one(&notification, None).await {
                error!("Failed to create notification: {:?}", e);
            } else {
                info!(
                    "Created like notification for user {} from user {}",
                    feed_owner_id, user_id
                );
            }
        }
    }
}

async fn update_top_feeds_liked_realtime(
    redis_client: &RedisClient,
    feed_id: i64,
    _user_id: i64,
    _username: &str,
    _mysql_pool: &DbPool,
) {
    let mut conn = match redis_client.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!(
                "Failed to get Redis connection for top:feeds_liked: {:?}",
                e
            );
            return;
        }
    };

    // Simply increment score for feed_id - much simpler and faster!
    let feed_id_str = feed_id.to_string();
    let _: Result<(), _> = redis::cmd("ZINCRBY")
        .arg("top:feeds_liked")
        .arg(1.0)
        .arg(&feed_id_str)
        .query_async(&mut conn)
        .await;
}

async fn update_top_feeds_commented_realtime(
    redis_client: &RedisClient,
    feed_id: i64,
    _mysql_pool: &DbPool,
) {
    let mut conn = match redis_client.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to get Redis connection for top:comments: {:?}", e);
            return;
        }
    };

    let feed_id_str = feed_id.to_string();
    match redis::cmd("ZINCRBY")
        .arg("top:comments")
        .arg(1.0)
        .arg(&feed_id_str)
        .query_async::<_, f64>(&mut conn)
        .await
    {
        Ok(score) => {
            info!(
                "Updated top:comments for feed {}: new score = {}",
                feed_id, score
            );
        }
        Err(e) => {
            error!(
                "Failed to update top:comments for feed {}: {:?}",
                feed_id, e
            );
        }
    }
}

async fn update_top_feeds_viewed_realtime(redis_client: &RedisClient, feed_id: i64) {
    let mut conn = match redis_client.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!(
                "Failed to get Redis connection for top:feeds_viewed: {:?}",
                e
            );
            return;
        }
    };

    let feed_id_str = feed_id.to_string();
    let _: Result<(), _> = redis::cmd("ZINCRBY")
        .arg("top:feeds_viewed")
        .arg(1.0)
        .arg(&feed_id_str)
        .query_async(&mut conn)
        .await;
}

pub async fn handle_feed_viewed_event(event_data: &Value, redis_client: &RedisClient) {
    if let Some(feed_id) = event_data.get("feed_id").and_then(|v| v.as_i64()) {
        update_top_feeds_viewed_realtime(redis_client, feed_id).await;
        info!("Updated top:feeds_viewed for feed {}", feed_id);
    }
}

pub async fn handle_feed_commented_event(
    event_data: &Value,
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    redis_client: &RedisClient,
) {
    info!("Processing feed commented event: {:?}", event_data);
    if let (Some(user_id), Some(feed_id), Some(content)) = (
        event_data.get("user_id").and_then(|v| v.as_i64()),
        event_data.get("feed_id").and_then(|v| v.as_i64()),
        event_data.get("content").and_then(|v| v.as_str()),
    ) {
        info!(
            "Comment event - feed_id: {}, user_id: {}, content: {}",
            feed_id, user_id, content
        );
        // Update top:comments first (always update, even if notification creation fails)
        update_top_feeds_commented_realtime(redis_client, feed_id, mysql_pool).await;

        // Get feed owner info using SeaORM
        let feed_owner_info =
            if let Ok(Some(feed_model)) = feed::Entity::find_by_id(feed_id).one(mysql_pool).await {
                Some(feed_model.user_id)
            } else {
                None
            };

        let feed_owner_id = match feed_owner_info {
            Some(owner_id) => owner_id,
            None => {
                error!("Feed {} not found when processing comment event", feed_id);
                return;
            }
        };

        if feed_owner_id == user_id {
            return;
        }

        // Get username using SeaORM
        let username: Option<String> = user::Entity::find_by_id(user_id)
            .one(mysql_pool)
            .await
            .ok()
            .flatten()
            .map(|user_model| user_model.username);

        if let Some(username) = username {
            let notification = Notification {
                id: Some(Uuid::new_v4().to_string()),
                user_id: feed_owner_id,
                from_user_id: user_id,
                from_username: username,
                feed_id,
                notification_type: NotificationType::Comment,
                content: content.to_string(),
                created_at: Utc::now(),
                is_read: false,
            };

            let collection = mongo_db.collection::<Notification>("notifications");
            if let Err(e) = collection.insert_one(&notification, None).await {
                error!("Failed to create notification: {:?}", e);
            } else {
                info!(
                    "Created comment notification for user {} from user {}",
                    feed_owner_id, user_id
                );
            }
        }
    }
}

async fn update_top_users_liked_realtime(
    redis_client: &RedisClient,
    user_id: i64,
    _username: &str,
) {
    let mut conn = match redis_client.get_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!(
                "Failed to get Redis connection for top:users_liked: {:?}",
                e
            );
            return;
        }
    };

    let user_id_str = user_id.to_string();
    let _: Result<(), _> = redis::cmd("ZINCRBY")
        .arg("top:users_liked")
        .arg(1.0)
        .arg(&user_id_str)
        .query_async(&mut conn)
        .await;
}
