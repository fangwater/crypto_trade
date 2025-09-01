use std::collections::HashMap;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::{debug, warn};

use common::types::Signal;
use crate::pipeline::shared_state::SharedState;

/// 风控规则trait - 所有规则都实现这个接口
pub trait RiskRule {
    /// 规则名称
    fn name(&self) -> &str;
    
    /// 执行检查，返回是否通过
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool>;
    
    /// 是否为关键规则（失败后停止）
    fn is_critical(&self) -> bool {
        true
    }
}

/// 仓位规则
pub struct PositionRule {
    pub max_single_position_ratio: Decimal,
    pub max_total_position_ratio: Decimal,
    pub max_correlated_position_ratio: Decimal,
}

/// 频率控制规则
pub struct FrequencyRule {
    pub max_trades_per_minute: usize,
    pub max_trades_per_hour: usize,
    pub max_trades_per_day: usize,
    pub min_trade_interval_ms: u32,
}

/// 盈亏规则
pub struct PnLRule {
    pub max_daily_loss: Decimal,
    pub max_single_loss: Decimal,
    pub max_consecutive_losses: usize,
    pub max_drawdown: Decimal,
}

/// 市场条件规则
pub struct MarketRule {
    pub max_slippage: Decimal,
    pub min_liquidity: Decimal,
    pub max_volatility: Decimal,
}

/// 时间规则
pub struct TimeRule {
    pub trading_windows: Vec<(String, String, Vec<u32>)>,  // (开始时间, 结束时间, 星期几)
    pub blackout_dates: Vec<DateTime<Utc>>,
}

/// 品种特定规则
pub struct SymbolRule {
    pub symbol: String,
    pub max_position: Decimal,
    pub max_capital_used: Decimal,
    pub max_pending_orders: usize,
    pub max_trades_per_window: usize,
    pub time_window_seconds: u32,
}

/// 风控规则集合
pub struct RiskRules {
    pub position_rule: Option<PositionRule>,
    pub frequency_rule: Option<FrequencyRule>,
    pub pnl_rule: Option<PnLRule>,
    pub market_rule: Option<MarketRule>,
    pub time_rule: Option<TimeRule>,
    pub symbol_rules: HashMap<String, SymbolRule>,
    
    // 全局限制
    pub max_total_exposure_ratio: Decimal,
    pub max_position_symbols: usize,
    pub max_daily_trades: usize,
    pub max_daily_loss: Decimal,
    pub total_capital: Decimal,
}

impl RiskRules {
    pub fn new() -> Self {
        Self {
            position_rule: None,
            frequency_rule: None,
            pnl_rule: None,
            market_rule: None,
            time_rule: None,
            symbol_rules: HashMap::new(),
            max_total_exposure_ratio: Decimal::from_f64(0.03).unwrap(),
            max_position_symbols: 10,
            max_daily_trades: 1000,
            max_daily_loss: Decimal::from(10000),
            total_capital: Decimal::from(1000000),
        }
    }
    
    /// 更新全局限制
    pub fn update_global_limits(
        &mut self,
        max_total_exposure_ratio: f64,
        max_position_symbols: usize,
        max_daily_trades: usize,
        max_daily_loss: Decimal,
        total_capital: Decimal,
    ) {
        self.max_total_exposure_ratio = Decimal::from_f64(max_total_exposure_ratio)
            .unwrap_or(self.max_total_exposure_ratio);
        self.max_position_symbols = max_position_symbols;
        self.max_daily_trades = max_daily_trades;
        self.max_daily_loss = max_daily_loss;
        self.total_capital = total_capital;
    }
    
    /// 添加品种规则
    pub fn add_symbol_rule(
        &mut self,
        symbol: String,
        max_position: Decimal,
        max_capital_used: Decimal,
        max_pending_orders: usize,
        max_trades_per_window: usize,
        time_window_seconds: u32,
    ) {
        let rule = SymbolRule {
            symbol: symbol.clone(),
            max_position,
            max_capital_used,
            max_pending_orders,
            max_trades_per_window,
            time_window_seconds,
        };
        self.symbol_rules.insert(symbol, rule);
    }
}

/// 单品种仓位限制规则
pub struct PositionLimitRule {
    pub max_position: Decimal,  // 最大仓位（手数）
}

impl PositionLimitRule {
    pub fn new(max_position: Decimal) -> Self {
        Self { max_position }
    }
}

