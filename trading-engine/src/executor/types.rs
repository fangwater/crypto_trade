use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCommand {
    pub id: Uuid,
    pub exchange: String,
    pub market_type: String, // "spot" or "futures"
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub client_order_id: Option<String>,
    pub reduce_only: bool,
    pub post_only: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,  // Good Till Cancel
    IOC,  // Immediate or Cancel
    FOK,  // Fill or Kill
    GTX,  // Good Till Crossing
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub time_in_force: TimeInForce,
    pub client_order_id: String,
    pub timestamp: i64,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub client_order_id: String,
    pub symbol: String,
    pub status: OrderStatus,
    pub executed_qty: Decimal,
    pub executed_price: Option<Decimal>,
    pub timestamp: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub command_id: Uuid,
    pub success: bool,
    pub responses: Vec<OrderResponse>,
    pub selected_response: Option<OrderResponse>,
    pub error: Option<String>,
}