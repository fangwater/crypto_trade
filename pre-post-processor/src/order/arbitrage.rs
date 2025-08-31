use std::collections::HashMap;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{debug, info, warn};

use crate::order::order_state::OrderState;

/// 套利组合管理器 - 管理MT（Maker-Taker）套利订单对
pub struct ArbitrageManager {
    // 套利组合
    pairs: HashMap<String, ArbitragePair>,
    
    // 订单ID到套利ID的映射
    order_to_arbitrage: HashMap<String, String>,
    
    // 统计信息
    stats: ArbitrageStats,
}

/// 套利订单对
#[derive(Debug, Clone)]
pub struct ArbitragePair {
    pub id: String,                        // 套利组合ID
    pub maker_order_id: Option<String>,    // Maker订单ID
    pub taker_order_id: Option<String>,    // Taker订单ID
    pub symbol: String,                    // 交易对
    pub quantity: Decimal,                 // 数量
    pub maker_price: Decimal,              // Maker价格
    pub taker_price: Decimal,              // Taker价格
    pub expected_profit: Decimal,          // 预期利润
    pub actual_profit: Option<Decimal>,    // 实际利润
    pub state: ArbitrageState,             // 套利状态
    pub created_at: DateTime<Utc>,         // 创建时间
    pub completed_at: Option<DateTime<Utc>>, // 完成时间
    pub maker_status: Option<OrderState>,  // Maker订单状态
    pub taker_status: Option<OrderState>,  // Taker订单状态
}

/// 套利状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitrageState {
    Created,        // 已创建
    MakerPending,   // 等待Maker订单
    MakerFilled,    // Maker已成交
    TakerPending,   // 等待Taker订单
    BothPending,    // 两边都在等待
    Completed,      // 完成
    PartialSuccess, // 部分成功
    Failed,         // 失败
    Cancelled,      // 已取消
}

impl ArbitrageState {
    /// 是否为终态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ArbitrageState::Completed |
            ArbitrageState::PartialSuccess |
            ArbitrageState::Failed |
            ArbitrageState::Cancelled
        )
    }
    
    /// 是否为活跃状态
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

/// 套利统计
#[derive(Debug, Clone)]
struct ArbitrageStats {
    total_pairs: usize,          // 总套利组合数
    active_pairs: usize,         // 活跃组合数
    completed_pairs: usize,      // 完成组合数
    successful_pairs: usize,     // 成功组合数
    failed_pairs: usize,         // 失败组合数
    total_expected_profit: Decimal, // 总预期利润
    total_actual_profit: Decimal,   // 总实际利润
    success_rate: f64,           // 成功率
}

impl ArbitrageStats {
    fn new() -> Self {
        Self {
            total_pairs: 0,
            active_pairs: 0,
            completed_pairs: 0,
            successful_pairs: 0,
            failed_pairs: 0,
            total_expected_profit: Decimal::ZERO,
            total_actual_profit: Decimal::ZERO,
            success_rate: 0.0,
        }
    }
    
    /// 更新成功率
    fn update_success_rate(&mut self) {
        if self.completed_pairs > 0 {
            self.success_rate = (self.successful_pairs as f64) / (self.completed_pairs as f64);
        }
    }
}

impl ArbitrageManager {
    pub fn new() -> Self {
        Self {
            pairs: HashMap::new(),
            order_to_arbitrage: HashMap::new(),
            stats: ArbitrageStats::new(),
        }
    }
    
    /// 创建套利组合
    pub fn create_pair(
        &mut self,
        id: String,
        symbol: String,
        quantity: Decimal,
        maker_price: Decimal,
        taker_price: Decimal,
    ) -> ArbitragePair {
        // 计算预期利润（简化计算，不考虑手续费）
        let expected_profit = (taker_price - maker_price) * quantity;
        
        let pair = ArbitragePair {
            id: id.clone(),
            maker_order_id: None,
            taker_order_id: None,
            symbol,
            quantity,
            maker_price,
            taker_price,
            expected_profit,
            actual_profit: None,
            state: ArbitrageState::Created,
            created_at: Utc::now(),
            completed_at: None,
            maker_status: None,
            taker_status: None,
        };
        
        self.pairs.insert(id.clone(), pair.clone());
        
        // 更新统计
        self.stats.total_pairs += 1;
        self.stats.active_pairs += 1;
        self.stats.total_expected_profit += expected_profit;
        
        info!("Arbitrage pair created: {} with expected profit: {}", id, expected_profit);
        pair
    }
    