impl RiskRule for PositionLimitRule {
    fn name(&self) -> &str {
        "PositionLimit"
    }
    
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        let current_position = state.positions
            .get(&signal.symbol)
            .map(|p| p.quantity)
            .unwrap_or(Decimal::ZERO);
        
        let quantity = signal.quantity
            .and_then(|q| Decimal::from_f64(q))
            .unwrap_or(Decimal::ZERO);
        let new_position = current_position + quantity;
        
        if new_position.abs() > self.max_position {
            debug!(
                "Position limit exceeded for {}: current={}, signal={}, limit={}", 
                signal.symbol, current_position, quantity, self.max_position
            );
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// 单品种资金限制规则
pub struct CapitalLimitRule {
    pub max_capital: Decimal,  // 最大资金（USDT）
}

impl CapitalLimitRule {
    pub fn new(max_capital: Decimal) -> Self {
        Self { max_capital }
    }
}

impl RiskRule for CapitalLimitRule {
    fn name(&self) -> &str {
        "CapitalLimit"
    }
    
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        let quota = state.risk_quotas.get(&signal.symbol);
        let current_capital = quota
            .map(|q| q.current_capital)
            .unwrap_or(Decimal::ZERO);
        
        let signal_capital = signal.price
            .and_then(|p| signal.quantity.map(|q| p * q))
            .and_then(|c| Decimal::from_f64(c))
            .unwrap_or(Decimal::ZERO);
        let new_capital = current_capital + signal_capital;
        
        if new_capital > self.max_capital {
            debug!(
                "Capital limit exceeded for {}: current={}, signal={}, limit={}", 
                signal.symbol, current_capital, signal_capital, self.max_capital
            );
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// 挂单数量限制规则
pub struct PendingOrdersRule {
    pub max_pending: usize,  // 最大挂单数
}

impl PendingOrdersRule {
    pub fn new(max_pending: usize) -> Self {
        Self { max_pending }
    }
}

impl RiskRule for PendingOrdersRule {
    fn name(&self) -> &str {
        "PendingOrders"
    }
    
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        let quota = state.risk_quotas.get(&signal.symbol);
        let pending_orders = quota
            .map(|q| q.pending_orders)
            .unwrap_or(0);
        
        if pending_orders >= self.max_pending {
            debug!(
                "Pending orders limit exceeded for {}: current={}, limit={}", 
                signal.symbol, pending_orders, self.max_pending
            );
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// 总敞口限制规则
pub struct TotalExposureRule {
    pub max_exposure: Decimal,     // 最大总敞口
    pub warning_threshold: Decimal, // 预警阈值
}

impl TotalExposureRule {
    pub fn new(max_exposure: Decimal, warning_threshold: Decimal) -> Self {
        Self { max_exposure, warning_threshold }
    }
}

impl RiskRule for TotalExposureRule {
    fn name(&self) -> &str {
        "TotalExposure"
    }
    
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        let signal_exposure = signal.price
            .and_then(|p| signal.quantity.map(|q| (p * q).abs()))
            .and_then(|e| Decimal::from_f64(e))
            .unwrap_or(Decimal::ZERO);
        let new_exposure = state.total_exposure + signal_exposure;
        
        // 超过最大敞口，拒绝
        if new_exposure > self.max_exposure {
            warn!(
                "Total exposure limit exceeded: current={}, signal={}, limit={}", 
                state.total_exposure, signal_exposure, self.max_exposure
            );
            return Ok(false);
        }
        
        // 达到预警阈值，记录警告但允许交易
        if new_exposure > self.warning_threshold {
            warn!(
                "Total exposure warning: current={}, signal={}, warning={}", 
                state.total_exposure, signal_exposure, self.warning_threshold
            );
        }
        
        Ok(true)
    }
}

/// 日内交易次数限制规则
pub struct DailyTradesRule {
    pub max_daily_trades: usize,  // 最大日内交易次数
}

impl DailyTradesRule {
    pub fn new(max_daily_trades: usize) -> Self {
        Self { max_daily_trades }
    }
}

impl RiskRule for DailyTradesRule {
    fn name(&self) -> &str {
        "DailyTrades"
    }
    
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        let quota = state.risk_quotas.get(&signal.symbol);
        let daily_trades = quota
            .map(|q| q.daily_trades)
            .unwrap_or(0);
        
        if daily_trades >= self.max_daily_trades {
            debug!(
                "Daily trades limit exceeded for {}: current={}, limit={}", 
                signal.symbol, daily_trades, self.max_daily_trades
            );
            return Ok(false);
        }
        
        Ok(true)
    }
    
    fn is_critical(&self) -> bool {
        false  // 非关键规则，可以继续
    }
}

/// 交易冷却时间规则
pub struct CooldownRule {
    pub cooldown_seconds: i64,  // 冷却时间（秒）
}

impl CooldownRule {
    pub fn new(cooldown_seconds: i64) -> Self {
        Self { cooldown_seconds }
    }
}

impl RiskRule for CooldownRule {
    fn name(&self) -> &str {
        "Cooldown"
    }
    
    fn check(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        let quota = state.risk_quotas.get(&signal.symbol);
        
        if let Some(quota) = quota {
            if let Some(last_trade_time) = quota.last_trade_time {
                let elapsed = Utc::now()
                    .signed_duration_since(last_trade_time)
                    .num_seconds();
                
                if elapsed < self.cooldown_seconds {
                    debug!(
                        "Cooldown period active for {}: elapsed={}s, required={}s", 
                        signal.symbol, elapsed, self.cooldown_seconds
                    );
                    return Ok(false);
                }
            }
        }
        
        Ok(true)
    }
}

/// 信号时效性规则
pub struct SignalAgeRule {
    pub max_age_ms: i64,  // 最大信号年龄（毫秒）
}

impl SignalAgeRule {
    pub fn new(max_age_ms: i64) -> Self {
        Self { max_age_ms }
    }
}

impl RiskRule for SignalAgeRule {
    fn name(&self) -> &str {
        "SignalAge"
    }
    
    fn check(&self, signal: &Signal, _state: &SharedState) -> Result<bool> {
        let age_ms = Utc::now()
            .signed_duration_since(signal.timestamp)
            .num_milliseconds();
        
        if age_ms > self.max_age_ms {
            debug!(
                "Signal too old for {}: age={}ms, max={}ms", 
                signal.symbol, age_ms, self.max_age_ms
            );
            return Ok(false);
        }
        
        Ok(true)
    }
}

/// 风控规则链 - 按顺序执行所有规则
pub struct RiskRuleChain {
    rules: Vec<Box<dyn RiskRule>>,
}

impl RiskRuleChain {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }
    
    /// 添加规则
    pub fn add_rule(mut self, rule: Box<dyn RiskRule>) -> Self {
        self.rules.push(rule);
        self
    }
    
    /// 执行所有规则检查
    pub fn check_all(&self, signal: &Signal, state: &SharedState) -> Result<bool> {
        for rule in &self.rules {
            match rule.check(signal, state) {
                Ok(true) => {
                    debug!("Rule {} passed", rule.name());
                }
                Ok(false) => {
                    debug!("Rule {} failed", rule.name());
                    if rule.is_critical() {
                        return Ok(false);  // 关键规则失败，立即返回
                    }
                    // 非关键规则失败，继续检查
                }
                Err(e) => {
                    warn!("Rule {} error: {:?}", rule.name(), e);
                    if rule.is_critical() {
                        return Err(e);  // 关键规则错误，返回错误
                    }
                    // 非关键规则错误，继续检查
                }
            }
        }
        
        Ok(true)
    }
}

/// 创建默认的风控规则链
pub fn create_default_rule_chain() -> RiskRuleChain {
    RiskRuleChain::new()
        // 信号时效性检查（100ms）
        .add_rule(Box::new(SignalAgeRule::new(100)))
        // 单品种仓位限制（100手）
        .add_rule(Box::new(PositionLimitRule::new(Decimal::from(100))))
        // 单品种资金限制（5000 USDT）
        .add_rule(Box::new(CapitalLimitRule::new(Decimal::from(5000))))
        // 挂单数量限制（3个）
        .add_rule(Box::new(PendingOrdersRule::new(3)))
        // 总敞口限制（0.03）
        .add_rule(Box::new(TotalExposureRule::new(
            Decimal::from_f64(0.03).unwrap(),
            Decimal::from_f64(0.025).unwrap(),
        )))
        // 交易冷却时间（60秒）
        .add_rule(Box::new(CooldownRule::new(60)))
        // 日内交易次数限制（1000次）
        .add_rule(Box::new(DailyTradesRule::new(1000)))
}