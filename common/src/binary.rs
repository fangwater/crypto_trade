use bytes::{Bytes, BufMut, BytesMut, Buf};
use chrono::{DateTime, Utc};
use crate::types::{
    Signal, SignalData, SignalType, FundingDirection, RiskLevel, OrderResponseStatus,
};
use crate::events::TradingEvent;
use crate::messages::EventMessage;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinarySignalType {
    AdaptiveSpreadDeviation = 0,
    FixedSpreadDeviation = 1,
    FundingRateDirection = 2,
    RealTimeFundingRisk = 3,
    OrderResponse = 4,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    OpenPosition = 0,
    ClosePosition = 1,
    HedgePosition = 2,
    CancelOrder = 3,
    ModifyOrder = 4,
}

impl Signal {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        
        // 写入信号类型
        buf.put_u32_le(self.signal_type as u32);
        
        // 写入基本字段
        buf.put_u32_le(self.id.len() as u32);
        buf.put_slice(self.id.as_bytes());
        buf.put_u32_le(self.symbol.len() as u32);
        buf.put_slice(self.symbol.as_bytes());
        buf.put_u32_le(self.exchange.len() as u32);
        buf.put_slice(self.exchange.as_bytes());
        buf.put_i64_le(self.timestamp.timestamp_millis());
        
        // 根据具体数据类型写入详细信息
        match &self.data {
            SignalData::AdaptiveSpreadDeviation { 
                exchange_id, 
                symbol_id, 
                spread_percentile, 
                current_spread, 
                threshold_percentile 
            } => {
                buf.put_u32_le(*exchange_id);
                buf.put_u32_le(*symbol_id);
                buf.put_f64_le(*spread_percentile);
                buf.put_f64_le(*current_spread);
                buf.put_f64_le(*threshold_percentile);
            }
            SignalData::FixedSpreadDeviation { 
                exchange_id, 
                symbol_id, 
                current_spread, 
                fixed_threshold 
            } => {
                buf.put_u32_le(*exchange_id);
                buf.put_u32_le(*symbol_id);
                buf.put_f64_le(*current_spread);
                buf.put_f64_le(*fixed_threshold);
            }
            SignalData::FundingRateDirection { 
                exchange_id, 
                symbol_id, 
                funding_rate, 
                direction 
            } => {
                buf.put_u32_le(*exchange_id);
                buf.put_u32_le(*symbol_id);
                buf.put_f64_le(*funding_rate);
                buf.put_u32_le(*direction as u32);
            }
            SignalData::RealTimeFundingRisk { 
                exchange_id, 
                symbol_id, 
                risk_level, 
                funding_rate, 
                position_cost 
            } => {
                buf.put_u32_le(*exchange_id);
                buf.put_u32_le(*symbol_id);
                buf.put_u32_le(*risk_level as u32);
                buf.put_f64_le(*funding_rate);
                buf.put_f64_le(*position_cost);
            }
            SignalData::OrderResponse { 
                order_id, 
                exchange_id, 
                symbol_id, 
                status 
            } => {
                buf.put_u32_le(order_id.len() as u32);
                buf.put_slice(order_id.as_bytes());
                buf.put_u32_le(*exchange_id);
                buf.put_u32_le(*symbol_id);
                buf.put_u32_le(*status as u32);
            }
            _ => {
                // 其他类型暂不处理二进制序列化
            }
        }
        
