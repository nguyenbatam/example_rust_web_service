use crate::config::Config;
use redis::Client as RedisClient;

pub fn create_redis_client(config: &Config) -> Result<RedisClient, anyhow::Error> {
    let url = config.redis_url();
    let client = RedisClient::open(url)?;
    Ok(client)
}
