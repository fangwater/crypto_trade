use super::adapter::AdapterTrait;
use crate::executor::types::*;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct BinanceAdapter {
    rate_limits: HashMap<String, u32>,
}

impl BinanceAdapter {
    pub fn new() -> Self {
        let mut rate_limits = HashMap::new();
        rate_limits.insert("orders_per_second".to_string(), 10);
        rate_limits.insert("orders_per_minute".to_string(), 1200);
        rate_limits.insert("weight_per_minute".to_string(), 6000);
        
        Self { rate_limits }
    }
}

#[async_trait]
impl AdapterTrait for BinanceAdapter {
    fn exchange_name(&self) -> &str {
        "binance"
    }
    
    fn format_ws_url(&self, market_type: &str, stream_type: &str) -> String {
        match market_type {
            "spot" => format!("wss://stream.binance.com:9443/ws/{}", stream_type),
            "futures" => format!("wss://fstream.binance.com/ws/{}", stream_type),
            _ => String::new(),
        }
    }
    
    fn format_order_message(&self, request: &OrderRequest) -> Result<Vec<u8>, anyhow::Error> {
        let order_type = match request.order_type {
            OrderType::Market => "MARKET",
            OrderType::Limit => "LIMIT",
            OrderType::StopMarket => "STOP_MARKET",
            OrderType::StopLimit => "STOP_LIMIT",
        };
        
        let side = match request.side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };
        
        let time_in_force = match request.time_in_force {
            TimeInForce::GTC => "GTC",
            TimeInForce::IOC => "IOC",
            TimeInForce::FOK => "FOK",
            TimeInForce::GTX => "GTX",
        };
        
        let mut msg = json!({
            "symbol": request.symbol,
            "side": side,
            "type": order_type,
            "quantity": request.quantity.to_string(),
            "timeInForce": time_in_force,
            "newClientOrderId": request.client_order_id,
            "timestamp": request.timestamp,
            "signature": request.signature,
        });
        
        if let Some(price) = request.price {
            msg["price"] = json!(price.to_string());
        }
        
        Ok(serde_json::to_vec(&msg)?)
    }
    
    fn parse_order_response(&self, data: &[u8]) -> Result<OrderResponse, anyhow::Error> {
        let json: Value = serde_json::from_slice(data)?;
        
        // Check for error response
        if let Some(code) = json.get("code") {
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
                error: Some(format!("Error {}: {}", code, error_msg)),
            });
        }
        
        let order_id = json.get("orderId")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string())
            .unwrap_or_default();
        
        let client_order_id = json.get("clientOrderId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let symbol = json.get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        
        let status = json.get("status")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "NEW" => OrderStatus::New,
                "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
                "FILLED" => OrderStatus::Filled,
                "CANCELED" => OrderStatus::Canceled,
                "REJECTED" => OrderStatus::Rejected,
                "EXPIRED" => OrderStatus::Expired,
                _ => OrderStatus::New,
            })
            .unwrap_or(OrderStatus::New);
        
        let executed_qty = json.get("executedQty")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Decimal>().ok())
            .unwrap_or(Decimal::ZERO);
        
        let executed_price = json.get("price")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Decimal>().ok());
        
        let timestamp = json.get("transactTime")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        
        Ok(OrderResponse {
            order_id,
            client_order_id,
            symbol,
            status,
            executed_qty,
            executed_price,
            timestamp,
            error: None,
        })
    }
    
    fn parse_market_data(&self, data: &[u8]) -> Result<Value, anyhow::Error> {
        Ok(serde_json::from_slice(data)?)
    }
    
    fn map_error_code(&self, code: i32) -> String {
        match code {
            -1000 => "Unknown error".to_string(),
            -1001 => "Disconnected".to_string(),
            -1002 => "Unauthorized".to_string(),
            -1003 => "Too many requests".to_string(),
            -1013 => "Invalid quantity".to_string(),
            -1014 => "Unknown order".to_string(),
            -1015 => "Too many orders".to_string(),
            -1016 => "Service unavailable".to_string(),
            -1021 => "Invalid timestamp".to_string(),
            -1022 => "Invalid signature".to_string(),
            -2010 => "Insufficient balance".to_string(),
            -2011 => "Order canceled".to_string(),
            _ => format!("Error code: {}", code),
        }
    }
    
    fn get_rate_limits(&self) -> HashMap<String, u32> {
        self.rate_limits.clone()
    }
}