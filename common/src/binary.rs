use bytes::{Bytes, BufMut, BytesMut, Buf};
use chrono::DateTime;
use crate::signals::*;
use crate::types::*;
use crate::events::*;
use crate::messages::EventMessage;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalType {
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
        
        match self {
            Signal::AdaptiveSpreadDeviation(s) => {
                buf.put_u32_le(SignalType::AdaptiveSpreadDeviation as u32);
                buf.put_u32_le(s.exchange_id);
                buf.put_u32_le(s.symbol_id);
                buf.put_f64_le(s.spread_percentile);
                buf.put_f64_le(s.current_spread);
                buf.put_f64_le(s.threshold_percentile);
                buf.put_i64_le(s.timestamp.timestamp_millis());
            }
            Signal::FixedSpreadDeviation(s) => {
                buf.put_u32_le(SignalType::FixedSpreadDeviation as u32);
                buf.put_u32_le(s.exchange_id);
                buf.put_u32_le(s.symbol_id);
                buf.put_f64_le(s.current_spread);
                buf.put_f64_le(s.fixed_threshold);
                buf.put_i64_le(s.timestamp.timestamp_millis());
            }
            Signal::FundingRateDirection(s) => {
                buf.put_u32_le(SignalType::FundingRateDirection as u32);
                buf.put_u32_le(s.exchange_id);
                buf.put_u32_le(s.symbol_id);
                buf.put_f64_le(s.funding_rate);
                buf.put_u32_le(s.direction as u32);
                buf.put_i64_le(s.timestamp.timestamp_millis());
            }
            Signal::RealTimeFundingRisk(s) => {
                buf.put_u32_le(SignalType::RealTimeFundingRisk as u32);
                buf.put_u32_le(s.exchange_id);
                buf.put_u32_le(s.symbol_id);
                buf.put_u32_le(s.risk_level as u32);
                buf.put_f64_le(s.funding_rate);
                buf.put_f64_le(s.position_cost);
                buf.put_i64_le(s.timestamp.timestamp_millis());
            }
            Signal::OrderResponse(s) => {
                buf.put_u32_le(SignalType::OrderResponse as u32);
                buf.put_u32_le(s.order_id.len() as u32);
                buf.put(s.order_id.as_bytes());
                buf.put_u32_le(s.exchange_id);
                buf.put_u32_le(s.symbol_id);
                buf.put_u32_le(s.status as u32);
                buf.put_i64_le(s.timestamp.timestamp_millis());
            }
        }
        
        buf.freeze()
    }
    
    pub fn from_bytes(mut bytes: Bytes) -> std::io::Result<Self> {
        if bytes.len() < 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not enough data for signal type"
            ));
        }
        
        let signal_type = bytes.get_u32_le();
        
        match signal_type {
            0 => {
                // AdaptiveSpreadDeviation: 4 + 4 + 8 + 8 + 8 + 8 = 40 bytes minimum
                if bytes.remaining() < 36 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not enough data for AdaptiveSpreadDeviation"
                    ));
                }
                let exchange_id = bytes.get_u32_le();
                let symbol_id = bytes.get_u32_le();
                let spread_percentile = bytes.get_f64_le();
                let current_spread = bytes.get_f64_le();
                let threshold_percentile = bytes.get_f64_le();
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(Signal::AdaptiveSpreadDeviation(AdaptiveSpreadDeviationSignal {
                    exchange_id,
                    symbol_id,
                    spread_percentile,
                    current_spread,
                    threshold_percentile,
                    timestamp,
                }))
            }
            1 => {
                // FixedSpreadDeviation: 4 + 4 + 8 + 8 + 8 = 32 bytes minimum
                if bytes.remaining() < 28 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not enough data for FixedSpreadDeviation"
                    ));
                }
                let exchange_id = bytes.get_u32_le();
                let symbol_id = bytes.get_u32_le();
                let current_spread = bytes.get_f64_le();
                let fixed_threshold = bytes.get_f64_le();
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(Signal::FixedSpreadDeviation(FixedSpreadDeviationSignal {
                    exchange_id,
                    symbol_id,
                    current_spread,
                    fixed_threshold,
                    timestamp,
                }))
            }
            2 => {
                // FundingRateDirection: 4 + 4 + 8 + 4 + 8 = 28 bytes minimum
                if bytes.remaining() < 24 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not enough data for FundingRateDirection"
                    ));
                }
                let exchange_id = bytes.get_u32_le();
                let symbol_id = bytes.get_u32_le();
                let funding_rate = bytes.get_f64_le();
                let direction = FundingDirection::from_u32(bytes.get_u32_le())?;
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(Signal::FundingRateDirection(FundingRateDirectionSignal {
                    exchange_id,
                    symbol_id,
                    funding_rate,
                    direction,
                    timestamp,
                }))
            }
            3 => {
                // RealTimeFundingRisk: 4 + 4 + 4 + 8 + 8 + 8 = 36 bytes minimum
                if bytes.remaining() < 32 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not enough data for RealTimeFundingRisk"
                    ));
                }
                let exchange_id = bytes.get_u32_le();
                let symbol_id = bytes.get_u32_le();
                let risk_level = RiskLevel::from_u32(bytes.get_u32_le())?;
                let funding_rate = bytes.get_f64_le();
                let position_cost = bytes.get_f64_le();
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(Signal::RealTimeFundingRisk(RealTimeFundingRiskSignal {
                    exchange_id,
                    symbol_id,
                    risk_level,
                    funding_rate,
                    position_cost,
                    timestamp,
                }))
            }
            4 => {
                // OrderResponse: at least 4 bytes for order_id_len
                if bytes.remaining() < 4 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not enough data for OrderResponse"
                    ));
                }
                let order_id_len = bytes.get_u32_le() as usize;
                // Check if we have enough bytes for the full message
                if bytes.remaining() < order_id_len + 16 { // order_id + symbol(4) + exchange(4) + status(4) + timestamp(8)
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not enough data for OrderResponse fields"
                    ));
                }
                let order_id_bytes = bytes.copy_to_bytes(order_id_len);
                let order_id = String::from_utf8(order_id_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let exchange_id = bytes.get_u32_le();
                let symbol_id = bytes.get_u32_le();
                let status = OrderResponseStatus::from_u32(bytes.get_u32_le())?;
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(Signal::OrderResponse(OrderResponseSignal {
                    order_id,
                    exchange_id,
                    symbol_id,
                    status,
                    timestamp,
                }))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid signal type: {}", signal_type)
            ))
        }
    }
}

