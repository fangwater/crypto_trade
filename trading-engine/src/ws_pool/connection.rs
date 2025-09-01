use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::{mpsc, watch};
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use serde_json::Value;
use std::sync::Arc;
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub enum ConnectionCommand {
    SendMessage(Vec<u8>),
    Disconnect,
}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_messages: u64,
    pub total_errors: u64,
    pub last_message_time: Option<tokio::time::Instant>,
    pub last_error_time: Option<tokio::time::Instant>,
    pub rtt_ms: f64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Clone)]
pub struct BaseConnection {
    pub id: Uuid,
    pub exchange: String,
    pub market_type: String,
    pub url: String,
    pub sub_msg: Value,
    pub message_tx: mpsc::UnboundedSender<Bytes>,
    pub shutdown_rx: watch::Receiver<bool>,
    pub command_tx: Option<mpsc::UnboundedSender<ConnectionCommand>>,
    pub state: Arc<RwLock<ConnectionState>>,
    pub stats: Arc<RwLock<ConnectionStats>>,
}

#[async_trait]
pub trait WsConnectionRunner: Send {
    async fn run(&mut self) -> anyhow::Result<()>;
}

impl BaseConnection {
    pub fn new(
        exchange: String,
        market_type: String,
        url: String,
        sub_msg: Value,
        message_tx: mpsc::UnboundedSender<Bytes>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            exchange,
            market_type,
            url,
            sub_msg,
            message_tx,
            shutdown_rx,
            command_tx: None,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            stats: Arc::new(RwLock::new(ConnectionStats {
                total_messages: 0,
                total_errors: 0,
                last_message_time: None,
                last_error_time: None,
                rtt_ms: 0.0,
                success_rate: 100.0,
            })),
        }
    }

    pub fn set_command_tx(&mut self, tx: mpsc::UnboundedSender<ConnectionCommand>) {
        self.command_tx = Some(tx);
    }

    pub fn send_command(&self, command: ConnectionCommand) -> anyhow::Result<()> {
        if let Some(tx) = &self.command_tx {
            tx.send(command).map_err(|e| anyhow::anyhow!("Failed to send command: {}", e))?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Command channel not initialized"))
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn exchange(&self) -> &str {
        &self.exchange
    }

    pub fn market_type(&self) -> &str {
        &self.market_type
    }
    
    pub fn update_stats<F>(&self, f: F) 
    where
        F: FnOnce(&mut ConnectionStats),
    {
        let mut stats = self.stats.write();
        f(&mut stats);
    }
    
    pub fn set_state(&self, state: ConnectionState) {
        *self.state.write() = state;
    }

    pub fn state(&self) -> ConnectionState {
        *self.state.read()
    }

    pub fn stats(&self) -> ConnectionStats {
        self.stats.read().clone()
    }

    pub fn health_score(&self) -> f64 {
        let stats = self.stats.read();
        let state = *self.state.read();
        
        let mut score = 0.0;
        
        // Connection state score (40%)
        score += match state {
            ConnectionState::Connected => 40.0,
            ConnectionState::Connecting => 20.0,
            ConnectionState::Disconnected => 10.0,
            ConnectionState::Error => 0.0,
        };
        
        // Success rate score (30%)
        score += stats.success_rate * 0.3;
        
        // RTT score (20%)
        let rtt_score = if stats.rtt_ms < 10.0 {
            20.0
        } else if stats.rtt_ms < 50.0 {
            15.0
        } else if stats.rtt_ms < 100.0 {
            10.0
        } else if stats.rtt_ms < 500.0 {
            5.0
        } else {
            0.0
        };
        score += rtt_score;
        
        // Recent activity score (10%)
        if let Some(last_msg_time) = stats.last_message_time {
            let elapsed = last_msg_time.elapsed().as_secs();
            if elapsed < 60 {
                score += 10.0;
            } else if elapsed < 300 {
                score += 5.0;
            }
        }
        
        score
    }
}