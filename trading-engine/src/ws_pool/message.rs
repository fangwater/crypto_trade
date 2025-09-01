use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct WsMessage {
    pub id: Uuid,
    pub exchange: String,
    pub market_type: String,
    pub connection_id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub data: Bytes,
}

impl WsMessage {
    pub fn new(
        exchange: String,
        market_type: String,
        connection_id: Uuid,
        data: Bytes,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            exchange,
            market_type,
            connection_id,
            timestamp: chrono::Utc::now(),
            data,
        }
    }
}