    /// 添加订单到套利组合
    pub fn add_order(&mut self, arbitrage_id: String, order_id: String) {
        self.order_to_arbitrage.insert(order_id.clone(), arbitrage_id.clone());
        
        if let Some(pair) = self.pairs.get_mut(&arbitrage_id) {
            // 根据订单类型分配到Maker或Taker
            if pair.maker_order_id.is_none() {
                pair.maker_order_id = Some(order_id.clone());
                pair.state = ArbitrageState::MakerPending;
                debug!("Added maker order {} to arbitrage {}", order_id, arbitrage_id);
            } else if pair.taker_order_id.is_none() {
                pair.taker_order_id = Some(order_id.clone());
                if pair.state == ArbitrageState::MakerPending {
                    pair.state = ArbitrageState::BothPending;
                } else {
                    pair.state = ArbitrageState::TakerPending;
                }
                debug!("Added taker order {} to arbitrage {}", order_id, arbitrage_id);
            } else {
                warn!("Arbitrage {} already has both orders", arbitrage_id);
            }
        }
    }
    
    /// 更新订单状态
    pub fn update_order_status(
        &mut self,
        arbitrage_id: &str,
        order_id: &str,
        new_status: OrderState,
    ) {
        if let Some(pair) = self.pairs.get_mut(arbitrage_id) {
            // 更新对应订单的状态
            if pair.maker_order_id.as_deref() == Some(order_id) {
                pair.maker_status = Some(new_status);
                debug!("Updated maker status to {:?} for arbitrage {}", new_status, arbitrage_id);
            } else if pair.taker_order_id.as_deref() == Some(order_id) {
                pair.taker_status = Some(new_status);
                debug!("Updated taker status to {:?} for arbitrage {}", new_status, arbitrage_id);
            }
            
            // 更新套利状态
            self.update_arbitrage_state(pair);
        }
    }
    
    /// 更新套利状态
    fn update_arbitrage_state(&mut self, pair: &mut ArbitragePair) {
        let maker_filled = matches!(pair.maker_status, Some(OrderState::Filled));
        let taker_filled = matches!(pair.taker_status, Some(OrderState::Filled));
        let maker_failed = matches!(
            pair.maker_status, 
            Some(OrderState::Cancelled) | Some(OrderState::Rejected) | Some(OrderState::Failed)
        );
        let taker_failed = matches!(
            pair.taker_status,
            Some(OrderState::Cancelled) | Some(OrderState::Rejected) | Some(OrderState::Failed)
        );
        
        let old_state = pair.state;
        
        pair.state = match (maker_filled, taker_filled, maker_failed, taker_failed) {
            // 两边都成交 - 完成
            (true, true, _, _) => {
                pair.completed_at = Some(Utc::now());
                self.calculate_actual_profit(pair);
                ArbitrageState::Completed
            }
            // Maker成交，Taker失败 - 部分成功（需要对冲）
            (true, false, _, true) => {
                pair.completed_at = Some(Utc::now());
                warn!("Arbitrage {} partial success: Maker filled but Taker failed", pair.id);
                ArbitrageState::PartialSuccess
            }
            // Taker成交，Maker失败 - 部分成功（需要对冲）
            (false, true, true, _) => {
                pair.completed_at = Some(Utc::now());
                warn!("Arbitrage {} partial success: Taker filled but Maker failed", pair.id);
                ArbitrageState::PartialSuccess
            }
            // 两边都失败
            (false, false, true, true) => {
                pair.completed_at = Some(Utc::now());
                ArbitrageState::Failed
            }
            // Maker成交，等待Taker
            (true, false, _, false) => ArbitrageState::MakerFilled,
            // 其他情况保持当前状态
            _ => pair.state,
        };
        
        // 状态变化时更新统计
        if old_state != pair.state && pair.state.is_terminal() {
            self.stats.active_pairs = self.stats.active_pairs.saturating_sub(1);
            self.stats.completed_pairs += 1;
            
            match pair.state {
                ArbitrageState::Completed => {
                    self.stats.successful_pairs += 1;
                    if let Some(profit) = pair.actual_profit {
                        self.stats.total_actual_profit += profit;
                    }
                }
                ArbitrageState::Failed => {
                    self.stats.failed_pairs += 1;
                }
                _ => {}
            }
            
            self.stats.update_success_rate();
            
            info!(
                "Arbitrage {} state changed from {:?} to {:?}", 
                pair.id, old_state, pair.state
            );
        }
    }
    
