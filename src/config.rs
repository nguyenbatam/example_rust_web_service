use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub jwt: JwtConfig,
    pub mysql: MysqlConfig,
    pub mongodb: MongodbConfig,
    pub redis: RedisConfig,
    pub kafka: KafkaConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MysqlConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MongodbConfig {
    pub uri: String,
    pub database: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaConfig {
    pub brokers: String,
    pub group_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self, anyhow::Error> {
        dotenv::dotenv().ok();

        Ok(Config {
            server: ServerConfig {
                host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("SERVER_PORT")
                    .unwrap_or_else(|_| "8080".to_string())
                    .parse()
                    .unwrap_or(8080),
            },
            jwt: JwtConfig {
                secret: env::var("JWT_SECRET")
                    .unwrap_or_else(|_| "your-secret-key-change-this".to_string()),
                expiration_hours: env::var("JWT_EXPIRATION_HOURS")
                    .unwrap_or_else(|_| "24".to_string())
                    .parse()
                    .unwrap_or(24),
            },
            mysql: MysqlConfig {
                host: env::var("MYSQL_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: env::var("MYSQL_PORT")
                    .unwrap_or_else(|_| "3306".to_string())
                    .parse()
                    .unwrap_or(3306),
                user: env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
                password: env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "password".to_string()),
                database: env::var("MYSQL_DATABASE").unwrap_or_else(|_| "example_db".to_string()),
            },
            mongodb: MongodbConfig {
                uri: env::var("MONGODB_URI")
                    .unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
                database: env::var("MONGODB_DATABASE").unwrap_or_else(|_| "example_db".to_string()),
            },
            redis: RedisConfig {
                host: env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: env::var("REDIS_PORT")
                    .unwrap_or_else(|_| "6379".to_string())
                    .parse()
                    .unwrap_or(6379),
                password: env::var("REDIS_PASSWORD").ok(),
            },
            kafka: KafkaConfig {
                brokers: env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()),
                group_id: env::var("KAFKA_GROUP_ID")
                    .unwrap_or_else(|_| "example_rust_service".to_string()),
            },
        })
    }

    pub fn mysql_url(&self) -> String {
        format!(
            "mysql://{}:{}@{}:{}/{}",
            self.mysql.user,
            self.mysql.password,
            self.mysql.host,
            self.mysql.port,
            self.mysql.database
        )
    }

    pub fn redis_url(&self) -> String {
        if let Some(password) = &self.redis.password {
            format!(
                "redis://:{}@{}:{}",
                password, self.redis.host, self.redis.port
            )
        } else {
            format!("redis://{}:{}", self.redis.host, self.redis.port)
        }
    }
}