        buf.freeze()
    }
    
    pub fn from_bytes(mut buf: Bytes) -> Result<Self, String> {
        if buf.remaining() < 4 {
            return Err("Buffer too small for signal type".to_string());
        }
        
        let signal_type = buf.get_u32_le();
        
        // 读取基本字段
        let id_len = buf.get_u32_le() as usize;
        let id = String::from_utf8(buf.copy_to_bytes(id_len).to_vec())
            .map_err(|e| e.to_string())?;
            
        let symbol_len = buf.get_u32_le() as usize;
        let symbol = String::from_utf8(buf.copy_to_bytes(symbol_len).to_vec())
            .map_err(|e| e.to_string())?;
            
        let exchange_len = buf.get_u32_le() as usize;
        let exchange = String::from_utf8(buf.copy_to_bytes(exchange_len).to_vec())
            .map_err(|e| e.to_string())?;
            
        let timestamp_millis = buf.get_i64_le();
        let timestamp = DateTime::<Utc>::from_timestamp_millis(timestamp_millis)
            .ok_or("Invalid timestamp")?;
        
        // 根据信号类型读取具体数据
        let (signal_type_enum, data) = match signal_type {
            0 => {
                let exchange_id = buf.get_u32_le();
                let symbol_id = buf.get_u32_le();
                let spread_percentile = buf.get_f64_le();
                let current_spread = buf.get_f64_le();
                let threshold_percentile = buf.get_f64_le();
                
                (SignalType::AdaptiveSpreadDeviation, SignalData::AdaptiveSpreadDeviation {
                    exchange_id,
                    symbol_id,
                    spread_percentile,
                    current_spread,
                    threshold_percentile,
                })
            }
            1 => {
                let exchange_id = buf.get_u32_le();
                let symbol_id = buf.get_u32_le();
                let current_spread = buf.get_f64_le();
                let fixed_threshold = buf.get_f64_le();
                
                (SignalType::FixedSpreadDeviation, SignalData::FixedSpreadDeviation {
                    exchange_id,
                    symbol_id,
                    current_spread,
                    fixed_threshold,
                })
            }
            2 => {
                let exchange_id = buf.get_u32_le();
                let symbol_id = buf.get_u32_le();
                let funding_rate = buf.get_f64_le();
                let direction = match buf.get_u32_le() {
                    0 => FundingDirection::Positive,
                    1 => FundingDirection::Negative,
                    _ => FundingDirection::Neutral,
                };
                
                (SignalType::FundingRateDirection, SignalData::FundingRateDirection {
                    exchange_id,
                    symbol_id,
                    funding_rate,
                    direction,
                })
            }
            3 => {
                let exchange_id = buf.get_u32_le();
                let symbol_id = buf.get_u32_le();
                let risk_level = match buf.get_u32_le() {
                    0 => RiskLevel::Low,
                    1 => RiskLevel::Medium,
                    2 => RiskLevel::High,
                    _ => RiskLevel::Critical,
                };
                let funding_rate = buf.get_f64_le();
                let position_cost = buf.get_f64_le();
                
                (SignalType::RealTimeFundingRisk, SignalData::RealTimeFundingRisk {
                    exchange_id,
                    symbol_id,
                    risk_level,
                    funding_rate,
                    position_cost,
                })
            }
            4 => {
                let order_id_len = buf.get_u32_le() as usize;
                let order_id = String::from_utf8(buf.copy_to_bytes(order_id_len).to_vec())
                    .map_err(|e| e.to_string())?;
                let exchange_id = buf.get_u32_le();
                let symbol_id = buf.get_u32_le();
                let status = match buf.get_u32_le() {
                    0 => OrderResponseStatus::Filled,
                    1 => OrderResponseStatus::PartiallyFilled,
                    2 => OrderResponseStatus::Rejected,
                    _ => OrderResponseStatus::Cancelled,
                };
                
                (SignalType::OrderResponse, SignalData::OrderResponse {
                    order_id,
                    exchange_id,
                    symbol_id,
                    status,
                })
            }
            _ => return Err(format!("Unknown signal type: {}", signal_type)),
        };
        
        let mut signal = Signal::new(signal_type_enum, data);
        signal.id = id;
        signal.symbol = symbol;
        signal.exchange = exchange;
        signal.timestamp = timestamp;
        
        Ok(signal)
    }
}

impl EventMessage {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(256);
        
        // 写入序列号和时间戳
        buf.put_u64_le(self.sequence_id);
        buf.put_i64_le(self.timestamp.timestamp_millis());
        
        // 根据事件类型写入具体数据
        match &self.event {
            TradingEvent::OpenPosition(e) => {
                buf.put_u32_le(EventType::OpenPosition as u32);
                buf.put_u32_le(e.symbol.0);  // Symbol是包装类型
                buf.put_u32_le(e.exchange as u32);
                buf.put_u32_le(e.side as u32);
                buf.put_f64_le(e.quantity);
                if let Some(price) = e.price {
                    buf.put_u8(1);
                    buf.put_f64_le(price);
                } else {
                    buf.put_u8(0);
                }
            }
            TradingEvent::ClosePosition(e) => {
                buf.put_u32_le(EventType::ClosePosition as u32);
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.exchange as u32);
                buf.put_u32_le(e.side as u32);
                buf.put_f64_le(e.quantity);
                if let Some(price) = e.price {
                    buf.put_u8(1);
                    buf.put_f64_le(price);
                } else {
                    buf.put_u8(0);
                }
            }
            TradingEvent::HedgePosition(e) => {
                buf.put_u32_le(EventType::HedgePosition as u32);
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.primary_exchange as u32);
                buf.put_u32_le(e.hedge_exchange as u32);
                buf.put_u32_le(e.side as u32);
                buf.put_f64_le(e.quantity);
            }
            TradingEvent::CancelOrder(e) => {
                buf.put_u32_le(EventType::CancelOrder as u32);
                buf.put_u32_le(e.order_id.len() as u32);
                buf.put_slice(e.order_id.as_bytes());
                buf.put_u32_le(e.reason.len() as u32);
                buf.put_slice(e.reason.as_bytes());
            }
            TradingEvent::ModifyOrder(e) => {
                buf.put_u32_le(EventType::ModifyOrder as u32);
                buf.put_u32_le(e.order_id.len() as u32);
                buf.put_slice(e.order_id.as_bytes());
                if let Some(new_price) = e.new_price {
                    buf.put_u8(1);
                    buf.put_f64_le(new_price);
                } else {
                    buf.put_u8(0);
                }
                if let Some(new_quantity) = e.new_quantity {
                    buf.put_u8(1);
                    buf.put_f64_le(new_quantity);
                } else {
                    buf.put_u8(0);
                }
            }
        }
        
        buf.freeze()
    }
}