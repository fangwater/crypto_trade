use chrono::Utc;
use common::types::{Signal, SignalType};
use common::signals::SignalStatus;

pub struct SignalManager {
    signals: Vec<SignalStatus>,  // 使用Vec，通过索引访问
}

impl SignalManager {
    pub fn new() -> Self {
        // 按照SignalType枚举顺序初始化
        let signals = vec![
            SignalStatus {
                signal_type: SignalType::AdaptiveSpreadDeviation,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::FixedSpreadDeviation,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::FundingRateDirection,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::RealTimeFundingRisk,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::OrderResponse,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::Arbitrage,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::Market,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
            SignalStatus {
                signal_type: SignalType::Hedge,
                last_signal: None,
                trigger_indices: Vec::new(),
                last_updated: Utc::now(),
            },
        ];

        Self { signals }
    }
    
    fn signal_type_to_idx(&self, signal_type: SignalType) -> usize {
        match signal_type {
            SignalType::AdaptiveSpreadDeviation => 0,
            SignalType::FixedSpreadDeviation => 1,
            SignalType::FundingRateDirection => 2,
            SignalType::RealTimeFundingRisk => 3,
            SignalType::OrderResponse => 4,
            SignalType::Arbitrage => 5,
            SignalType::Market => 6,
            SignalType::Hedge => 7,
        }
    }

    pub fn update_signal(&mut self, signal: Signal) {
        let idx = self.signal_type_to_idx(signal.signal_type);  // 直接访问字段
        if let Some(status) = self.signals.get_mut(idx) {
            status.last_signal = Some(signal);
            status.last_updated = Utc::now();
        }
    }

    pub fn get_status(&self, signal_idx: usize) -> Option<&SignalStatus> {
        self.signals.get(signal_idx)
    }
    
    pub fn get_status_by_type(&self, signal_type: SignalType) -> Option<&SignalStatus> {
        let idx = self.signal_type_to_idx(signal_type);
        self.signals.get(idx)
    }

    pub fn get_last_signal(&self, signal_idx: usize) -> Option<&Signal> {
        self.signals.get(signal_idx)?.last_signal.as_ref()
    }
    
    pub fn get_last_signal_by_type(&self, signal_type: SignalType) -> Option<&Signal> {
        let idx = self.signal_type_to_idx(signal_type);
        self.signals.get(idx)?.last_signal.as_ref()
    }

    pub fn register_trigger(&mut self, signal_idx: usize, trigger_idx: usize) {
        if let Some(status) = self.signals.get_mut(signal_idx) {
            if !status.trigger_indices.contains(&trigger_idx) {
                status.trigger_indices.push(trigger_idx);
            }
        }
    }

    pub fn unregister_trigger(&mut self, signal_idx: usize, trigger_idx: usize) {
        if let Some(status) = self.signals.get_mut(signal_idx) {
            status.trigger_indices.retain(|&idx| idx != trigger_idx);
        }
    }

    pub fn get_all_signals(&self) -> Vec<&Signal> {
        self.signals
            .iter()
            .filter_map(|status| status.last_signal.as_ref())
            .collect()
    }
    
    pub fn get_trigger_indices_for_signal(&self, signal_type: SignalType) -> Vec<usize> {
        let idx = self.signal_type_to_idx(signal_type);
        self.signals.get(idx)
            .map(|status| status.trigger_indices.clone())
            .unwrap_or_default()
    }
}