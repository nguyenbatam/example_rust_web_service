use crate::config::Config;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};

pub type DbPool = DatabaseConnection;

pub async fn create_mysql_pool(config: &Config) -> Result<DbPool, anyhow::Error> {
    let url = config.mysql_url();
    let db = Database::connect(&url).await?;

    // Create tables if not exists using SeaORM migrations or raw SQL
    // For now, we'll use raw SQL for schema creation
    // In production, use SeaORM migrations: sea-orm-migration
    let sql = r#"
        CREATE TABLE IF NOT EXISTS users (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            email VARCHAR(255) UNIQUE NOT NULL,
            username VARCHAR(255) UNIQUE NOT NULL,
            password_hash VARCHAR(255) NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
        );
        
        CREATE TABLE IF NOT EXISTS feeds (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            user_id BIGINT NOT NULL,
            content TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            INDEX idx_user_id (user_id),
            INDEX idx_created_at (created_at)
        );
        
        CREATE TABLE IF NOT EXISTS feed_likes (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            feed_id BIGINT NOT NULL,
            user_id BIGINT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE KEY unique_feed_user (feed_id, user_id),
            FOREIGN KEY (feed_id) REFERENCES feeds(id) ON DELETE CASCADE,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
            INDEX idx_feed_id (feed_id),
            INDEX idx_user_id (user_id)
        );
    "#;

    // Execute schema creation
    for statement in sql.split(';') {
        let statement = statement.trim();
        if !statement.is_empty() {
            let stmt = sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::MySql,
                statement.to_string(),
            );
            db.execute(stmt).await?;
        }
    }

    Ok(db)
}
