use super::types::*;
use super::signer::Signer;
use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

pub struct OrderBuilder {
    exchange: String,
}

impl OrderBuilder {
    pub fn new(exchange: String) -> Self {
        Self { exchange }
    }

    pub fn build_order_request(
        &self,
        command: &ExecutionCommand,
        client_order_id: String,
        signer: &Signer,
    ) -> OrderRequest {
        let timestamp = Utc::now().timestamp_millis();
        
        let mut params = self.build_params(command, &client_order_id, timestamp);
        
        let signature = match self.exchange.as_str() {
            "binance" => signer.sign_binance(&params),
            "okex" => {
                let body = serde_json::to_string(&params).unwrap_or_default();
                signer.sign_okex(
                    &timestamp.to_string(),
                    "POST",
                    "/api/v5/trade/order",
                    &body,
                )
            }
            "bybit" => signer.sign_bybit(&params),
            _ => String::new(),
        };

        OrderRequest {
            symbol: command.symbol.clone(),
            side: command.side,
            order_type: command.order_type,
            quantity: command.quantity,
            price: command.price,
            time_in_force: command.time_in_force,
            client_order_id,
            timestamp,
            signature,
        }
    }

    fn build_params(
        &self,
        command: &ExecutionCommand,
        client_order_id: &str,
        timestamp: i64,
    ) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        
        params.insert("symbol".to_string(), command.symbol.clone());
        params.insert("side".to_string(), self.format_side(command.side));
        params.insert("type".to_string(), self.format_order_type(command.order_type));
        params.insert("quantity".to_string(), command.quantity.to_string());
        
        if let Some(price) = command.price {
            params.insert("price".to_string(), price.to_string());
        }
        
        params.insert("timeInForce".to_string(), self.format_time_in_force(command.time_in_force));
        params.insert("newClientOrderId".to_string(), client_order_id.to_string());
        params.insert("timestamp".to_string(), timestamp.to_string());
        
        if command.reduce_only {
            params.insert("reduceOnly".to_string(), "true".to_string());
        }
        
        if command.post_only {
            params.insert("postOnly".to_string(), "true".to_string());
        }
        
        params
    }

    fn format_side(&self, side: OrderSide) -> String {
        match side {
            OrderSide::Buy => "BUY".to_string(),
            OrderSide::Sell => "SELL".to_string(),
        }
    }

    fn format_order_type(&self, order_type: OrderType) -> String {
        match order_type {
            OrderType::Market => "MARKET".to_string(),
            OrderType::Limit => "LIMIT".to_string(),
            OrderType::StopMarket => "STOP_MARKET".to_string(),
            OrderType::StopLimit => "STOP_LIMIT".to_string(),
        }
    }

    fn format_time_in_force(&self, tif: TimeInForce) -> String {
        match tif {
            TimeInForce::GTC => "GTC".to_string(),
            TimeInForce::IOC => "IOC".to_string(),
            TimeInForce::FOK => "FOK".to_string(),
            TimeInForce::GTX => "GTX".to_string(),
        }
    }
}