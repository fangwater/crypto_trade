use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct HealthMetrics {
    pub connection_id: Uuid,
    pub exchange: String,
    pub market_type: String,
    pub health_score: f64,
    pub rtt_ms: f64,
    pub success_rate: f64,
    pub total_messages: u64,
    pub total_errors: u64,
    pub last_update: Instant,
    pub consecutive_failures: u32,
}

impl HealthMetrics {
    pub fn new(connection_id: Uuid, exchange: String, market_type: String) -> Self {
        Self {
            connection_id,
            exchange,
            market_type,
            health_score: 100.0,
            rtt_ms: 0.0,
            success_rate: 100.0,
            total_messages: 0,
            total_errors: 0,
            last_update: Instant::now(),
            consecutive_failures: 0,
        }
    }

    pub fn update_success(&mut self, rtt_ms: f64) {
        self.total_messages += 1;
        self.consecutive_failures = 0;
        self.rtt_ms = (self.rtt_ms * 0.9) + (rtt_ms * 0.1); // Exponential moving average
        self.last_update = Instant::now();
        self.recalculate_health_score();
    }

    pub fn update_failure(&mut self) {
        self.total_errors += 1;
        self.consecutive_failures += 1;
        self.last_update = Instant::now();
        self.recalculate_health_score();
    }

    fn recalculate_health_score(&mut self) {
        let total = self.total_messages + self.total_errors;
        if total > 0 {
            self.success_rate = (self.total_messages as f64 / total as f64) * 100.0;
        }

        let mut score = 0.0;

        // Success rate (40%)
        score += self.success_rate * 0.4;

        // RTT score (30%)
        let rtt_score = if self.rtt_ms < 10.0 {
            30.0
        } else if self.rtt_ms < 50.0 {
            25.0
        } else if self.rtt_ms < 100.0 {
            20.0
        } else if self.rtt_ms < 200.0 {
            15.0
        } else if self.rtt_ms < 500.0 {
            10.0
        } else {
            5.0
        };
        score += rtt_score;

        // Consecutive failures penalty (20%)
        let failure_penalty = match self.consecutive_failures {
            0 => 20.0,
            1 => 15.0,
            2 => 10.0,
            3 => 5.0,
            _ => 0.0,
        };
        score += failure_penalty;

        // Recency bonus (10%)
        let elapsed = self.last_update.elapsed().as_secs();
        let recency_bonus = if elapsed < 10 {
            10.0
        } else if elapsed < 30 {
            8.0
        } else if elapsed < 60 {
            6.0
        } else if elapsed < 300 {
            4.0
        } else {
            2.0
        };
        score += recency_bonus;

        self.health_score = score.min(100.0).max(0.0);
    }

    pub fn is_healthy(&self) -> bool {
        self.health_score >= 50.0 && self.consecutive_failures < 5
    }
}

pub struct HealthTracker {
    metrics: Arc<DashMap<Uuid, HealthMetrics>>,
}

impl HealthTracker {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(DashMap::new()),
        }
    }

    pub fn register_connection(&self, connection_id: Uuid, exchange: String, market_type: String) {
        let metrics = HealthMetrics::new(connection_id, exchange, market_type);
        self.metrics.insert(connection_id, metrics);
    }

    pub fn update_success(&self, connection_id: Uuid, rtt_ms: f64) {
        if let Some(mut entry) = self.metrics.get_mut(&connection_id) {
            entry.update_success(rtt_ms);
        }
    }

    pub fn update_failure(&self, connection_id: Uuid) {
        if let Some(mut entry) = self.metrics.get_mut(&connection_id) {
            entry.update_failure();
        }
    }

    pub fn get_metrics(&self, connection_id: Uuid) -> Option<HealthMetrics> {
        self.metrics.get(&connection_id).map(|entry| entry.clone())
    }

    pub fn get_healthy_connections(
        &self,
        exchange: &str,
        market_type: &str,
        min_score: f64,
    ) -> Vec<HealthMetrics> {
        let mut connections = Vec::new();

        for entry in self.metrics.iter() {
            let metrics = entry.value();
            if metrics.exchange == exchange
                && metrics.market_type == market_type
                && metrics.health_score >= min_score
                && metrics.is_healthy()
            {
                connections.push(metrics.clone());
            }
        }

        connections.sort_by(|a, b| b.health_score.partial_cmp(&a.health_score).unwrap());
        connections
    }

    pub fn get_top_k_connections(
        &self,
        exchange: &str,
        market_type: &str,
        k: usize,
    ) -> Vec<HealthMetrics> {
        let mut connections = self.get_healthy_connections(exchange, market_type, 0.0);
        connections.truncate(k);
        connections
    }

    pub fn remove_connection(&self, connection_id: Uuid) {
        self.metrics.remove(&connection_id);
    }

    pub fn cleanup_stale_connections(&self, max_age: Duration) {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for entry in self.metrics.iter() {
            if now.duration_since(entry.value().last_update) > max_age {
                to_remove.push(*entry.key());
            }
        }

        for id in to_remove {
            self.metrics.remove(&id);
        }
    }
}