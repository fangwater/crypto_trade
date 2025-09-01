use super::adapter::AdapterTrait;
use crate::executor::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct OkexAdapter {
    rate_limits: HashMap<String, u32>,
}

impl OkexAdapter {
    pub fn new() -> Self {
        let mut rate_limits = HashMap::new();
        rate_limits.insert("orders_per_second".to_string(), 10);
        rate_limits.insert("orders_per_2seconds".to_string(), 20);
        
        Self { rate_limits }
    }
}

#[async_trait]
impl AdapterTrait for OkexAdapter {
    fn exchange_name(&self) -> &str {
        "okex"
    }
    
    fn format_ws_url(&self, market_type: &str, stream_type: &str) -> String {
        format!("wss://ws.okx.com:8443/ws/v5/{}", stream_type)
    }
    
    fn format_order_message(&self, request: &OrderRequest) -> Result<Vec<u8>, anyhow::Error> {
        let order_type = match request.order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::StopMarket => "trigger",
            OrderType::StopLimit => "trigger",
        };
        
        let side = match request.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };
        
        let msg = json!({
            "id": request.client_order_id,
            "op": "order",
            "args": [{
                "instId": request.symbol,
                "tdMode": "cash",
                "side": side,
                "ordType": order_type,
                "sz": request.quantity.to_string(),
                "px": request.price.map(|p| p.to_string()),
                "clOrdId": request.client_order_id,
            }]
        });
        
        Ok(serde_json::to_vec(&msg)?)
    }
    
    fn parse_order_response(&self, data: &[u8]) -> Result<OrderResponse, anyhow::Error> {
        let json: Value = serde_json::from_slice(data)?;
        
        // Check for error response
        if json.get("code").and_then(|v| v.as_str()) != Some("0") {
            let error_msg = json.get("msg")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            
            return Ok(OrderResponse {
                order_id: String::new(),
                client_order_id: String::new(),
                symbol: String::new(),
                status: OrderStatus::Rejected,
                executed_qty: Decimal::ZERO,
                executed_price: None,
                timestamp: chrono::Utc::now().timestamp_millis(),
                error: Some(error_msg),
            });
        }
        
        // Parse successful response
        let data = json.get("data")
            .and_then(|v| v.get(0))
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;
        
        let order_id = data.get("ordId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let client_order_id = data.get("clOrdId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        Ok(OrderResponse {
            order_id,
            client_order_id,
            symbol: String::new(),
            status: OrderStatus::New,
            executed_qty: Decimal::ZERO,
            executed_price: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            error: None,
        })
    }
    
    fn parse_market_data(&self, data: &[u8]) -> Result<Value, anyhow::Error> {
        Ok(serde_json::from_slice(data)?)
    }
    
    fn map_error_code(&self, code: i32) -> String {
        match code {
            1 => "Operation failed".to_string(),
            50000 => "General error".to_string(),
            50001 => "Service temporarily unavailable".to_string(),
            50002 => "Service busy".to_string(),
            50004 => "Request timeout".to_string(),
            50005 => "Too many requests".to_string(),
            50006 => "Invalid request".to_string(),
            50007 => "Invalid API key".to_string(),
            50008 => "Invalid signature".to_string(),
            51000 => "Invalid instrument".to_string(),
            51001 => "Instrument does not exist".to_string(),
            51006 => "Invalid order price".to_string(),
            51008 => "Order amount exceeds limit".to_string(),
            51009 => "Order placement failed".to_string(),
            51010 => "Insufficient balance".to_string(),
            _ => format!("Error code: {}", code),
        }
    }
    
    fn get_rate_limits(&self) -> HashMap<String, u32> {
        self.rate_limits.clone()
    }
}