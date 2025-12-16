use crate::db::DbPool;
use crate::entities::{feed, user};
use crate::models::{Comment, FeedView, TopFeed, TopUser};
use chrono::{Duration, Utc};
use log::{error, info};
use mongodb::bson::doc;
use mongodb::Database as MongoDatabase;
use redis::Client as RedisClient;
use sea_orm::{ConnectionTrait, EntityTrait};

pub async fn calculate_top_stats(
    mysql_pool: &DbPool,
    mongo_db: &MongoDatabase,
    redis_client: &RedisClient,
) {
    let seven_days_ago = Utc::now() - Duration::days(7);

    let top_users = calculate_top_users_liked(mysql_pool, seven_days_ago).await;
    let top_feeds_commented = calculate_top_comments(mongo_db, mysql_pool, seven_days_ago).await;
    let top_feeds_viewed = calculate_top_feeds_viewed(mongo_db, mysql_pool, seven_days_ago).await;
    let top_feeds_liked = calculate_top_feeds_liked(mysql_pool, seven_days_ago).await;
    let mut conn = redis_client.get_async_connection().await;
    if let Ok(ref mut conn) = conn {
        let _: Result<(), _> = redis::cmd("DEL")
            .arg("top:users_liked")
            .query_async(conn)
            .await;

        for user in top_users {
            let user_id_str = user.user_id.to_string();
            let score = user.total_likes as f64;
            let _: Result<(), _> = redis::cmd("ZADD")
                .arg("top:users_liked")
                .arg(score)
                .arg(&user_id_str)
                .query_async(conn)
                .await;
        }

        let _: Result<(), _> = redis::cmd("DEL")
            .arg("top:comments")
            .query_async(conn)
            .await;

        for feed in top_feeds_commented {
            let feed_id_str = feed.feed_id.to_string();
            let score = feed.count as f64;
            let _: Result<(), _> = redis::cmd("ZADD")
                .arg("top:comments")
                .arg(score)
                .arg(&feed_id_str)
                .query_async(conn)
                .await;
        }

        let _: Result<(), _> = redis::cmd("DEL")
            .arg("top:feeds_viewed")
            .query_async(conn)
            .await;

        for feed in top_feeds_viewed {
            let feed_id_str = feed.feed_id.to_string();
            let score = feed.count as f64;
            let _: Result<(), _> = redis::cmd("ZADD")
                .arg("top:feeds_viewed")
                .arg(score)
                .arg(&feed_id_str)
                .query_async(conn)
                .await;
        }

        let _: Result<(), _> = redis::cmd("DEL")
            .arg("top:feeds_liked")
            .query_async(conn)
            .await;

        for feed in top_feeds_liked {
            let feed_id_str = feed.feed_id.to_string();
            let score = feed.count as f64;
            let _: Result<(), _> = redis::cmd("ZADD")
                .arg("top:feeds_liked")
                .arg(score)
                .arg(&feed_id_str)
                .query_async(conn)
                .await;
        }
    }

    info!("Top stats calculated and stored in Redis");
}

async fn calculate_top_users_liked(
    pool: &DbPool,
    since: chrono::DateTime<chrono::Utc>,
) -> Vec<TopUser> {
    let query = r#"
        SELECT 
            f.user_id,
            u.username,
            COUNT(fl.id) as total_likes
        FROM feeds f
        INNER JOIN feed_likes fl ON f.id = fl.feed_id
        INNER JOIN users u ON f.user_id = u.id
        WHERE fl.created_at >= ?
        GROUP BY f.user_id, u.username
        ORDER BY total_likes DESC
        LIMIT 1000
    "#;

    // Use raw SQL for complex aggregation with SeaORM
    let stmt = sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::MySql,
        query,
        [sea_orm::Value::ChronoDateTimeUtc(Some(since.into()))],
    );

    match pool.query_all(stmt).await {
        Ok(rows) => rows
            .iter()
            .filter_map(|row| {
                Some(TopUser {
                    user_id: row.try_get::<i64>("", "user_id").ok()?,
                    username: row.try_get::<String>("", "username").ok()?,
                    total_likes: row.try_get::<i64>("", "total_likes").ok()?,
                })
            })
            .collect(),
        Err(e) => {
            error!("Error calculating top users liked: {:?}", e);
            Vec::new()
        }
    }
}

