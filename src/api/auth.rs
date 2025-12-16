use crate::auth::{create_token, hash_password, verify_password, Claims};
use crate::config::Config;
use crate::db::DbPool;
use crate::entities::user;
use crate::kafka::{KafkaProducer, UserCreatedEvent};
use crate::models::{AuthResponse, LoginRequest, SignupRequest, UserResponse};
use actix_web::{web, HttpResponse, Result as ActixResult};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use serde_json::json;

#[utoipa::path(
    post,
    path = "/api/auth/signup",
    request_body = SignupRequest,
    responses(
        (status = 200, description = "User created successfully", body = AuthResponse),
        (status = 400, description = "Bad request"),
        (status = 409, description = "User already exists")
    ),
    tag = "auth"
)]
pub async fn signup(
    req: web::Json<SignupRequest>,
    pool: web::Data<DbPool>,
    config: web::Data<Config>,
    kafka_producer: web::Data<KafkaProducer>,
) -> ActixResult<HttpResponse> {
    // Check if user exists using SeaORM
    let existing_user = user::Entity::find()
        .filter(
            Condition::any()
                .add(user::Column::Email.eq(&req.email))
                .add(user::Column::Username.eq(&req.username)),
        )
        .one(pool.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    if existing_user.is_some() {
        return Ok(HttpResponse::Conflict().json(json!({
            "error": "User with this email or username already exists"
        })));
    }

    let password_hash =
        hash_password(&req.password).map_err(actix_web::error::ErrorInternalServerError)?;

    // Create user using SeaORM
    let new_user = user::ActiveModel {
        email: sea_orm::Set(req.email.clone()),
        username: sea_orm::Set(req.username.clone()),
        password_hash: sea_orm::Set(password_hash),
        ..Default::default()
    };

    let user = user::Entity::insert(new_user)
        .exec_with_returning(pool.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let claims = Claims::new(user.id, user.email.clone(), config.jwt.expiration_hours);
    let token = create_token(&claims, &config.jwt.secret)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let event = UserCreatedEvent::new(user.id as u64, user.email.clone(), user.username.clone());
    if let Ok(event_json) = serde_json::to_string(&event) {
        if let Err(e) = kafka_producer
            .send_message("user_events", &user.id.to_string(), &event_json)
            .await
        {
            log::warn!("Failed to send Kafka event: {:?}", e);
        }
    }

    Ok(HttpResponse::Created().json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            email: user.email,
            username: user.username,
        },
    }))
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 404, description = "User not found")
    ),
    tag = "auth"
)]
pub async fn login(
    req: web::Json<LoginRequest>,
    pool: web::Data<DbPool>,
    config: web::Data<Config>,
) -> ActixResult<HttpResponse> {
    // Find user by email using SeaORM
    let user = user::Entity::find()
        .filter(user::Column::Email.eq(&req.email))
        .one(pool.get_ref())
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    let user = match user {
        Some(u) => u,
        None => {
            return Ok(HttpResponse::NotFound().json(json!({
                "error": "User not found"
            })));
        }
    };

    let is_valid = verify_password(&req.password, &user.password_hash)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    if !is_valid {
        return Ok(HttpResponse::Unauthorized().json(json!({
            "error": "Invalid credentials"
        })));
    }

    let claims = Claims::new(user.id, user.email.clone(), config.jwt.expiration_hours);
    let token = create_token(&claims, &config.jwt.secret)
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            email: user.email,
            username: user.username,
        },
    }))
}
