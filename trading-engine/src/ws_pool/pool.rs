use super::connection::{BaseConnection, ConnectionCommand, ConnectionState};
use super::{BinanceConnection, OkexConnection, BybitConnection, WsConnectionRunner};
use super::message::WsMessage;
use crate::config::{ExchangeConfig, TradingEngineConfig, WsPoolConfig};
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::time::{self, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use serde_json::Value;

pub struct WsPool {
    config: WsPoolConfig,
    connections: Arc<DashMap<Uuid, BaseConnection>>,
    message_tx: mpsc::UnboundedSender<Bytes>,
    message_rx: Option<mpsc::UnboundedReceiver<Bytes>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    exchanges: Arc<DashMap<String, ExchangeConfig>>,
}

impl WsPool {
    pub fn new(config: TradingEngineConfig) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        
        let exchanges = Arc::new(DashMap::new());
        for (name, exchange_config) in config.exchanges {
            if exchange_config.enabled {
                exchanges.insert(name, exchange_config);
            }
        }
        
        Self {
            config: config.ws_pool,
            connections: Arc::new(DashMap::new()),
            message_tx,
            message_rx: Some(message_rx),
            shutdown_tx,
            shutdown_rx,
            exchanges,
        }
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        info!("Starting WebSocket pool");
        
        // Initialize connections for each exchange
        for entry in self.exchanges.iter() {
            let exchange_name = entry.key().clone();
            let exchange_config = entry.value().clone();
            
            // Initialize spot connections
            if exchange_config.spot.enabled {
                for url in &exchange_config.spot.ws_endpoints {
                    for _ in 0..exchange_config.spot.connection_count {
                        // Create subscription message based on exchange
                        let sub_msg = self.create_subscription_message(&exchange_name, "spot");
                        self.create_connection(
                            exchange_name.clone(),
                            "spot".to_string(),
                            url.clone(),
                            sub_msg,
                        ).await?;
                    }
                }
            }
            
            // Initialize futures connections
            if exchange_config.futures.enabled {
                for url in &exchange_config.futures.ws_endpoints {
                    for _ in 0..exchange_config.futures.connection_count {
                        let sub_msg = self.create_subscription_message(&exchange_name, "futures");
                        self.create_connection(
                            exchange_name.clone(),
                            "futures".to_string(),
                            url.clone(),
                            sub_msg,
                        ).await?;
                    }
                }
            }
        }
        
        // Start health monitoring
        self.start_health_monitor();
        
        // Start reconnection manager
        self.start_reconnection_manager();
        
        Ok(())
    }

    async fn create_connection(
        &self,
        exchange: String,
        market_type: String,
        url: String,
        sub_msg: Value,
    ) -> anyhow::Result<Uuid> {
        let base = BaseConnection::new(
            exchange.clone(),
            market_type.clone(),
            url,
            sub_msg,
            self.message_tx.clone(),
            self.shutdown_rx.clone(),
        );
        
        let id = base.id;
        
        // Store base connection for tracking
        self.connections.insert(id, base.clone());
        
        // Create and spawn the appropriate connection runner
        match exchange.as_str() {
            "binance" => {
                let mut connection = BinanceConnection::new(base);
                tokio::spawn(async move {
                    if let Err(e) = connection.run().await {
                        error!("Binance connection error: {}", e);
                    }
                });
            }
            "okex" => {
                let mut connection = OkexConnection::new(base);
                tokio::spawn(async move {
                    if let Err(e) = connection.run().await {
                        error!("OKEx connection error: {}", e);
                    }
                });
            }
            "bybit" => {
                let mut connection = BybitConnection::new(base);
                tokio::spawn(async move {
                    if let Err(e) = connection.run().await {
                        error!("Bybit connection error: {}", e);
                    }
                });
            }
            _ => {
                self.connections.remove(&id);
                return Err(anyhow::anyhow!("Unsupported exchange: {}", exchange));
            }
        };
        
        info!("Created connection {} for {} {}", id, exchange, market_type);
        Ok(id)
    }

    fn start_health_monitor(&self) {
        // Health monitoring is now handled by HealthTracker
        // This method can be used for additional monitoring if needed
        let interval_ms = self.config.health_check_interval_ms;
        
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(interval_ms));
            
            loop {
                interval.tick().await;
                // Health tracking is handled by the HealthTracker component
                debug!("Health check interval triggered");
            }
        });
    }

    fn start_reconnection_manager(&self) {
        // Reconnection is handled automatically by the connection runners
        // This method can be enhanced to manage reconnection centrally if needed
        let reconnect_delay_ms = self.config.reconnect_delay_ms;
        let max_attempts = self.config.max_reconnect_attempts;
        
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(5000));
            
            loop {
                interval.tick().await;
                // Connection runners handle their own reconnection
                debug!("Reconnection check interval triggered");
            }
        });
    }

    pub async fn get_healthy_connections(
        &self,
        exchange: &str,
        market_type: &str,
        top_k: usize,
    ) -> Vec<Uuid> {
        // This should be handled by the HealthTracker and ConnectionSelector
        // For now, return connection IDs that match the criteria
        let mut matching_ids = Vec::new();
        
        for entry in self.connections.iter() {
            let id = *entry.key();
            // Add basic filtering logic here
            matching_ids.push(id);
        }
        
        matching_ids.into_iter().take(top_k).collect()
    }

    pub async fn send_to_connection(&self, connection_id: Uuid, message: Vec<u8>) -> anyhow::Result<()> {
        if let Some(conn) = self.connections.get(&connection_id) {
            conn.send_command(ConnectionCommand::SendMessage(message))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Connection {} not found", connection_id))
        }
    }

    pub fn take_message_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<Bytes>> {
        self.message_rx.take()
    }

    fn create_subscription_message(&self, exchange: &str, market_type: &str) -> Value {
        // Create appropriate subscription message based on exchange
        match exchange {
            "binance" => {
                serde_json::json!({
                    "method": "SUBSCRIBE",
                    "params": [],
                    "id": 1
                })
            }
            "okex" => {
                serde_json::json!({
                    "op": "subscribe",
                    "args": []
                })
            }
            "bybit" => {
                serde_json::json!({
                    "op": "subscribe",
                    "args": []
                })
            }
            _ => serde_json::json!({})
        }
    }
    
    pub async fn shutdown(&self) {
        info!("Shutting down WebSocket pool");
        
        // Send shutdown signal
        let _ = self.shutdown_tx.send(true);
        
        // Wait a bit for connections to close gracefully
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        self.connections.clear();
    }
}