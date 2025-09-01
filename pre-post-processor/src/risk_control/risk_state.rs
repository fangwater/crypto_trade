use std::collections::HashMap;
use chrono::{DateTime, Utc, Timelike};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use tracing::{debug, info, warn};

use common::types::{Signal, ExecutionReport, OrderStatus};
use crate::risk_control::risk_calculator::RiskMetrics;

/// 风控状态 - 管理所有风控相关的状态信息
#[derive(Clone)]
pub struct RiskState {
    // 品种级别的风控状态
    pub symbol_states: HashMap<String, SymbolRiskState>,
    
    // 全局风控状态
    pub global_state: GlobalRiskState,
    
    // 风险指标
    pub metrics: RiskMetrics,
    
    // 最后更新时间
    pub last_update: DateTime<Utc>,
}

/// 单个品种的风控状态
#[derive(Debug, Clone)]
pub struct SymbolRiskState {
    pub symbol: String,
    
    // 仓位和资金
    pub position: Decimal,           // 当前仓位
    pub capital_used: Decimal,       // 占用资金
    pub pending_orders: usize,       // 挂单数量
    
    // 交易统计
    pub daily_trades: usize,         // 今日交易次数
    pub last_trade_time: Option<DateTime<Utc>>, // 最后交易时间
    pub trades_in_window: usize,     // 时间窗口内交易次数
    
    // 盈亏统计
    pub realized_pnl: Decimal,       // 已实现盈亏
    pub unrealized_pnl: Decimal,     // 未实现盈亏
    pub max_drawdown: Decimal,       // 最大回撤
    
    // 风控标记
    pub is_restricted: bool,         // 是否被限制交易
    pub restriction_reason: Option<String>, // 限制原因
    pub restriction_until: Option<DateTime<Utc>>, // 限制结束时间
}

impl SymbolRiskState {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            position: Decimal::ZERO,
            capital_used: Decimal::ZERO,
            pending_orders: 0,
            daily_trades: 0,
            last_trade_time: None,
            trades_in_window: 0,
            realized_pnl: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            is_restricted: false,
            restriction_reason: None,
            restriction_until: None,
        }
    }
    
    /// 检查是否可以交易
    pub fn can_trade(&self) -> bool {
        if self.is_restricted {
            if let Some(until) = self.restriction_until {
                if Utc::now() < until {
                    debug!("Symbol {} restricted until {}", self.symbol, until);
                    return false;
                }
                // 限制时间已过，解除限制
                return true;
            }
            return false;
        }
        true
    }
    
    /// 更新交易统计
    pub fn update_trade_stats(&mut self) {
        self.daily_trades += 1;
        self.trades_in_window += 1;
        self.last_trade_time = Some(Utc::now());
    }
    
    /// 设置交易限制
    pub fn set_restriction(&mut self, reason: String, duration_seconds: i64) {
        self.is_restricted = true;
        self.restriction_reason = Some(reason.clone());
        self.restriction_until = Some(Utc::now() + chrono::Duration::seconds(duration_seconds));
        warn!("Symbol {} restricted: {}", self.symbol, reason);
    }
    
    /// 清除交易限制
    pub fn clear_restriction(&mut self) {
        self.is_restricted = false;
        self.restriction_reason = None;
        self.restriction_until = None;
        info!("Symbol {} restriction cleared", self.symbol);
    }
    
    /// 重置日内统计（每日UTC 0点调用）
    pub fn reset_daily_stats(&mut self) {
        self.daily_trades = 0;
        self.trades_in_window = 0;
        debug!("Daily stats reset for {}", self.symbol);
    }
}

/// 全局风控状态
#[derive(Debug, Clone)]
pub struct GlobalRiskState {
    // 总体统计
    pub total_exposure: Decimal,     // 总敞口
    pub total_capital_used: Decimal, // 总占用资金
    pub total_positions: usize,      // 总持仓品种数
    
    // 日内统计
    pub daily_trades: usize,         // 今日总交易次数
    pub daily_pnl: Decimal,          // 今日总盈亏
    pub max_daily_drawdown: Decimal, // 今日最大回撤
    
    // 风险等级
    pub risk_level: RiskLevel,       // 当前风险等级
    pub last_risk_check: DateTime<Utc>, // 最后风险检查时间
    
    // 限制标记
    pub global_restricted: bool,     // 全局限制标记
    pub restriction_reason: Option<String>,
}

impl GlobalRiskState {
    pub fn new() -> Self {
        Self {
            total_exposure: Decimal::ZERO,
            total_capital_used: Decimal::ZERO,
            total_positions: 0,
            daily_trades: 0,
            daily_pnl: Decimal::ZERO,
            max_daily_drawdown: Decimal::ZERO,
            risk_level: RiskLevel::Low,
            last_risk_check: Utc::now(),
            global_restricted: false,
            restriction_reason: None,
        }
    }
    
    /// 更新风险等级
    pub fn update_risk_level(&mut self) {
        self.risk_level = if self.total_exposure > Decimal::from_f64(0.03).unwrap() {
            RiskLevel::Critical
        } else if self.total_exposure > Decimal::from_f64(0.025).unwrap() {
            RiskLevel::High
        } else if self.total_exposure > Decimal::from_f64(0.015).unwrap() {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };
        
        self.last_risk_check = Utc::now();
        
        if self.risk_level >= RiskLevel::High {
            warn!("Risk level elevated to {:?}, exposure: {}", self.risk_level, self.total_exposure);
        }
    }
    
