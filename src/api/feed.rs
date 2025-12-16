use crate::auth::AuthenticatedUser;
use crate::config::Config;
use crate::db::DbPool;
use crate::entities::{feed, feed_like};
use crate::kafka::{
    FeedCommentedEvent, FeedCreatedEvent, FeedLikedEvent, FeedViewedEvent, KafkaProducer,
};
use crate::models::{
    Comment, CommentRequest, CommentResponse, CreateFeedRequest, FeedResponse, FeedView,
};
use actix_web::{web, HttpResponse, Result as ActixResult};
use chrono::Utc;
use mongodb::Database as MongoDatabase;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct FeedQuery {
    #[schema(example = 1)]
    pub page: Option<u64>,
    #[schema(example = 20)]
    pub limit: Option<u64>,
}

#[utoipa::path(
    post,
    path = "/api/feed",
    request_body = CreateFeedRequest,
    responses(
        (status = 200, description = "Feed created successfully", body = FeedResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "feed"
)]
pub async fn create_feed(
    req: web::Json<CreateFeedRequest>,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    _config: web::Data<Config>,
    kafka_producer: web::Data<KafkaProducer>,
) -> ActixResult<HttpResponse> {
    let user_id = user.user_id;

    // Create feed using SeaORM
    let new_feed = feed::ActiveModel {
        user_id: sea_orm::Set(user_id),
        content: sea_orm::Set(req.content.clone()),
        ..Default::default()
    };

    let feed = feed::Entity::insert(new_feed)
        .exec_with_returning(pool.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let event = FeedCreatedEvent::new(feed.id as u64, user_id, req.content.clone());
    if let Ok(event_json) = serde_json::to_string(&event) {
        if let Err(e) = kafka_producer
            .send_message("feed_events", &feed.id.to_string(), &event_json)
            .await
        {
            log::warn!("Failed to send Kafka event: {:?}", e);
        }
    }

    Ok(HttpResponse::Ok().json(FeedResponse {
        id: feed.id,
        user_id,
        content: req.content.clone(),
        like_count: 0,
        comment_count: 0,
        is_liked: false,
        created_at: feed.created_at,
    }))
}

#[utoipa::path(
    get,
    path = "/api/feed",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 20)")
    ),
    responses(
        (status = 200, description = "List of feeds", body = Vec<FeedResponse>)
    ),
    tag = "feed"
)]
pub async fn get_feeds(
    user: Option<AuthenticatedUser>,
    pool: web::Data<DbPool>,
    mongo_db: web::Data<MongoDatabase>,
    query: web::Query<FeedQuery>,
) -> ActixResult<HttpResponse> {
    let user_id = user.map(|u| u.user_id);

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    // Get feeds using SeaORM
    let feeds = feed::Entity::find()
        .order_by_desc(feed::Column::CreatedAt)
        .limit(limit)
        .offset(offset)
        .all(pool.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut feed_responses = Vec::new();
    for feed in feeds {
        let feed_id = feed.id;

        // Count likes using SeaORM
        let like_count = feed_like::Entity::find()
            .filter(feed_like::Column::FeedId.eq(feed_id))
            .all(pool.get_ref())
            .await
            .unwrap_or_default()
            .len() as i64;

        let comment_count = {
            let collection = mongo_db.collection::<Comment>("comments");
            let filter = mongodb::bson::doc! {"feed_id": feed_id};
            collection.count_documents(filter, None).await.unwrap_or(0) as i64
        };

        let is_liked = if let Some(uid) = user_id {
            feed_like::Entity::find()
                .filter(
                    Condition::all()
                        .add(feed_like::Column::FeedId.eq(feed_id))
                        .add(feed_like::Column::UserId.eq(uid)),
                )
                .one(pool.get_ref())
                .await
                .unwrap_or(None)
                .is_some()
        } else {
            false
        };

        feed_responses.push(FeedResponse {
            id: feed_id,
            user_id: feed.user_id,
            content: feed.content,
            like_count,
            comment_count,
            is_liked,
            created_at: feed.created_at,
        });
    }

    Ok(HttpResponse::Ok().json(feed_responses))
}

#[utoipa::path(
    post,
    path = "/api/feed/{feed_id}/like",
    responses(
        (status = 200, description = "Feed liked successfully"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "feed"
)]
pub async fn like_feed(
    path: web::Path<i64>,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
    kafka_producer: web::Data<KafkaProducer>,
) -> ActixResult<HttpResponse> {
    let user_id = user.user_id;
    let feed_id = path.into_inner();

    // Check if already liked
    let existing = feed_like::Entity::find()
        .filter(
            Condition::all()
                .add(feed_like::Column::FeedId.eq(feed_id))
                .add(feed_like::Column::UserId.eq(user_id)),
        )
        .one(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error checking existing like: {:?}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    if existing.is_some() {
        return Ok(HttpResponse::Ok().json(json!({"message": "Already liked"})));
    }

    // Verify feed exists
    let feed_exists = feed::Entity::find_by_id(feed_id)
        .one(pool.get_ref())
        .await
        .map_err(|e| {
            log::error!("Database error checking feed existence: {:?}", e);
            actix_web::error::ErrorInternalServerError("Database error")
        })?;

    if feed_exists.is_none() {
        return Ok(HttpResponse::NotFound().json(json!({
            "error": "Feed not found"
        })));
    }

    // Create like using SeaORM
    let new_like = feed_like::ActiveModel {
        feed_id: sea_orm::Set(feed_id),
        user_id: sea_orm::Set(user_id),
        ..Default::default()
    };

    match feed_like::Entity::insert(new_like)
        .exec(pool.get_ref())
        .await
    {
        Ok(_) => {
            let event = FeedLikedEvent::new(feed_id, user_id);
            if let Ok(event_json) = serde_json::to_string(&event) {
                if let Err(e) = kafka_producer
                    .send_message("feed_events", &feed_id.to_string(), &event_json)
                    .await
                {
                    log::warn!("Failed to send Kafka event: {:?}", e);
                }
            }

            Ok(HttpResponse::Ok().json(json!({"message": "Feed liked"})))
        }
        Err(e) => {
            // Check if it's a unique constraint violation (race condition)
            let error_msg =
                if e.to_string().contains("unique") || e.to_string().contains("Duplicate") {
                    "Feed already liked"
                } else {
                    log::error!("Database error inserting like: {:?}", e);
                    "Failed to like feed"
                };
            Ok(HttpResponse::BadRequest().json(json!({
                "error": error_msg
            })))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/feed/{feed_id}/like",
    responses(
        (status = 200, description = "Feed unliked successfully"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "feed"
)]
pub async fn unlike_feed(
    path: web::Path<i64>,
    user: AuthenticatedUser,
    pool: web::Data<DbPool>,
) -> ActixResult<HttpResponse> {
    let user_id = user.user_id;
    let feed_id = path.into_inner();

    // Delete like using SeaORM
    let result = feed_like::Entity::delete_many()
        .filter(
            Condition::all()
                .add(feed_like::Column::FeedId.eq(feed_id))
                .add(feed_like::Column::UserId.eq(user_id)),
        )
        .exec(pool.get_ref())
        .await;

    match result {
        Ok(_) => Ok(HttpResponse::Ok().json(json!({"message": "Feed unliked"}))),
        Err(e) => {
            log::error!("Database error: {:?}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "error": "Failed to unlike feed"
            })))
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/feed/{feed_id}/comment",
    request_body = CommentRequest,
    responses(
        (status = 200, description = "Comment created successfully", body = CommentResponse),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "feed"
)]
pub async fn comment_feed(
    path: web::Path<i64>,
    req: web::Json<CommentRequest>,
    user: AuthenticatedUser,
    mongo_db: web::Data<MongoDatabase>,
    kafka_producer: web::Data<KafkaProducer>,
) -> ActixResult<HttpResponse> {
    let user_id = user.user_id;
    let feed_id = path.into_inner();

    let comment_id = Uuid::new_v4().to_string();
    let comment = Comment {
        id: Some(comment_id.clone()),
        feed_id,
        user_id,
        content: req.content.clone(),
        created_at: Utc::now(),
    };

    let collection = mongo_db.collection::<Comment>("comments");
    collection
        .insert_one(&comment, None)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let event = FeedCommentedEvent::new(feed_id, user_id, comment_id.clone(), req.content.clone());
    if let Ok(event_json) = serde_json::to_string(&event) {
        if let Err(e) = kafka_producer
            .send_message("feed_events", &feed_id.to_string(), &event_json)
            .await
        {
            log::warn!("Failed to send Kafka event: {:?}", e);
        }
    }

    Ok(HttpResponse::Ok().json(CommentResponse {
        id: comment_id,
        feed_id: comment.feed_id,
        user_id: comment.user_id,
        content: comment.content,
        created_at: comment.created_at,
    }))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CommentQuery {
    #[schema(example = 1)]
    pub page: Option<u64>,
    #[schema(example = 20)]
    pub limit: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/feed/{feed_id}/comments",
    params(
        ("feed_id" = i64, Path, description = "Feed ID"),
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 20)")
    ),
    responses(
        (status = 200, description = "List of comments", body = Vec<CommentResponse>)
    ),
    tag = "feed"
)]
pub async fn get_comments(
    path: web::Path<i64>,
    query: web::Query<CommentQuery>,
    mongo_db: web::Data<MongoDatabase>,
) -> ActixResult<HttpResponse> {
    let feed_id = path.into_inner();
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20) as i64;
    let skip = ((page - 1) * limit as u64) as i64;

    let collection = mongo_db.collection::<Comment>("comments");
    let filter = mongodb::bson::doc! {"feed_id": feed_id};
    let options = mongodb::options::FindOptions::builder()
        .sort(mongodb::bson::doc! {"created_at": -1})
        .limit(limit)
        .skip(skip as u64)
        .build();
    let mut cursor = collection
        .find(filter, options)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut comments = Vec::new();
    while let Ok(true) = cursor.advance().await {
        let comment: Comment = cursor
            .deserialize_current()
            .map_err(actix_web::error::ErrorInternalServerError)?;

        let comment_id = comment
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        comments.push(CommentResponse {
            id: comment_id,
            feed_id: comment.feed_id,
            user_id: comment.user_id,
            content: comment.content,
            created_at: comment.created_at,
        });
    }

    Ok(HttpResponse::Ok().json(comments))
}

#[utoipa::path(
    post,
    path = "/api/feed/{feed_id}/view",
    responses(
        (status = 200, description = "Feed view recorded")
    ),
    tag = "feed"
)]
pub async fn view_feed(
    path: web::Path<i64>,
    user: Option<AuthenticatedUser>,
    mongo_db: web::Data<MongoDatabase>,
    kafka_producer: web::Data<KafkaProducer>,
) -> ActixResult<HttpResponse> {
    let user_id = user.map(|u| u.user_id).unwrap_or(0);
    let feed_id = path.into_inner();

    let feed_view = FeedView {
        id: Some(Uuid::new_v4().to_string()),
        feed_id,
        user_id,
        viewed_at: Utc::now(),
    };

    let collection = mongo_db.collection::<FeedView>("feed_views");
    collection
        .insert_one(&feed_view, None)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let event = FeedViewedEvent::new(feed_id, user_id);
    if let Ok(event_json) = serde_json::to_string(&event) {
        if let Err(e) = kafka_producer
            .send_message("feed_events", &feed_id.to_string(), &event_json)
            .await
        {
            log::warn!("Failed to send Kafka event: {:?}", e);
        }
    }

    Ok(HttpResponse::Ok().json(json!({"message": "View recorded"})))
}
