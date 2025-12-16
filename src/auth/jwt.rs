use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user id
    pub email: String,
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    pub fn new(user_id: i64, email: String, expiration_hours: i64) -> Self {
        let now = Utc::now();
        Claims {
            sub: user_id.to_string(),
            email,
            exp: (now + Duration::hours(expiration_hours)).timestamp(),
            iat: now.timestamp(),
        }
    }
}

pub fn create_token(claims: &Claims, secret: &str) -> Result<String, anyhow::Error> {
    let token = encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )?;
    Ok(token)
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, anyhow::Error> {
    let validation = Validation::default();
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )?;
    Ok(token_data.claims)
}
