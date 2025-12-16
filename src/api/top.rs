use crate::db::DbPool;
use crate::entities::{feed, user};
use crate::models::{TopFeed, TopUser};
use actix_web::{web, HttpResponse, Result as ActixResult};
use log;
use redis::Client as RedisClient;
use sea_orm::EntityTrait;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct TopQuery {
    #[schema(example = 1)]
    pub page: Option<u64>,
    #[schema(example = 10)]
    pub limit: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/top/users-liked",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 10)")
    ),
    responses(
        (status = 200, description = "Top users liked", body = Vec<TopUser>)
    ),
    tag = "top"
)]
pub async fn get_top_users_liked(
    redis_client: web::Data<RedisClient>,
    pool: web::Data<DbPool>,
    query: web::Query<TopQuery>,
) -> ActixResult<HttpResponse> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let start = ((page - 1) * limit) as i64;
    let stop = start + limit as i64 - 1;

    let mut conn = redis_client
        .get_async_connection()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let results: Vec<(String, f64)> = redis::cmd("ZREVRANGE")
        .arg("top:users_liked")
        .arg(start)
        .arg(stop)
        .arg("WITHSCORES")
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    if results.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<TopUser>::new()));
    }

    let user_ids: Vec<i64> = results
        .iter()
        .filter_map(|(user_id_str, _)| user_id_str.parse::<i64>().ok())
        .collect();

    if user_ids.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<TopUser>::new()));
    }

    let mut username_map: std::collections::HashMap<i64, String> = std::collections::HashMap::new();

    // Batch fetch usernames using SeaORM
    for user_id in &user_ids {
        if let Ok(Some(user_model)) = user::Entity::find_by_id(*user_id).one(pool.get_ref()).await {
            username_map.insert(*user_id, user_model.username);
        }
    }

    let top_users: Vec<TopUser> = results
        .iter()
        .filter_map(|(user_id_str, score)| {
            let user_id = user_id_str.parse::<i64>().ok()?;
            let username = username_map.get(&user_id)?.clone();
            let total_likes = *score as i64;

            Some(TopUser {
                user_id,
                username,
                total_likes,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(top_users))
}

#[utoipa::path(
    get,
    path = "/api/top/feeds-commented",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 10)")
    ),
    responses(
        (status = 200, description = "Top feeds by comments", body = Vec<TopFeed>)
    ),
    tag = "top"
)]
pub async fn get_top_comments(
    redis_client: web::Data<RedisClient>,
    pool: web::Data<DbPool>,
    query: web::Query<TopQuery>,
) -> ActixResult<HttpResponse> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let start = ((page - 1) * limit) as i64;
    let stop = start + limit as i64 - 1;

    let mut conn = redis_client
        .get_async_connection()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let results: Vec<(String, f64)> = redis::cmd("ZREVRANGE")
        .arg("top:comments")
        .arg(start)
        .arg(stop)
        .arg("WITHSCORES")
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    log::info!("get_top_comments: Redis results: {:?}", results);

    if results.is_empty() {
        log::info!("get_top_comments: No results from Redis");
        return Ok(HttpResponse::Ok().json(Vec::<TopFeed>::new()));
    }

    let feed_ids: Vec<i64> = results
        .iter()
        .filter_map(|(feed_id_str, _)| feed_id_str.parse::<i64>().ok())
        .collect();

    log::info!("get_top_comments: Parsed feed_ids: {:?}", feed_ids);

    if feed_ids.is_empty() {
        log::warn!("get_top_comments: Failed to parse feed_ids from Redis results");
        return Ok(HttpResponse::Ok().json(Vec::<TopFeed>::new()));
    }

    let mut feed_map: std::collections::HashMap<i64, (i64, String, String)> =
        std::collections::HashMap::new();

    // Fetch feed info with user using SeaORM
    for feed_id in &feed_ids {
        log::debug!("get_top_comments: Looking up feed_id: {}", feed_id);
        match feed::Entity::find_by_id(*feed_id).one(pool.get_ref()).await {
            Ok(Some(feed_model)) => {
                log::debug!(
                    "get_top_comments: Found feed {} with user_id: {}",
                    feed_id,
                    feed_model.user_id
                );
                match user::Entity::find_by_id(feed_model.user_id)
                    .one(pool.get_ref())
                    .await
                {
                    Ok(Some(user_model)) => {
                        log::debug!(
                            "get_top_comments: Found user {} with username: {}",
                            feed_model.user_id,
                            user_model.username
                        );
                        feed_map.insert(
                            *feed_id,
                            (feed_model.user_id, user_model.username, feed_model.content),
                        );
                    }
                    Ok(None) => {
                        log::warn!(
                            "get_top_comments: User {} not found for feed {}",
                            feed_model.user_id,
                            feed_id
                        );
                    }
                    Err(e) => {
                        log::error!(
                            "get_top_comments: Error looking up user {}: {:?}",
                            feed_model.user_id,
                            e
                        );
                    }
                }
            }
            Ok(None) => {
                log::warn!("get_top_comments: Feed {} not found in database", feed_id);
            }
            Err(e) => {
                log::error!(
                    "get_top_comments: Error looking up feed {}: {:?}",
                    feed_id,
                    e
                );
            }
        }
    }

    log::info!("get_top_comments: Feed map size: {}", feed_map.len());

    // Build TopFeed responses
    let top_feeds: Vec<TopFeed> = results
        .iter()
        .filter_map(|(feed_id_str, score)| {
            let feed_id = feed_id_str.parse::<i64>().ok()?;
            let (user_id, username, content) = feed_map.get(&feed_id)?.clone();
            let count = *score as i64;

            Some(TopFeed {
                feed_id,
                user_id,
                username,
                content,
                count,
            })
        })
        .collect();

    log::info!(
        "get_top_comments: Returning {} top feeds (out of {} from Redis)",
        top_feeds.len(),
        results.len()
    );
    Ok(HttpResponse::Ok().json(top_feeds))
}

