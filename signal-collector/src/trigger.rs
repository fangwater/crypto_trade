use std::rc::Rc;
use std::collections::HashMap;
use chrono::Utc;
use common::signals::{Signal, SignalType, FundingDirection};
use common::events::{TradingEvent, OpenPositionEvent};
use common::types::{Priority, Side, OrderType, TriggerType, Symbol, Exchange};
use crate::signal_manager::SignalManager;

pub trait Trigger {
    fn name(&self) -> &str;
    fn priority(&self) -> Priority;
    fn evaluate(&self, manager: &SignalManager, signal: &Signal) -> Option<TradingEvent>;
}

pub struct TriggerRegistry {
    triggers: Vec<Rc<dyn Trigger>>,
    name_to_idx: HashMap<String, usize>,
}

impl TriggerRegistry {
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
            name_to_idx: HashMap::new(),
        }
    }

    pub fn register(&mut self, trigger: Rc<dyn Trigger>) -> usize {
        let idx = self.triggers.len();
        let name = trigger.name().to_string();
        self.triggers.push(trigger);
        self.name_to_idx.insert(name, idx);
        idx
    }

    pub fn get_trigger(&self, idx: usize) -> Option<Rc<dyn Trigger>> {
        self.triggers.get(idx).cloned()
    }

    pub fn get_trigger_by_name(&self, name: &str) -> Option<(usize, Rc<dyn Trigger>)> {
        self.name_to_idx.get(name).and_then(|&idx| {
            self.triggers.get(idx).map(|t| (idx, t.clone()))
        })
    }

    pub fn register_default_triggers(&mut self) -> Vec<(usize, Vec<usize>)> {
        // 注册MT策略触发器
        let mt_trigger = Rc::new(MTTrigger::new());
        let mt_idx = self.register(mt_trigger);
        
        // 注册MT平仓触发器
        let mt_close_trigger = Rc::new(MTCloseTrigger::new());
        let mt_close_idx = self.register(mt_close_trigger);
        
        // 注册对冲触发器
        let hedge_trigger = Rc::new(HedgeTrigger::new());
        let hedge_idx = self.register(hedge_trigger);
        
        // 返回触发器索引和它们依赖的信号索引
        // 这里使用信号类型的枚举值作为索引
        vec![
            (mt_idx, vec![0, 1, 2]),           // AdaptiveSpread, FixedSpread, FundingRate
            (mt_close_idx, vec![3, 0, 1]),     // RealTimeFundingRisk, AdaptiveSpread, FixedSpread
            (hedge_idx, vec![0, 1]),           // AdaptiveSpread, FixedSpread
        ]
    }
}

pub struct MTTrigger {
    // 配置参数
    spread_threshold: f64,
    funding_threshold: f64,
}

impl MTTrigger {
    pub fn new() -> Self {
        Self {
            spread_threshold: 0.001,
            funding_threshold: 0.0001,
        }
    }
}

impl Trigger for MTTrigger {
    fn name(&self) -> &str {
        "MTTrigger"
    }

    fn priority(&self) -> Priority {
        Priority::Medium
    }

    fn evaluate(&self, manager: &SignalManager, signal: &Signal) -> Option<TradingEvent> {
        // 简单的测试逻辑
        match signal {
            Signal::FundingRateDirection(funding) => {
                // 检查是否有价差信号
                if let Some(Signal::AdaptiveSpreadDeviation(spread)) = 
                    manager.get_last_signal_by_type(SignalType::AdaptiveSpreadDeviation) {
                    if spread.spread_percentile > 0.8 && funding.funding_rate.abs() > self.funding_threshold {
                        let side = match funding.direction {
                            FundingDirection::Positive => Side::Sell,
                            FundingDirection::Negative => Side::Buy,
                            _ => return None,
                        };
                        
                        return Some(TradingEvent::OpenPosition(OpenPositionEvent {
                            symbol: Symbol(funding.symbol_id),
                            exchange: Exchange::Binance, // TODO: 从exchange_id转换
                            side,
                            quantity: 100.0,
                            order_type: OrderType::Market,
                            price: None,
                            trigger_type: TriggerType::MTTrigger,
                            reason: format!("MT开仓信号触发"),
                            timestamp: Utc::now(),
                        }));
                    }
                }
            }
            _ => {}
        }
        None
    }
}

pub struct MTCloseTrigger {
    // 配置参数
}

impl MTCloseTrigger {
    pub fn new() -> Self {
        Self {}
    }
}

impl Trigger for MTCloseTrigger {
    fn name(&self) -> &str {
        "MTCloseTrigger"
    }

    fn priority(&self) -> Priority {
        Priority::High
    }

    fn evaluate(&self, _manager: &SignalManager, _signal: &Signal) -> Option<TradingEvent> {
        // TODO: 实现MT平仓触发逻辑
        None
    }
}

pub struct HedgeTrigger {
    // 配置参数
}

impl HedgeTrigger {
    pub fn new() -> Self {
        Self {}
    }
}

impl Trigger for HedgeTrigger {
    fn name(&self) -> &str {
        "HedgeTrigger"
    }

    fn priority(&self) -> Priority {
        Priority::High
    }

    fn evaluate(&self, _manager: &SignalManager, _signal: &Signal) -> Option<TradingEvent> {
        // TODO: 实现对冲触发逻辑
        None
    }
}