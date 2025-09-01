use super::types::*;
use rust_decimal::Decimal;
use serde_json::Value;
use tracing::{debug, error, warn};

pub struct ResponseHandler {
    exchange: String,
}

impl ResponseHandler {
    pub fn new(exchange: String) -> Self {
        Self { exchange }
    }

    pub fn parse_response(&self, data: &[u8]) -> Result<OrderResponse, anyhow::Error> {
        let json: Value = serde_json::from_slice(data)?;
        
        match self.exchange.as_str() {
            "binance" => self.parse_binance_response(json),
            "okex" => self.parse_okex_response(json),
            "bybit" => self.parse_bybit_response(json),
            _ => Err(anyhow::anyhow!("Unsupported exchange: {}", self.exchange)),
        }
    }

    fn parse_binance_response(&self, json: Value) -> Result<OrderResponse, anyhow::Error> {
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
                error: Some(error_msg),
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
            .map(|s| self.parse_binance_status(s))
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

    fn parse_okex_response(&self, json: Value) -> Result<OrderResponse, anyhow::Error> {
        // Similar parsing logic for OKEx
        todo!("Implement OKEx response parsing")
    }

    fn parse_bybit_response(&self, json: Value) -> Result<OrderResponse, anyhow::Error> {
        // Similar parsing logic for Bybit
        todo!("Implement Bybit response parsing")
    }

    fn parse_binance_status(&self, status: &str) -> OrderStatus {
        match status {
            "NEW" => OrderStatus::New,
            "PARTIALLY_FILLED" => OrderStatus::PartiallyFilled,
            "FILLED" => OrderStatus::Filled,
            "CANCELED" => OrderStatus::Canceled,
            "REJECTED" => OrderStatus::Rejected,
            "EXPIRED" => OrderStatus::Expired,
            _ => OrderStatus::New,
        }
    }

    pub fn select_best_response(&self, responses: Vec<OrderResponse>) -> Option<OrderResponse> {
        // Select the best response from multiple responses
        // Priority: Filled > PartiallyFilled > New > Others
        responses.into_iter()
            .filter(|r| r.error.is_none())
            .max_by_key(|r| match r.status {
                OrderStatus::Filled => 6,
                OrderStatus::PartiallyFilled => 5,
                OrderStatus::New => 4,
                OrderStatus::Canceled => 3,
                OrderStatus::Expired => 2,
                OrderStatus::Rejected => 1,
            })
    }
}