    /// 设置全局限制
    pub fn set_global_restriction(&mut self, reason: String) {
        self.global_restricted = true;
        self.restriction_reason = Some(reason.clone());
        warn!("Global trading restricted: {}", reason);
    }
    
    /// 清除全局限制
    pub fn clear_global_restriction(&mut self) {
        self.global_restricted = false;
        self.restriction_reason = None;
        info!("Global restriction cleared");
    }
    
    /// 重置日内统计
    pub fn reset_daily_stats(&mut self) {
        self.daily_trades = 0;
        self.daily_pnl = Decimal::ZERO;
        self.max_daily_drawdown = Decimal::ZERO;
        debug!("Global daily stats reset");
    }
}

/// 风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,      // 低风险
    Medium,   // 中等风险
    High,     // 高风险
    Critical, // 危急
}

impl RiskState {
    pub fn new() -> Self {
        Self {
            symbol_states: HashMap::new(),
            global_state: GlobalRiskState::new(),
            metrics: RiskMetrics::new(),
            last_update: Utc::now(),
        }
    }
    
    /// 处理新信号 - 更新风控状态
    pub fn process_signal(&mut self, signal: &Signal) -> bool {
        // 检查全局限制
        if self.global_state.global_restricted {
            debug!("Global restriction active: {:?}", self.global_state.restriction_reason);
            return false;
        }
        
        // 获取或创建品种状态
        let symbol_state = self.symbol_states
            .entry(signal.symbol.clone())
            .or_insert_with(|| SymbolRiskState::new(signal.symbol.clone()));
        
        // 检查品种限制
        if !symbol_state.can_trade() {
            return false;
        }
        
        // 更新挂单数
        symbol_state.pending_orders += 1;
        
        // 更新全局统计
        let notional = signal.price
            .and_then(|p| signal.quantity.map(|q| p * q))
            .and_then(|n| Decimal::from_f64(n.abs()))
            .unwrap_or(Decimal::ZERO);
        self.global_state.total_exposure += notional;
        self.global_state.update_risk_level();
        
        true
    }
    
    /// 处理执行报告 - 更新风控状态
    pub fn process_execution(&mut self, report: &ExecutionReport) {
        let symbol_str = format!("{:?}", report.symbol);
        let symbol_state = self.symbol_states
            .entry(symbol_str.clone())
            .or_insert_with(|| SymbolRiskState::new(symbol_str));
        
        // 更新仓位和资金
        let filled_quantity = Decimal::from_f64(report.filled_quantity).unwrap_or(Decimal::ZERO);
        let price = Decimal::from_f64(report.price).unwrap_or(Decimal::ZERO);
        
        if report.side == common::types::Side::Buy {
            symbol_state.position += filled_quantity;
            symbol_state.capital_used += price * filled_quantity;
        } else {
            symbol_state.position -= filled_quantity;
            symbol_state.capital_used -= price * filled_quantity;
        }
        
        // 更新交易统计
        if report.status == OrderStatus::Filled {
            symbol_state.update_trade_stats();
            symbol_state.pending_orders = symbol_state.pending_orders.saturating_sub(1);
            
            self.global_state.daily_trades += 1;
        }
        
        // 重新计算全局敞口
        self.recalculate_global_exposure();
        
        self.last_update = Utc::now();
    }
    
    /// 重新计算全局敞口
    fn recalculate_global_exposure(&mut self) {
        self.global_state.total_exposure = self.symbol_states
            .values()
            .map(|s| s.capital_used.abs())
            .sum();
        
        self.global_state.total_positions = self.symbol_states
            .values()
            .filter(|s| s.position != Decimal::ZERO)
            .count();
        
        self.global_state.total_capital_used = self.symbol_states
            .values()
            .map(|s| s.capital_used)
            .sum();
        
        self.global_state.update_risk_level();
    }
    
    /// 检查并重置日内统计（定时任务调用）
    pub fn check_daily_reset(&mut self) {
        let now = Utc::now();
        if now.hour() == 0 && now.minute() == 0 {
            info!("Resetting daily risk statistics");
            
            // 重置所有品种的日内统计
            for symbol_state in self.symbol_states.values_mut() {
                symbol_state.reset_daily_stats();
            }
            
            // 重置全局日内统计
            self.global_state.reset_daily_stats();
            
            // 重新计算风险指标
            self.metrics.reset_daily_metrics();
        }
    }
    
    /// 获取风控摘要信息
    pub fn get_summary(&self) -> RiskSummary {
        RiskSummary {
            total_exposure: self.global_state.total_exposure,
            risk_level: self.global_state.risk_level,
            active_positions: self.global_state.total_positions,
            daily_trades: self.global_state.daily_trades,
            daily_pnl: self.global_state.daily_pnl,
            restricted_symbols: self.symbol_states
                .values()
                .filter(|s| s.is_restricted)
                .map(|s| s.symbol.clone())
                .collect(),
            global_restricted: self.global_state.global_restricted,
        }
    }
}

/// 风控摘要信息
#[derive(Debug, Clone)]
pub struct RiskSummary {
    pub total_exposure: Decimal,
    pub risk_level: RiskLevel,
    pub active_positions: usize,
    pub daily_trades: usize,
    pub daily_pnl: Decimal,
    pub restricted_symbols: Vec<String>,
    pub global_restricted: bool,
}