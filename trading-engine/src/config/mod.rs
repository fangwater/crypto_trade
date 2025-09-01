use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingEngineConfig {
    pub exchanges: HashMap<String, ExchangeConfig>,
    pub ws_pool: WsPoolConfig,
    pub executor: ExecutorConfig,
    pub ipc: IpcConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub enabled: bool,
    pub spot: ExchangeEndpointConfig,
    pub futures: ExchangeEndpointConfig,
    pub api_key: String,
    pub secret_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeEndpointConfig {
    pub enabled: bool,
    pub ws_endpoints: Vec<String>,
    pub rest_endpoint: String,
    pub connection_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsPoolConfig {
    pub max_connections_per_exchange: usize,
    pub heartbeat_interval_ms: u64,
    pub reconnect_delay_ms: u64,
    pub max_reconnect_attempts: usize,
    pub health_check_interval_ms: u64,
    pub message_buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub order_timeout_ms: u64,
    pub max_retry_attempts: usize,
    pub concurrent_send_count: usize,
    pub idempotent_key_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    pub service_name: String,
    pub input_topic: String,
    pub output_topic: String,
    pub buffer_size: usize,
}

impl TradingEngineConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}