async fn calculate_top_comments(
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    since: chrono::DateTime<chrono::Utc>,
) -> Vec<TopFeed> {
    let collection = mongo_db.collection::<Comment>("comments");
    let filter = doc! {
        "created_at": {
            "$gte": since.timestamp()
        }
    };

    let mut cursor = match collection.find(filter, None).await {
        Ok(c) => c,
        Err(e) => {
            error!("Error fetching comments: {:?}", e);
            return Vec::new();
        }
    };

    let mut comment_counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();

    while let Ok(true) = cursor.advance().await {
        match cursor.deserialize_current() {
            Ok(comment) => {
                *comment_counts.entry(comment.feed_id).or_insert(0) += 1;
            }
            Err(_) => continue,
        }
    }

    let mut top_feeds = Vec::new();
    let mut sorted: Vec<_> = comment_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    for (feed_id, count) in sorted.iter().take(1000) {
        // Get feed info using SeaORM
        let feed_info = if let Ok(Some(feed_model)) =
            feed::Entity::find_by_id(**feed_id).one(mysql_pool).await
        {
            if let Ok(Some(user_model)) = user::Entity::find_by_id(feed_model.user_id)
                .one(mysql_pool)
                .await
            {
                Some((
                    feed_model.id,
                    feed_model.user_id,
                    user_model.username,
                    feed_model.content,
                ))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((feed_id_val, user_id, username, content)) = feed_info {
            top_feeds.push(TopFeed {
                feed_id: feed_id_val,
                user_id,
                username,
                content,
                count: **count,
            });
        }
    }

    top_feeds
}

async fn calculate_top_feeds_viewed(
    mongo_db: &MongoDatabase,
    mysql_pool: &DbPool,
    since: chrono::DateTime<chrono::Utc>,
) -> Vec<TopFeed> {
    let collection = mongo_db.collection::<FeedView>("feed_views");
    let filter = doc! {
        "viewed_at": {
            "$gte": since.timestamp()
        }
    };

    let mut cursor = match collection.find(filter, None).await {
        Ok(c) => c,
        Err(e) => {
            error!("Error fetching feed views: {:?}", e);
            return Vec::new();
        }
    };

    let mut view_counts: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();

    while let Ok(true) = cursor.advance().await {
        match cursor.deserialize_current() {
            Ok(view) => {
                *view_counts.entry(view.feed_id).or_insert(0) += 1;
            }
            Err(_) => continue,
        }
    }

    let mut top_feeds = Vec::new();
    let mut sorted: Vec<_> = view_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    for (feed_id, count) in sorted.iter().take(1000) {
        // Get feed and user info using SeaORM
        let feed: Option<(i64, String)> = feed::Entity::find_by_id(**feed_id)
            .one(mysql_pool)
            .await
            .ok()
            .flatten()
            .map(|feed_model| (feed_model.user_id, feed_model.content));

        if let Some((user_id, content)) = feed {
            let username: Option<String> = user::Entity::find_by_id(user_id)
                .one(mysql_pool)
                .await
                .ok()
                .flatten()
                .map(|user_model| user_model.username);

            if let Some(username) = username {
                top_feeds.push(TopFeed {
                    feed_id: **feed_id,
                    user_id,
                    username,
                    content,
                    count: **count,
                });
            }
        }
    }

    top_feeds
}

async fn calculate_top_feeds_liked(
    pool: &DbPool,
    since: chrono::DateTime<chrono::Utc>,
) -> Vec<TopFeed> {
    let query = r#"
        SELECT 
            f.id,
            f.user_id,
            u.username,
            f.content,
            COUNT(fl.id) as like_count
        FROM feeds f
        INNER JOIN feed_likes fl ON f.id = fl.feed_id
        INNER JOIN users u ON f.user_id = u.id
        WHERE fl.created_at >= ?
        GROUP BY f.id, f.user_id, u.username, f.content
        ORDER BY like_count DESC
        LIMIT 1000
    "#;

    // Use raw SQL for complex aggregation with SeaORM
    let stmt = sea_orm::Statement::from_sql_and_values(
        sea_orm::DatabaseBackend::MySql,
        query,
        [sea_orm::Value::ChronoDateTimeUtc(Some(since.into()))],
    );

    match pool.query_all(stmt).await {
        Ok(rows) => rows
            .iter()
            .filter_map(|row| {
                Some(TopFeed {
                    feed_id: row.try_get::<i64>("", "id").ok()?,
                    user_id: row.try_get::<i64>("", "user_id").ok()?,
                    username: row.try_get::<String>("", "username").ok()?,
                    content: row.try_get::<String>("", "content").ok()?,
                    count: row.try_get::<i64>("", "like_count").ok()?,
                })
            })
            .collect(),
        Err(e) => {
            error!("Error calculating top feeds liked: {:?}", e);
            Vec::new()
        }
    }
}
