use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::types::{Symbol, Exchange, Side, OrderType, Priority, TriggerType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingEvent {
    OpenPosition(OpenPositionEvent),
    ClosePosition(ClosePositionEvent),
    HedgePosition(HedgePositionEvent),
    CancelOrder(CancelOrderEvent),
    ModifyOrder(ModifyOrderEvent),
}

impl TradingEvent {
    pub fn priority(&self) -> Priority {
        match self {
            TradingEvent::ClosePosition(_) => Priority::High,
            TradingEvent::HedgePosition(_) => Priority::High,
            TradingEvent::CancelOrder(_) => Priority::High,
            TradingEvent::ModifyOrder(_) => Priority::Medium,
            TradingEvent::OpenPosition(_) => Priority::Low,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenPositionEvent {
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub side: Side,
    pub quantity: f64,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub trigger_type: TriggerType,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosePositionEvent {
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub side: Side,
    pub quantity: f64,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub trigger_type: TriggerType,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HedgePositionEvent {
    pub symbol: Symbol,
    pub primary_exchange: Exchange,
    pub hedge_exchange: Exchange,
    pub side: Side,
    pub quantity: f64,
    pub trigger_type: TriggerType,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelOrderEvent {
    pub order_id: String,
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyOrderEvent {
    pub order_id: String,
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub new_price: Option<f64>,
    pub new_quantity: Option<f64>,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}