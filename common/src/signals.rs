use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalType {
    AdaptiveSpreadDeviation,
    FixedSpreadDeviation,
    FundingRateDirection,
    RealTimeFundingRisk,
    OrderResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Signal {
    AdaptiveSpreadDeviation(AdaptiveSpreadDeviationSignal),
    FixedSpreadDeviation(FixedSpreadDeviationSignal),
    FundingRateDirection(FundingRateDirectionSignal),
    RealTimeFundingRisk(RealTimeFundingRiskSignal),
    OrderResponse(OrderResponseSignal),
}

impl Signal {
    pub fn signal_type(&self) -> SignalType {
        match self {
            Signal::AdaptiveSpreadDeviation(_) => SignalType::AdaptiveSpreadDeviation,
            Signal::FixedSpreadDeviation(_) => SignalType::FixedSpreadDeviation,
            Signal::FundingRateDirection(_) => SignalType::FundingRateDirection,
            Signal::RealTimeFundingRisk(_) => SignalType::RealTimeFundingRisk,
            Signal::OrderResponse(_) => SignalType::OrderResponse,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveSpreadDeviationSignal {
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub spread_percentile: f64,
    pub current_spread: f64,
    pub threshold_percentile: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedSpreadDeviationSignal {
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub current_spread: f64,
    pub fixed_threshold: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRateDirectionSignal {
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub funding_rate: f64,
    pub direction: FundingDirection,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingDirection {
    Positive,
    Negative,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeFundingRiskSignal {
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub risk_level: RiskLevel,
    pub funding_rate: f64,
    pub position_cost: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponseSignal {
    pub order_id: String,
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub status: OrderResponseStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderResponseStatus {
    Filled,
    PartiallyFilled,
    Rejected,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStatus {
    pub signal_type: SignalType,
    pub last_signal: Option<Signal>,
    pub trigger_indices: Vec<usize>,
    pub last_updated: DateTime<Utc>,
}