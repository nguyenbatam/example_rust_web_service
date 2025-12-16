use crate::auth::AuthenticatedUser;
use crate::models::{Notification, NotificationResponse};
use actix_web::{web, HttpResponse, Result as ActixResult};
use mongodb::Database as MongoDatabase;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub struct NotificationQuery {
    #[schema(example = 1)]
    pub page: Option<u64>,
    #[schema(example = 50)]
    pub limit: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/api/notify",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 50)")
    ),
    responses(
        (status = 200, description = "List of notifications", body = Vec<NotificationResponse>),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "notify"
)]
pub async fn get_notifications(
    user: AuthenticatedUser,
    mongo_db: web::Data<MongoDatabase>,
    query: web::Query<NotificationQuery>,
) -> ActixResult<HttpResponse> {
    let user_id = user.user_id;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(50) as i64;
    let skip = ((page - 1) * limit as u64) as i64;

    let collection = mongo_db.collection::<Notification>("notifications");
    let filter = mongodb::bson::doc! {
        "user_id": user_id
    };
    let options = mongodb::options::FindOptions::builder()
        .sort(mongodb::bson::doc! {"created_at": -1})
        .limit(limit)
        .skip(skip as u64)
        .build();

    let mut cursor = collection
        .find(filter, options)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let mut notifications = Vec::new();
    while let Ok(true) = cursor.advance().await {
        let notif = cursor
            .deserialize_current()
            .map_err(actix_web::error::ErrorInternalServerError)?;
        notifications.push(NotificationResponse {
            id: notif.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            from_user_id: notif.from_user_id,
            from_username: notif.from_username,
            feed_id: notif.feed_id,
            notification_type: notif.notification_type,
            content: notif.content,
            created_at: notif.created_at,
            is_read: notif.is_read,
        });
    }

    Ok(HttpResponse::Ok().json(notifications))
}

#[utoipa::path(
    put,
    path = "/api/notify/{notification_id}/read",
    responses(
        (status = 200, description = "Notification marked as read"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "notify"
)]
pub async fn mark_notification_read(
    path: web::Path<String>,
    user: AuthenticatedUser,
    mongo_db: web::Data<MongoDatabase>,
) -> ActixResult<HttpResponse> {
    let user_id = user.user_id;
    let notification_id = path.into_inner();

    let collection = mongo_db.collection::<Notification>("notifications");
    let filter = mongodb::bson::doc! {
        "_id": &notification_id,
        "user_id": user_id
    };
    let update = mongodb::bson::doc! {
        "$set": {"is_read": true}
    };

    collection
        .update_one(filter, update, None)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"message": "Notification marked as read"})))
}
