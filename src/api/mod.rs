pub mod auth;
pub mod feed;
pub mod notify;
pub mod top;

use crate::models::{
    AuthResponse, Comment, CommentRequest, CommentResponse, CreateFeedRequest, FeedResponse,
    FeedView, LoginRequest, Notification, NotificationResponse, NotificationType, SignupRequest,
    TopFeed, TopUser, UserResponse,
};
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        // Auth endpoints
        auth::signup,
        auth::login,
        // Feed endpoints
        feed::create_feed,
        feed::get_feeds,
        feed::like_feed,
        feed::unlike_feed,
        feed::comment_feed,
        feed::get_comments,
        feed::view_feed,
        // Notification endpoints
        notify::get_notifications,
        notify::mark_notification_read,
        // Top stats endpoints
        top::get_top_users_liked,
        top::get_top_comments,
        top::get_top_feeds_viewed,
        top::get_top_feeds_liked,
    ),
    components(schemas(
        // Auth schemas
        SignupRequest,
        LoginRequest,
        AuthResponse,
        UserResponse,
        // Feed schemas
        CreateFeedRequest,
        FeedResponse,
        CommentRequest,
        CommentResponse,
        Comment,
        FeedView,
        // Notification schemas
        Notification,
        NotificationResponse,
        NotificationType,
        // Top stats schemas
        TopUser,
        TopFeed,
        top::TopQuery,
        // Query schemas
        feed::FeedQuery,
        feed::CommentQuery,
        notify::NotificationQuery,
    )),
    tags(
        (name = "auth", description = "Authentication endpoints"),
        (name = "feed", description = "Feed management endpoints"),
        (name = "notify", description = "Notification endpoints"),
        (name = "top", description = "Top statistics endpoints"),
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

use utoipa::Modify;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            )
        }
    }
}
