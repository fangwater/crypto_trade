use super::health_tracker::{HealthMetrics, HealthTracker};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum SelectionStrategy {
    RoundRobin,
    HealthScore,
    LeastLatency,
    Random,
}

pub struct ConnectionSelector {
    health_tracker: Arc<HealthTracker>,
    strategy: SelectionStrategy,
    round_robin_indices: Arc<RwLock<HashMap<String, usize>>>,
}

impl ConnectionSelector {
    pub fn new(health_tracker: Arc<HealthTracker>, strategy: SelectionStrategy) -> Self {
        Self {
            health_tracker,
            strategy,
            round_robin_indices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn select_connections(
        &self,
        exchange: &str,
        market_type: &str,
        count: usize,
    ) -> Vec<Uuid> {
        match self.strategy {
            SelectionStrategy::RoundRobin => self.select_round_robin(exchange, market_type, count),
            SelectionStrategy::HealthScore => self.select_by_health(exchange, market_type, count),
            SelectionStrategy::LeastLatency => self.select_by_latency(exchange, market_type, count),
            SelectionStrategy::Random => self.select_random(exchange, market_type, count),
        }
    }

    fn select_round_robin(&self, exchange: &str, market_type: &str, count: usize) -> Vec<Uuid> {
        let connections = self.health_tracker.get_healthy_connections(exchange, market_type, 30.0);
        
        if connections.is_empty() {
            return Vec::new();
        }

        let key = format!("{}_{}", exchange, market_type);
        let mut indices = self.round_robin_indices.write();
        let index = indices.entry(key.clone()).or_insert(0);

        let mut selected = Vec::new();
        for _ in 0..count.min(connections.len()) {
            selected.push(connections[*index % connections.len()].connection_id);
            *index = (*index + 1) % connections.len();
        }

        selected
    }

    fn select_by_health(&self, exchange: &str, market_type: &str, count: usize) -> Vec<Uuid> {
        let connections = self.health_tracker.get_top_k_connections(exchange, market_type, count);
        connections.into_iter().map(|m| m.connection_id).collect()
    }

    fn select_by_latency(&self, exchange: &str, market_type: &str, count: usize) -> Vec<Uuid> {
        let mut connections = self.health_tracker.get_healthy_connections(exchange, market_type, 30.0);
        connections.sort_by(|a, b| a.rtt_ms.partial_cmp(&b.rtt_ms).unwrap());
        connections
            .into_iter()
            .take(count)
            .map(|m| m.connection_id)
            .collect()
    }

    fn select_random(&self, exchange: &str, market_type: &str, count: usize) -> Vec<Uuid> {
        use rand::seq::SliceRandom;
        
        let connections = self.health_tracker.get_healthy_connections(exchange, market_type, 30.0);
        let mut rng = rand::thread_rng();
        let mut connection_ids: Vec<Uuid> = connections.into_iter().map(|m| m.connection_id).collect();
        connection_ids.shuffle(&mut rng);
        connection_ids.into_iter().take(count).collect()
    }

    pub fn select_best_connection(&self, exchange: &str, market_type: &str) -> Option<Uuid> {
        self.select_connections(exchange, market_type, 1).first().copied()
    }

    pub fn get_backup_connections(
        &self,
        exchange: &str,
        market_type: &str,
        primary_id: Uuid,
        count: usize,
    ) -> Vec<Uuid> {
        let connections = self.health_tracker.get_healthy_connections(exchange, market_type, 30.0);
        connections
            .into_iter()
            .filter(|m| m.connection_id != primary_id)
            .take(count)
            .map(|m| m.connection_id)
            .collect()
    }
}