    /// 计算实际利润
    fn calculate_actual_profit(&mut self, pair: &mut ArbitragePair) {
        // 简化计算：实际利润 = (Taker成交价 - Maker成交价) * 数量 - 手续费
        // 这里暂时使用预期利润，实际应该从订单的实际成交价格计算
        pair.actual_profit = Some(pair.expected_profit * Decimal::from_f64(0.95).unwrap()); // 假设5%的滑点和手续费
    }
    
    /// 获取需要对冲的套利组合
    pub fn get_hedge_required_pairs(&self) -> Vec<&ArbitragePair> {
        self.pairs
            .values()
            .filter(|p| p.state == ArbitrageState::PartialSuccess)
            .collect()
    }
    
    /// 获取活跃的套利组合
    pub fn get_active_pairs(&self) -> Vec<&ArbitragePair> {
        self.pairs
            .values()
            .filter(|p| p.state.is_active())
            .collect()
    }
    
    /// 获取套利组合
    pub fn get_pair(&self, arbitrage_id: &str) -> Option<&ArbitragePair> {
        self.pairs.get(arbitrage_id)
    }
    
    /// 根据订单ID获取套利组合
    pub fn get_pair_by_order(&self, order_id: &str) -> Option<&ArbitragePair> {
        self.order_to_arbitrage
            .get(order_id)
            .and_then(|arb_id| self.pairs.get(arb_id))
    }
    
    /// 取消套利组合
    pub fn cancel_pair(&mut self, arbitrage_id: &str) {
        if let Some(pair) = self.pairs.get_mut(arbitrage_id) {
            if !pair.state.is_terminal() {
                pair.state = ArbitrageState::Cancelled;
                pair.completed_at = Some(Utc::now());
                
                self.stats.active_pairs = self.stats.active_pairs.saturating_sub(1);
                
                info!("Arbitrage pair {} cancelled", arbitrage_id);
            }
        }
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> ArbitrageStats {
        self.stats.clone()
    }
    
    /// 清理已完成的套利组合（定期调用）
    pub fn cleanup_completed_pairs(&mut self, keep_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(keep_hours);
        
        let to_remove: Vec<String> = self.pairs
            .iter()
            .filter(|(_, pair)| {
                pair.state.is_terminal() && 
                pair.completed_at.map_or(false, |t| t < cutoff_time)
            })
            .map(|(id, _)| id.clone())
            .collect();
        
        for id in to_remove {
            if let Some(pair) = self.pairs.remove(&id) {
                // 清理订单映射
                if let Some(ref maker_id) = pair.maker_order_id {
                    self.order_to_arbitrage.remove(maker_id);
                }
                if let Some(ref taker_id) = pair.taker_order_id {
                    self.order_to_arbitrage.remove(taker_id);
                }
                
                debug!("Cleaned up arbitrage pair {}", id);
            }
        }
    }
    
    /// 获取套利摘要
    pub fn get_summary(&self) -> String {
        format!(
            "Arbitrage Summary: Active={}, Success={}/{}, Success Rate={:.2}%, Expected Profit={}, Actual Profit={}",
            self.stats.active_pairs,
            self.stats.successful_pairs,
            self.stats.completed_pairs,
            self.stats.success_rate * 100.0,
            self.stats.total_expected_profit,
            self.stats.total_actual_profit
        )
    }
}