#[utoipa::path(
    get,
    path = "/api/top/feeds-viewed",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 10)")
    ),
    responses(
        (status = 200, description = "Top feeds viewed", body = Vec<TopFeed>)
    ),
    tag = "top"
)]
pub async fn get_top_feeds_viewed(
    redis_client: web::Data<RedisClient>,
    pool: web::Data<DbPool>,
    query: web::Query<TopQuery>,
) -> ActixResult<HttpResponse> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let start = ((page - 1) * limit) as i64;
    let stop = start + limit as i64 - 1;

    let mut conn = redis_client
        .get_async_connection()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Use ZREVRANGE with WITHSCORES to get feed_ids and scores
    // Now we only store feed_id as member, score is view count
    let results: Vec<(String, f64)> = redis::cmd("ZREVRANGE")
        .arg("top:feeds_viewed")
        .arg(start)
        .arg(stop)
        .arg("WITHSCORES")
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    if results.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<TopFeed>::new()));
    }

    let feed_ids: Vec<i64> = results
        .iter()
        .filter_map(|(feed_id_str, _)| feed_id_str.parse::<i64>().ok())
        .collect();

    if feed_ids.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<TopFeed>::new()));
    }

    let mut feed_map: std::collections::HashMap<i64, (i64, String, String)> =
        std::collections::HashMap::new();

    // Fetch feed info with user using SeaORM
    for feed_id in &feed_ids {
        if let Ok(Some(feed_model)) = feed::Entity::find_by_id(*feed_id).one(pool.get_ref()).await {
            if let Ok(Some(user_model)) = user::Entity::find_by_id(feed_model.user_id)
                .one(pool.get_ref())
                .await
            {
                feed_map.insert(
                    *feed_id,
                    (feed_model.user_id, user_model.username, feed_model.content),
                );
            }
        }
    }

    // Build TopFeed responses
    let top_feeds_viewed: Vec<TopFeed> = results
        .iter()
        .filter_map(|(feed_id_str, score)| {
            let feed_id = feed_id_str.parse::<i64>().ok()?;
            let (user_id, username, content) = feed_map.get(&feed_id)?.clone();
            let count = *score as i64;

            Some(TopFeed {
                feed_id,
                user_id,
                username,
                content,
                count,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(top_feeds_viewed))
}

#[utoipa::path(
    get,
    path = "/api/top/feeds-liked",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 10)")
    ),
    responses(
        (status = 200, description = "Top feeds liked", body = Vec<TopFeed>)
    ),
    tag = "top"
)]
pub async fn get_top_feeds_liked(
    redis_client: web::Data<RedisClient>,
    pool: web::Data<DbPool>,
    query: web::Query<TopQuery>,
) -> ActixResult<HttpResponse> {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let start = ((page - 1) * limit) as i64;
    let stop = start + limit as i64 - 1;

    let mut conn = redis_client
        .get_async_connection()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // Use ZREVRANGE with WITHSCORES to get feed_ids and scores
    // Now we only store feed_id as member, score is like count
    let results: Vec<(String, f64)> = redis::cmd("ZREVRANGE")
        .arg("top:feeds_liked")
        .arg(start)
        .arg(stop)
        .arg("WITHSCORES")
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    if results.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<TopFeed>::new()));
    }

    let feed_ids: Vec<i64> = results
        .iter()
        .filter_map(|(feed_id_str, _)| feed_id_str.parse::<i64>().ok())
        .collect();

    if feed_ids.is_empty() {
        return Ok(HttpResponse::Ok().json(Vec::<TopFeed>::new()));
    }

    let mut feed_map: std::collections::HashMap<i64, (i64, String, String)> =
        std::collections::HashMap::new();

    // Fetch feed info with user using SeaORM
    for feed_id in &feed_ids {
        if let Ok(Some(feed_model)) = feed::Entity::find_by_id(*feed_id).one(pool.get_ref()).await {
            if let Ok(Some(user_model)) = user::Entity::find_by_id(feed_model.user_id)
                .one(pool.get_ref())
                .await
            {
                feed_map.insert(
                    *feed_id,
                    (feed_model.user_id, user_model.username, feed_model.content),
                );
            }
        }
    }

    // Build TopFeed responses
    let top_feeds_liked: Vec<TopFeed> = results
        .iter()
        .filter_map(|(feed_id_str, score)| {
            let feed_id = feed_id_str.parse::<i64>().ok()?;
            let (user_id, username, content) = feed_map.get(&feed_id)?.clone();
            let count = *score as i64;

            Some(TopFeed {
                feed_id,
                user_id,
                username,
                content,
                count,
            })
        })
        .collect();

    Ok(HttpResponse::Ok().json(top_feeds_liked))
}