impl TradingEvent {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(512);
        
        match self {
            TradingEvent::OpenPosition(e) => {
                buf.put_u32_le(EventType::OpenPosition as u32);
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.exchange as u32);
                buf.put_u32_le(e.side as u32);
                buf.put_f64_le(e.quantity);
                buf.put_u32_le(e.order_type as u32);
                
                // Handle Option<f64> for price
                match e.price {
                    Some(price) => {
                        buf.put_u8(1);
                        buf.put_f64_le(price);
                    }
                    None => {
                        buf.put_u8(0);
                    }
                }
                
                buf.put_u32_le(e.trigger_type as u32);
                buf.put_u32_le(e.reason.len() as u32);
                buf.put(e.reason.as_bytes());
                buf.put_i64_le(e.timestamp.timestamp_millis());
            }
            TradingEvent::ClosePosition(e) => {
                buf.put_u32_le(EventType::ClosePosition as u32);
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.exchange as u32);
                buf.put_u32_le(e.side as u32);
                buf.put_f64_le(e.quantity);
                buf.put_u32_le(e.order_type as u32);
                
                // Handle Option<f64> for price
                match e.price {
                    Some(price) => {
                        buf.put_u8(1);
                        buf.put_f64_le(price);
                    }
                    None => {
                        buf.put_u8(0);
                    }
                }
                
