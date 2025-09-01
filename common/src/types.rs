use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub u32);

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Symbol({})", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Binance,
    OKX,
    Bybit,
    Bitget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
    PostOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,  // Good Till Cancel
    IOC,  // Immediate or Cancel
    FOK,  // Fill or Kill
    GTX,  // Good Till Cross
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Placed,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub side: Side,
    pub order_type: OrderType,
    pub price: f64,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub status: OrderStatus,
    pub timestamp: DateTime<Utc>,
    pub priority: u8,
}

impl Order {
    pub fn from_signal(signal: &Signal) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            symbol: Symbol(0), // TODO: 从signal.symbol解析
            exchange: Exchange::Binance, // TODO: 从signal.exchange解析
            side: signal.side.unwrap_or(Side::Buy),
            order_type: OrderType::Market,
            price: signal.price.unwrap_or(0.0),
            quantity: signal.quantity.unwrap_or(0.0),
            filled_quantity: 0.0,
            status: OrderStatus::Pending,
            timestamp: chrono::Utc::now(),
            priority: signal.priority,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub side: Side,
    pub quantity: f64,
    pub avg_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerType {
    MTTrigger,
    MTCloseTrigger,
    HedgeTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub order_id: String,
    pub client_order_id: String,
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub side: Side,
    pub order_type: OrderType,
    pub price: f64,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub status: OrderStatus,
    pub execution_type: ExecutionType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionType {
    New,
    Trade,
    Cancelled,
    Rejected,
    Expired,
}

// Signal 统一定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalType {
    AdaptiveSpreadDeviation,
    FixedSpreadDeviation,
    FundingRateDirection,
    RealTimeFundingRisk,
    OrderResponse,
    Arbitrage,
    Market,
    Hedge,
    RiskControlInit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub id: String,
    pub signal_type: SignalType,
    pub symbol: String,
    pub exchange: String,
    pub side: Option<Side>,
    pub price: Option<f64>,
    pub quantity: Option<f64>,
    pub source: String,
    pub priority: u8,
    pub metadata: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
    // 具体信号数据
    pub data: SignalData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalData {
    AdaptiveSpreadDeviation {
        exchange_id: u32,
        symbol_id: u32,
        spread_percentile: f64,
        current_spread: f64,
        threshold_percentile: f64,
    },
    FixedSpreadDeviation {
        exchange_id: u32,
        symbol_id: u32,
        current_spread: f64,
        fixed_threshold: f64,
    },
    FundingRateDirection {
        exchange_id: u32,
        symbol_id: u32,
        funding_rate: f64,
        direction: FundingDirection,
    },
    RealTimeFundingRisk {
        exchange_id: u32,
        symbol_id: u32,
        risk_level: RiskLevel,
        funding_rate: f64,
        position_cost: f64,
    },
    OrderResponse {
        order_id: String,
        exchange_id: u32,
        symbol_id: u32,
        status: OrderResponseStatus,
    },
    Arbitrage {
        arbitrage_id: String,
        pair: (String, String),
        expected_profit: f64,
    },
    Market {
        market_data: String,
    },
    Hedge {
        hedge_id: String,
        target_position: f64,
    },
    RiskControlInit {
        protobuf_data: Vec<u8>,
    },
}

impl Signal {
    pub fn new(signal_type: SignalType, data: SignalData) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            signal_type,
            symbol: String::new(),
            exchange: String::new(),
            side: None,
            price: None,
            quantity: None,
            source: String::from("unknown"),
            priority: 1,
            metadata: HashMap::new(),
            timestamp: Utc::now(),
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingDirection {
    Positive,
    Negative,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderResponseStatus {
    Filled,
    PartiallyFilled,
    Rejected,
    Cancelled,
}