use std::collections::HashMap;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{debug, warn};

use common::types::{Signal, ExecutionReport};

/// 仓位信息
#[derive(Debug, Clone)]
pub struct PositionInfo {
    pub symbol: String,
    pub quantity: Decimal,         // 当前持仓量
    pub avg_price: Decimal,         // 平均成本价
    pub realized_pnl: Decimal,      // 已实现盈亏
    pub unrealized_pnl: Decimal,    // 未实现盈亏
    pub last_update: DateTime<Utc>,
}

/// 风控配额
#[derive(Debug, Clone)]
pub struct RiskQuota {
    // 限制参数
    pub max_position: Decimal,      // 最大仓位（手数）
    pub max_capital: Decimal,       // 最大资金（USDT）
    pub max_pending_orders: usize,  // 最大挂单数
    
    // 当前使用情况
    pub current_position: Decimal,  // 当前仓位
    pub current_capital: Decimal,   // 当前占用资金
    pub pending_orders: usize,      // 当前挂单数
    pub daily_trades: usize,        // 日内交易次数
    pub last_trade_time: Option<DateTime<Utc>>, // 最后交易时间
}

impl RiskQuota {
    pub fn new() -> Self {
        Self {
            max_position: Decimal::from(100),
            max_capital: Decimal::from(5000),
            max_pending_orders: 3,
            current_position: Decimal::ZERO,
            current_capital: Decimal::ZERO,
            pending_orders: 0,
            daily_trades: 0,
            last_trade_time: None,
        }
    }
    
    /// 检查是否可以交易
    #[inline]
    pub fn can_trade(&self, quantity: Decimal, capital: Decimal) -> bool {
        self.current_position + quantity <= self.max_position
            && self.current_capital + capital <= self.max_capital
            && self.pending_orders < self.max_pending_orders
            && self.daily_trades < 1000
    }
    
    /// 检查冷却时间
    #[inline]
    pub fn check_cooldown(&self, cooldown_seconds: i64) -> bool {
        match self.last_trade_time {
            None => true,
            Some(last_time) => {
                let elapsed = Utc::now().signed_duration_since(last_time).num_seconds();
                elapsed >= cooldown_seconds
            }
        }
    }
}

