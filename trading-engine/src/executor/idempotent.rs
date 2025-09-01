use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct IdempotentManager {
    prefix: String,
    used_ids: Arc<DashMap<String, i64>>,
}

impl IdempotentManager {
    pub fn new(prefix: String) -> Self {
        Self {
            prefix,
            used_ids: Arc::new(DashMap::new()),
        }
    }

    pub fn generate_client_order_id(&self, command_id: Uuid) -> String {
        let timestamp = Utc::now().timestamp_millis();
        let id = format!("{}_{}_{}", self.prefix, command_id, timestamp);
        self.used_ids.insert(id.clone(), timestamp);
        id
    }

    pub fn is_duplicate(&self, client_order_id: &str) -> bool {
        self.used_ids.contains_key(client_order_id)
    }

    pub fn cleanup_old_ids(&self, max_age_ms: i64) {
        let now = Utc::now().timestamp_millis();
        let mut to_remove = Vec::new();

        for entry in self.used_ids.iter() {
            if now - *entry.value() > max_age_ms {
                to_remove.push(entry.key().clone());
            }
        }

        for key in to_remove {
            self.used_ids.remove(&key);
        }
    }
}