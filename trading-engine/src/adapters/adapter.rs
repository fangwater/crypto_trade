use crate::executor::types::*;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

#[async_trait]
pub trait AdapterTrait: Send + Sync {
    fn exchange_name(&self) -> &str;
    
    fn format_ws_url(&self, market_type: &str, stream_type: &str) -> String;
    
    fn format_order_message(&self, request: &OrderRequest) -> Result<Vec<u8>, anyhow::Error>;
    
    fn parse_order_response(&self, data: &[u8]) -> Result<OrderResponse, anyhow::Error>;
    
    fn parse_market_data(&self, data: &[u8]) -> Result<Value, anyhow::Error>;
    
    fn map_error_code(&self, code: i32) -> String;
    
    fn get_rate_limits(&self) -> HashMap<String, u32>;
}

pub struct ExchangeAdapter {
    adapters: HashMap<String, Box<dyn AdapterTrait>>,
}

impl ExchangeAdapter {
    pub fn new() -> Self {
        let mut adapters: HashMap<String, Box<dyn AdapterTrait>> = HashMap::new();
        
        adapters.insert("binance".to_string(), Box::new(super::BinanceAdapter::new()));
        adapters.insert("okex".to_string(), Box::new(super::OkexAdapter::new()));
        adapters.insert("bybit".to_string(), Box::new(super::BybitAdapter::new()));
        
        Self { adapters }
    }
    
    pub fn get_adapter(&self, exchange: &str) -> Option<&Box<dyn AdapterTrait>> {
        self.adapters.get(exchange)
    }
    
    pub fn list_exchanges(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
}