/// 共享状态 - 单线程环境，不需要Arc/Mutex
pub struct SharedState {
    pub positions: HashMap<String, PositionInfo>,     // 所有仓位
    pub risk_quotas: HashMap<String, RiskQuota>,      // 风控配额
    pub total_exposure: Decimal,                      // 总敞口
    pub max_total_exposure: Decimal,                  // 最大总敞口（0.03）
    pub warning_threshold: Decimal,                   // 预警阈值（0.025）
    pub hedge_thresholds: HashMap<String, Decimal>,   // 对冲触发阈值
    pub last_persist_time: DateTime<Utc>,            // 最后持久化时间
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            risk_quotas: HashMap::new(),
            total_exposure: Decimal::ZERO,
            max_total_exposure: Decimal::from_f64(0.03).unwrap(),
            warning_threshold: Decimal::from_f64(0.025).unwrap(),
            hedge_thresholds: HashMap::new(),
            last_persist_time: Utc::now(),
        }
    }
    
    /// 风控检查 - 检查信号是否满足风控要求
    #[inline]
    pub fn risk_check(&self, signal: &Signal) -> bool {
        // 获取该品种的风控配额，如果没有则使用默认值
        let quota = self.risk_quotas.get(&signal.symbol)
            .unwrap_or(&RiskQuota::new());
        
        // 检查配额限制
        if !quota.can_trade(signal.quantity, signal.price * signal.quantity) {
            debug!("Risk quota exceeded for {}", signal.symbol);
            return false;
        }
        
        // 检查冷却时间（60秒）
        if !quota.check_cooldown(60) {
            debug!("Cooldown period active for {}", signal.symbol);
            return false;
        }
        
        // 检查总敞口限制
        if self.total_exposure >= self.max_total_exposure {
            warn!("Total exposure limit reached: {}", self.total_exposure);
            return false;
        }
        
        // 预警提示
        if self.total_exposure >= self.warning_threshold {
            warn!("Exposure warning threshold reached: {}", self.total_exposure);
        }
        
        true
    }
    
    /// 仓位检查 - 检查是否超过单品种仓位限制
    #[inline]
    pub fn position_check(&self, symbol: &str, quantity: Decimal) -> bool {
        if let Some(position) = self.positions.get(symbol) {
            let new_quantity = position.quantity + quantity;
            new_quantity.abs() <= Decimal::from(100)  // 单品种最大100手
        } else {
            quantity.abs() <= Decimal::from(100)
        }
    }
    
    /// 更新仓位 - 根据执行报告更新仓位信息
    pub fn update_position(&mut self, report: &ExecutionReport) {
        let position = self.positions
            .entry(report.symbol.clone())
            .or_insert_with(|| PositionInfo {
                symbol: report.symbol.clone(),
                quantity: Decimal::ZERO,
                avg_price: Decimal::ZERO,
                realized_pnl: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                last_update: Utc::now(),
            });
        
        // 买入：更新均价和数量
        if report.side == common::types::Side::Buy {
            let new_quantity = position.quantity + report.executed_quantity;
            position.avg_price = (position.avg_price * position.quantity 
                + report.executed_price * report.executed_quantity) / new_quantity;
            position.quantity = new_quantity;
        } else {
            // 卖出：计算已实现盈亏
            position.quantity -= report.executed_quantity;
            let pnl = (report.executed_price - position.avg_price) * report.executed_quantity;
            position.realized_pnl += pnl;
        }
        
        position.last_update = Utc::now();
        self.calculate_total_exposure();  // 重新计算总敞口
    }
    
    /// 更新风控配额使用情况
    pub fn update_risk_quota(&mut self, report: &ExecutionReport) {
        let quota = self.risk_quotas
            .entry(report.symbol.clone())
            .or_insert_with(RiskQuota::new);
        
        quota.current_position += report.executed_quantity;
        quota.current_capital += report.executed_price * report.executed_quantity;
        quota.daily_trades += 1;
        quota.last_trade_time = Some(Utc::now());
        
        // 订单完成后减少挂单数
        if report.status == common::types::OrderStatus::Filled {
            quota.pending_orders = quota.pending_orders.saturating_sub(1);
        }
    }
    
    /// 检查是否需要触发对冲
    #[inline]
    pub fn should_trigger_hedge(&self, symbol: &str) -> bool {
        if let Some(position) = self.positions.get(symbol) {
            if let Some(threshold) = self.hedge_thresholds.get(symbol) {
                return position.quantity.abs() >= *threshold;
            }
        }
        false
    }
    
    /// 计算盈亏
    pub fn calculate_pnl(&mut self, report: &ExecutionReport) {
        if let Some(position) = self.positions.get_mut(&report.symbol) {
            let market_price = report.executed_price;
            // 未实现盈亏 = (市价 - 均价) * 持仓量
            position.unrealized_pnl = (market_price - position.avg_price) * position.quantity;
            debug!(
                "PnL for {}: realized={}, unrealized={}", 
                report.symbol, 
                position.realized_pnl, 
                position.unrealized_pnl
            );
        }
    }
    
    /// 计算总敞口
    fn calculate_total_exposure(&mut self) {
        self.total_exposure = self.positions
            .values()
            .map(|p| (p.quantity * p.avg_price).abs())
            .sum();
    }
    
    /// 持久化状态（每60秒）
    pub fn persist(&self) {
        if Utc::now().signed_duration_since(self.last_persist_time).num_seconds() > 60 {
            debug!("Persisting state to disk");
            // TODO: 实际的持久化逻辑
        }
    }
}