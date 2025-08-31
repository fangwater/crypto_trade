use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::signals::Signal;
use crate::events::TradingEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcMessage {
    Signal(SignalMessage),
    Event(EventMessage),
    Control(ControlMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalMessage {
    pub signal: Signal,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMessage {
    pub event: TradingEvent,
    pub sequence_id: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlMessage {
    Start,
    Stop,
    Pause,
    Resume,
    Shutdown,
    HealthCheck,
    ConfigUpdate(String),
}