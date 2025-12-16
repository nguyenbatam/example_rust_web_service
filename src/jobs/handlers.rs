use log::{error, info};
use serde_json::Value;

pub fn handle_user_created_event(topic: String, key: String, payload: Vec<u8>) {
    info!(
        "Handling user_created event from topic: {}, key: {}",
        topic, key
    );

    match std::str::from_utf8(&payload) {
        Ok(payload_str) => {
            match serde_json::from_str::<Value>(payload_str) {
                Ok(data) => {
                    info!("User created event data: {:?}", data);
                    // Process the event here
                    // Example: send welcome email, create user profile, etc.
                }
                Err(e) => {
                    error!("Failed to parse event payload: {:?}", e);
                }
            }
        }
        Err(e) => {
            error!("Failed to decode event payload: {:?}", e);
        }
    }
}