                buf.put_u32_le(e.trigger_type as u32);
                buf.put_u32_le(e.reason.len() as u32);
                buf.put(e.reason.as_bytes());
                buf.put_i64_le(e.timestamp.timestamp_millis());
            }
            TradingEvent::HedgePosition(e) => {
                buf.put_u32_le(EventType::HedgePosition as u32);
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.primary_exchange as u32);
                buf.put_u32_le(e.hedge_exchange as u32);
                buf.put_u32_le(e.side as u32);
                buf.put_f64_le(e.quantity);
                buf.put_u32_le(e.trigger_type as u32);
                buf.put_u32_le(e.reason.len() as u32);
                buf.put(e.reason.as_bytes());
                buf.put_i64_le(e.timestamp.timestamp_millis());
            }
            TradingEvent::CancelOrder(e) => {
                buf.put_u32_le(EventType::CancelOrder as u32);
                buf.put_u32_le(e.order_id.len() as u32);
                buf.put(e.order_id.as_bytes());
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.exchange as u32);
                buf.put_u32_le(e.reason.len() as u32);
                buf.put(e.reason.as_bytes());
                buf.put_i64_le(e.timestamp.timestamp_millis());
            }
            TradingEvent::ModifyOrder(e) => {
                buf.put_u32_le(EventType::ModifyOrder as u32);
                buf.put_u32_le(e.order_id.len() as u32);
                buf.put(e.order_id.as_bytes());
                buf.put_u32_le(e.symbol.0);
                buf.put_u32_le(e.exchange as u32);
                
                // Handle Option<f64> for new_price
                match e.new_price {
                    Some(price) => {
                        buf.put_u8(1);
                        buf.put_f64_le(price);
                    }
                    None => {
                        buf.put_u8(0);
                    }
                }
                
                // Handle Option<f64> for new_quantity
                match e.new_quantity {
                    Some(quantity) => {
                        buf.put_u8(1);
                        buf.put_f64_le(quantity);
                    }
                    None => {
                        buf.put_u8(0);
                    }
                }
                
                buf.put_u32_le(e.reason.len() as u32);
                buf.put(e.reason.as_bytes());
                buf.put_i64_le(e.timestamp.timestamp_millis());
            }
        }
        
        buf.freeze()
    }
    
    pub fn from_bytes(mut bytes: Bytes) -> std::io::Result<Self> {
        if bytes.len() < 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not enough data for event type"
            ));
        }
        
        let event_type = bytes.get_u32_le();
        
        match event_type {
            0 => { // OpenPosition
                let symbol = Symbol(bytes.get_u32_le());
                let exchange = Exchange::from_u32(bytes.get_u32_le())?;
                let side = Side::from_u32(bytes.get_u32_le())?;
                let quantity = bytes.get_f64_le();
                let order_type = OrderType::from_u32(bytes.get_u32_le())?;
                
                let price = if bytes.get_u8() == 1 {
                    Some(bytes.get_f64_le())
                } else {
                    None
                };
                
                let trigger_type = TriggerType::from_u32(bytes.get_u32_le())?;
                
                let reason_len = bytes.get_u32_le() as usize;
                let reason_bytes = bytes.copy_to_bytes(reason_len);
                let reason = String::from_utf8(reason_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(TradingEvent::OpenPosition(OpenPositionEvent {
                    symbol,
                    exchange,
                    side,
                    quantity,
                    order_type,
                    price,
                    trigger_type,
                    reason,
                    timestamp,
                }))
            }
            1 => { // ClosePosition
                let symbol = Symbol(bytes.get_u32_le());
                let exchange = Exchange::from_u32(bytes.get_u32_le())?;
                let side = Side::from_u32(bytes.get_u32_le())?;
                let quantity = bytes.get_f64_le();
                let order_type = OrderType::from_u32(bytes.get_u32_le())?;
                
                let price = if bytes.get_u8() == 1 {
                    Some(bytes.get_f64_le())
                } else {
                    None
                };
                
                let trigger_type = TriggerType::from_u32(bytes.get_u32_le())?;
                
                let reason_len = bytes.get_u32_le() as usize;
                let reason_bytes = bytes.copy_to_bytes(reason_len);
                let reason = String::from_utf8(reason_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(TradingEvent::ClosePosition(ClosePositionEvent {
                    symbol,
                    exchange,
                    side,
                    quantity,
                    order_type,
                    price,
                    trigger_type,
                    reason,
                    timestamp,
                }))
            }
            2 => { // HedgePosition
                let symbol = Symbol(bytes.get_u32_le());
                let primary_exchange = Exchange::from_u32(bytes.get_u32_le())?;
                let hedge_exchange = Exchange::from_u32(bytes.get_u32_le())?;
                let side = Side::from_u32(bytes.get_u32_le())?;
                let quantity = bytes.get_f64_le();
                let trigger_type = TriggerType::from_u32(bytes.get_u32_le())?;
                
                let reason_len = bytes.get_u32_le() as usize;
                let reason_bytes = bytes.copy_to_bytes(reason_len);
                let reason = String::from_utf8(reason_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(TradingEvent::HedgePosition(HedgePositionEvent {
                    symbol,
                    primary_exchange,
                    hedge_exchange,
                    side,
                    quantity,
                    trigger_type,
                    reason,
                    timestamp,
                }))
            }
            3 => { // CancelOrder
                let order_id_len = bytes.get_u32_le() as usize;
                let order_id_bytes = bytes.copy_to_bytes(order_id_len);
                let order_id = String::from_utf8(order_id_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let symbol = Symbol(bytes.get_u32_le());
                let exchange = Exchange::from_u32(bytes.get_u32_le())?;
                
                let reason_len = bytes.get_u32_le() as usize;
                let reason_bytes = bytes.copy_to_bytes(reason_len);
                let reason = String::from_utf8(reason_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(TradingEvent::CancelOrder(CancelOrderEvent {
                    order_id,
                    symbol,
                    exchange,
                    reason,
                    timestamp,
                }))
            }
            4 => { // ModifyOrder
                let order_id_len = bytes.get_u32_le() as usize;
                let order_id_bytes = bytes.copy_to_bytes(order_id_len);
                let order_id = String::from_utf8(order_id_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let symbol = Symbol(bytes.get_u32_le());
                let exchange = Exchange::from_u32(bytes.get_u32_le())?;
                
                let new_price = if bytes.get_u8() == 1 {
                    Some(bytes.get_f64_le())
                } else {
                    None
                };
                
                let new_quantity = if bytes.get_u8() == 1 {
                    Some(bytes.get_f64_le())
                } else {
                    None
                };
                
                let reason_len = bytes.get_u32_le() as usize;
                let reason_bytes = bytes.copy_to_bytes(reason_len);
                let reason = String::from_utf8(reason_bytes.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                
                let timestamp_millis = bytes.get_i64_le();
                let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid timestamp"
                    ))?;
                
                Ok(TradingEvent::ModifyOrder(ModifyOrderEvent {
                    order_id,
                    symbol,
                    exchange,
                    new_price,
                    new_quantity,
                    reason,
                    timestamp,
                }))
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid event type: {}", event_type)
            ))
        }
    }
}

impl EventMessage {
    pub fn to_bytes(&self) -> Bytes {
        let event_bytes = self.event.to_bytes();
        let mut buf = BytesMut::with_capacity(event_bytes.len() + 16);
        
        buf.put(event_bytes);
        buf.put_u64_le(self.sequence_id);
        buf.put_i64_le(self.timestamp.timestamp_millis());
        
        buf.freeze()
    }
    
    pub fn from_bytes(bytes: Bytes) -> std::io::Result<Self> {
        let total_len = bytes.len();
        if total_len < 16 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not enough data for EventMessage"
            ));
        }
        
        // Split off the last 16 bytes for sequence_id and timestamp
        let event_bytes = bytes.slice(0..total_len - 16);
        let mut metadata_bytes = bytes.slice(total_len - 16..);
        
        let event = TradingEvent::from_bytes(event_bytes)?;
        let sequence_id = metadata_bytes.get_u64_le();
        let timestamp_millis = metadata_bytes.get_i64_le();
        let timestamp = DateTime::from_timestamp_millis(timestamp_millis)
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid timestamp"
            ))?;
        
        Ok(EventMessage {
            event,
            sequence_id,
            timestamp,
        })
    }
}

