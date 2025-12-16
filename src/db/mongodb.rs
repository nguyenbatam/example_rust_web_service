use crate::config::Config;
use mongodb::{Client, Database};

pub async fn create_mongodb_client(config: &Config) -> Result<Database, anyhow::Error> {
    let client = Client::with_uri_str(&config.mongodb.uri).await?;
    let db = client.database(&config.mongodb.database);
    Ok(db)
}
