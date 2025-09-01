use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// 从 types 模块导入统一的 Signal 定义
pub use crate::types::{Signal, SignalType, SignalData, FundingDirection, RiskLevel, OrderResponseStatus};

// 保留原有的具体信号结构，用于兼容性
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeFundingRiskSignal {
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub risk_level: RiskLevel,
    pub funding_rate: f64,
    pub position_cost: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponseSignal {
    pub order_id: String,
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub status: OrderResponseStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalStatus {
    pub signal_type: SignalType,
    pub last_signal: Option<Signal>,
    pub trigger_indices: Vec<usize>,
    pub last_updated: DateTime<Utc>,
}