// Helper trait implementations for enum conversions
impl Exchange {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(Exchange::Binance),
            1 => Ok(Exchange::OKX),
            2 => Ok(Exchange::Bybit),
            3 => Ok(Exchange::Bitget),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid Exchange: {}", value)
            ))
        }
    }
}

impl Side {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(Side::Buy),
            1 => Ok(Side::Sell),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid Side: {}", value)
            ))
        }
    }
}

impl FundingDirection {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(FundingDirection::Positive),
            1 => Ok(FundingDirection::Negative),
            2 => Ok(FundingDirection::Neutral),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid FundingDirection: {}", value)
            ))
        }
    }
}

impl RiskLevel {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(RiskLevel::Low),
            1 => Ok(RiskLevel::Medium),
            2 => Ok(RiskLevel::High),
            3 => Ok(RiskLevel::Critical),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid RiskLevel: {}", value)
            ))
        }
    }
}

impl OrderType {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(OrderType::Market),
            1 => Ok(OrderType::Limit),
            2 => Ok(OrderType::PostOnly),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid OrderType: {}", value)
            ))
        }
    }
}

impl TriggerType {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(TriggerType::MTTrigger),
            1 => Ok(TriggerType::MTCloseTrigger),
            2 => Ok(TriggerType::HedgeTrigger),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid TriggerType: {}", value)
            ))
        }
    }
}

impl OrderResponseStatus {
    fn from_u32(value: u32) -> std::io::Result<Self> {
        match value {
            0 => Ok(OrderResponseStatus::Filled),
            1 => Ok(OrderResponseStatus::PartiallyFilled),
            2 => Ok(OrderResponseStatus::Rejected),
            3 => Ok(OrderResponseStatus::Cancelled),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid OrderResponseStatus: {}", value)
            ))
        }
    }
}