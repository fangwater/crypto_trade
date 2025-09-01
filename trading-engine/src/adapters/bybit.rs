use super::adapter::AdapterTrait;
use crate::executor::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct BybitAdapter {
    rate_limits: HashMap<String, u32>,
}

impl BybitAdapter {
    pub fn new() -> Self {
        let mut rate_limits = HashMap::new();
        rate_limits.insert("orders_per_second".to_string(), 10);
        rate_limits.insert("api_rate_limit".to_string(), 100);
        
        Self { rate_limits }
    }
}

#[async_trait]
impl AdapterTrait for BybitAdapter {
    fn exchange_name(&self) -> &str {
        "bybit"
    }
    
    fn format_ws_url(&self, market_type: &str, stream_type: &str) -> String {
        match market_type {
            "spot" => format!("wss://stream.bybit.com/v5/public/spot"),
            "futures" => format!("wss://stream.bybit.com/v5/public/linear"),
            _ => String::new(),
        }
    }
    
    fn format_order_message(&self, request: &OrderRequest) -> Result<Vec<u8>, anyhow::Error> {
        let order_type = match request.order_type {
            OrderType::Market => "Market",
            OrderType::Limit => "Limit",
            OrderType::StopMarket => "Stop",
            OrderType::StopLimit => "StopLimit",
        };
        
        let side = match request.side {
            OrderSide::Buy => "Buy",
            OrderSide::Sell => "Sell",
        };
        
        let time_in_force = match request.time_in_force {
            TimeInForce::GTC => "GTC",
            TimeInForce::IOC => "IOC",
            TimeInForce::FOK => "FOK",
            TimeInForce::GTX => "PostOnly",
        };
        
        let msg = json!({
            "category": "spot",
            "symbol": request.symbol,
            "side": side,
            "orderType": order_type,
            "qty": request.quantity.to_string(),
            "price": request.price.map(|p| p.to_string()),
            "timeInForce": time_in_force,
            "orderLinkId": request.client_order_id,
        });
        
        Ok(serde_json::to_vec(&msg)?)
    }
    
    fn parse_order_response(&self, data: &[u8]) -> Result<OrderResponse, anyhow::Error> {
        let json: Value = serde_json::from_slice(data)?;
        
        // Check for error response
        if json.get("retCode").and_then(|v| v.as_i64()) != Some(0) {
            let error_msg = json.get("retMsg")
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
        let result = json.get("result")
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?;
        
        let order_id = result.get("orderId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let client_order_id = result.get("orderLinkId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let symbol = result.get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let status = result.get("orderStatus")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "New" => OrderStatus::New,
                "PartiallyFilled" => OrderStatus::PartiallyFilled,
                "Filled" => OrderStatus::Filled,
                "Cancelled" => OrderStatus::Canceled,
                "Rejected" => OrderStatus::Rejected,
                _ => OrderStatus::New,
            })
            .unwrap_or(OrderStatus::New);
        
        Ok(OrderResponse {
            order_id,
            client_order_id,
            symbol,
            status,
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
            10001 => "Parameter error".to_string(),
            10002 => "Request expired".to_string(),
            10003 => "API key error".to_string(),
            10004 => "Sign error".to_string(),
            10005 => "Permission denied".to_string(),
            10006 => "Too many requests".to_string(),
            10007 => "Invalid request".to_string(),
            10010 => "Server error".to_string(),
            20001 => "Order not exists".to_string(),
            20003 => "Operation not allowed".to_string(),
            20004 => "Duplicate order".to_string(),
            20005 => "Order amount too small".to_string(),
            20006 => "Order amount exceed limit".to_string(),
            20007 => "Order cancelled".to_string(),
            30001 => "Position not exists".to_string(),
            30003 => "Insufficient balance".to_string(),
            _ => format!("Error code: {}", code),
        }
    }
    
    fn get_rate_limits(&self) -> HashMap<String, u32> {
        self.rate_limits.clone()
    }
}