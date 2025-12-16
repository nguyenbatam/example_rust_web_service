use crate::auth::verify_token;
use crate::config::Config;
use actix_web::{web, Error, FromRequest, HttpRequest};
use std::future::{ready, Ready};

pub struct AuthenticatedUser {
    pub user_id: i64,
    #[allow(dead_code)]
    pub email: String,
}

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let auth_header = req.headers().get("Authorization");

        if let Some(header_value) = auth_header {
            if let Ok(header_str) = header_value.to_str() {
                if let Some(token) = header_str.strip_prefix("Bearer ") {
                    let config = req.app_data::<web::Data<Config>>();
                    if let Some(config) = config {
                        match verify_token(token, &config.jwt.secret) {
                            Ok(claims) => {
                                if let Ok(user_id) = claims.sub.parse::<i64>() {
                                    return ready(Ok(AuthenticatedUser {
                                        user_id,
                                        email: claims.email,
                                    }));
                                }
                            }
                            Err(_) => {
                                return ready(Err(actix_web::error::ErrorUnauthorized(
                                    "Invalid token",
                                )));
                            }
                        }
                    }
                }
            }
        }

        ready(Err(actix_web::error::ErrorUnauthorized(
            "Missing or invalid authorization header",
        )))
    }
}
