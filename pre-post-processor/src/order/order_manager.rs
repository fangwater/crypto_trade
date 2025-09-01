use std::collections::{HashMap, VecDeque};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use anyhow::{Result, bail};
use tracing::{debug, info, warn};

use common::types::{Signal, ExecutionReport};
use crate::order::{
    order::{Order, OrderBook, Fill},
    order_state::{OrderState, StateManager, StateTransitionEvent},
    arbitrage::ArbitrageManager,
};

/// 订单管理器 - 管理所有订单的生命周期
pub struct OrderManager {
    // 订单簿
    order_book: OrderBook,
    
    // 状态管理器
    state_manager: StateManager,
    
    // 套利管理器
    arbitrage_manager: ArbitrageManager,
    
    // 订单队列（按优先级）
    priority_queue: PriorityQueue,
    
    // 成交记录
    fills: HashMap<String, Vec<Fill>>,
    
    // 统计信息
    stats: OrderStats,
}

/// 优先级队列
struct PriorityQueue {
    queues: Vec<VecDeque<String>>, // 按优先级0-10的队列
}

impl PriorityQueue {
    fn new() -> Self {
        let mut queues = Vec::with_capacity(11);
        for _ in 0..=10 {
            queues.push(VecDeque::new());
        }
        Self { queues }
    }
    
    /// 添加订单到队列
    fn push(&mut self, order_id: String, priority: u8) {
        let priority = priority.min(10) as usize;
        self.queues[priority].push_back(order_id);
    }
    
    /// 获取最高优先级的订单
    fn pop(&mut self) -> Option<String> {
        // 从高到低遍历优先级
        for queue in self.queues.iter_mut().rev() {
            if let Some(order_id) = queue.pop_front() {
                return Some(order_id);
            }
        }
        None
    }
    
    /// 是否为空
    fn is_empty(&self) -> bool {
        self.queues.iter().all(|q| q.is_empty())
    }
    
    /// 获取队列大小
    fn len(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }
}

/// 订单统计
#[derive(Debug, Clone)]
pub struct OrderStats {
    pub total_orders: usize,           // 总订单数
    pub active_orders: usize,          // 活跃订单数
    pub filled_orders: usize,          // 成交订单数
    pub cancelled_orders: usize,       // 取消订单数
    pub rejected_orders: usize,        // 拒绝订单数
    pub total_volume: Decimal,         // 总成交量
    pub total_fees: Decimal,           // 总手续费
    pub success_rate: f64,            // 成功率
    pub avg_fill_time_ms: i64,        // 平均成交时间（毫秒）
}

impl OrderStats {
    fn new() -> Self {
        Self {
            total_orders: 0,
            active_orders: 0,
            filled_orders: 0,
            cancelled_orders: 0,
            rejected_orders: 0,
            total_volume: Decimal::ZERO,
            total_fees: Decimal::ZERO,
            success_rate: 0.0,
            avg_fill_time_ms: 0,
        }
    }
    
    /// 更新成功率
    fn update_success_rate(&mut self) {
        let completed = self.filled_orders + self.cancelled_orders + self.rejected_orders;
        if completed > 0 {
            self.success_rate = (self.filled_orders as f64) / (completed as f64);
        }
    }
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            order_book: OrderBook::new(),
            state_manager: StateManager::new(),
            arbitrage_manager: ArbitrageManager::new(),
            priority_queue: PriorityQueue::new(),
            fills: HashMap::new(),
            stats: OrderStats::new(),
        }
    }
    
    /// 创建订单（从信号）
    pub fn create_order_from_signal(&mut self, signal: Signal) -> Result<Order> {
        // 创建订单
        let mut order = Order::from_signal(&signal);
        
        // 创建状态机
        self.state_manager.create_order(order.client_order_id.clone());
        
        // 添加到订单簿
        self.order_book.add_order(order.clone());
        
        // 如果是套利订单，注册到套利管理器
        if let Some(ref arb_id) = order.arbitrage_id {
            self.arbitrage_manager.add_order(arb_id.clone(), order.client_order_id.clone());
        }
        
        // 更新统计
        self.stats.total_orders += 1;
        
        info!("Order created: {}", order.summary());
        Ok(order)
    }
    
    /// 验证订单
    pub fn validate_order(&mut self, order_id: &str) -> Result<()> {
        // 更新状态
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Validate
        )?;
        
        // 更新订单状态
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Validated;
            order.updated_at = Utc::now();
            
            // 添加到优先级队列
            self.priority_queue.push(order_id.to_string(), order.priority);
            
            debug!("Order {} validated", order_id);
        }
        
        Ok(())
    }
    
    /// 获取下一个待提交的订单
    pub fn get_next_pending_order(&mut self) -> Option<Order> {
        while let Some(order_id) = self.priority_queue.pop() {
            if let Some(order) = self.order_book.get_by_client_id(&order_id) {
                if order.state == OrderState::Validated {
                    return Some(order.clone());
                }
            }
        }
        None
    }
    
    /// 标记订单为提交中
    pub fn mark_submitting(&mut self, order_id: &str) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Submit
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Submitting;
            order.updated_at = Utc::now();
        }
        
        Ok(())
    }
    
    /// 标记订单提交成功
    pub fn mark_submitted(&mut self, order_id: &str, exchange_order_id: String) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::SubmitSuccess(exchange_order_id.clone())
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Submitted;
            order.set_exchange_order_id(exchange_order_id);
            
            self.stats.active_orders += 1;
        }
        
        Ok(())
    }
    
    /// 标记订单提交失败
    pub fn mark_submit_failed(&mut self, order_id: &str, reason: String) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::SubmitFailed(reason.clone())
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Failed;
            order.updated_at = Utc::now();
            
            // 检查是否可以重试
            if order.can_retry() {
                order.increment_retry();
                // 重新加入队列
                self.priority_queue.push(order_id.to_string(), order.priority);
                info!("Order {} will retry, attempt {}/{}", order_id, order.retry_count, order.max_retry);
            } else {
                warn!("Order {} failed and cannot retry: {}", order_id, reason);
            }
        }
        
        Ok(())
    }
    
    /// 处理执行报告
    pub fn process_execution_report(&mut self, report: ExecutionReport) -> Result<()> {
        let order = self.order_book
            .get_by_exchange_id(&report.order_id)
            .or_else(|| self.order_book.get_by_client_id(&report.order_id))
            .ok_or_else(|| anyhow::anyhow!("Order {} not found", report.order_id))?
            .clone();
        
        match report.status {
            common::types::OrderStatus::Pending => {
                self.handle_order_acknowledged(&order.client_order_id)?;
            }
            common::types::OrderStatus::PartiallyFilled => {
                self.handle_partial_fill(&order.client_order_id, &report)?;
            }
            common::types::OrderStatus::Filled => {
                self.handle_order_filled(&order.client_order_id, &report)?;
            }
            common::types::OrderStatus::Cancelled => {
                self.handle_order_cancelled(&order.client_order_id)?;
            }
            common::types::OrderStatus::Rejected => {
                self.handle_order_rejected(&order.client_order_id, "Order rejected".to_string())?;
            }
            _ => {
                debug!("Unhandled order status: {:?}", report.status);
            }
        }
        
        Ok(())
    }
    
    /// 处理订单确认
    fn handle_order_acknowledged(&mut self, order_id: &str) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Acknowledge
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Acknowledged;
            order.updated_at = Utc::now();
        }
        
        info!("Order {} acknowledged", order_id);
        Ok(())
    }
    
    /// 处理部分成交
    fn handle_partial_fill(&mut self, order_id: &str, report: &ExecutionReport) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::PartialFill(
                Decimal::from_f64(report.filled_quantity).unwrap_or(Decimal::ZERO),
                Decimal::from_f64(report.price).unwrap_or(Decimal::ZERO)
            )
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.update_execution(
                Decimal::from_f64(report.filled_quantity).unwrap_or(Decimal::ZERO),
                Decimal::from_f64(report.price).unwrap_or(Decimal::ZERO)
            );
            
            // 记录成交
            self.record_fill(order_id, report);
        }
        
        debug!("Order {} partially filled: {} @ {}", order_id, report.filled_quantity, report.price);
        Ok(())
    }
    
    /// 处理完全成交
    fn handle_order_filled(&mut self, order_id: &str, report: &ExecutionReport) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Fill
        )?;
        
        let (executed_quantity, executed_price, submitted_at, arbitrage_id) = {
            if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
                order.update_execution(
                    Decimal::from_f64(report.filled_quantity).unwrap_or(Decimal::ZERO),
                    Decimal::from_f64(report.price).unwrap_or(Decimal::ZERO)
                );
                
                // 更新统计
                self.stats.filled_orders += 1;
                self.stats.active_orders = self.stats.active_orders.saturating_sub(1);
                
                (order.executed_quantity, order.executed_price, order.submitted_at, order.arbitrage_id.clone())
            } else {
                return Ok(());
            }
        };
        
        // 记录成交
        self.record_fill(order_id, report);
        
        // 更新总成交量
        self.stats.total_volume += executed_quantity * executed_price;
        
        // 计算成交时间
        if let Some(submitted_at) = submitted_at {
            let fill_time = Utc::now()
                .signed_duration_since(submitted_at)
                .num_milliseconds();
            self.update_avg_fill_time(fill_time);
        }
        
        // 如果是套利订单，更新套利状态
        if let Some(ref arb_id) = arbitrage_id {
            self.arbitrage_manager.update_order_status(arb_id, order_id, OrderState::Filled);
        }
        
        self.stats.update_success_rate();
        info!("Order {} filled", order_id);
        Ok(())
    }
    
    /// 处理订单取消
    fn handle_order_cancelled(&mut self, order_id: &str) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::CancelConfirmed
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Cancelled;
            order.updated_at = Utc::now();
            
            self.stats.cancelled_orders += 1;
            self.stats.active_orders = self.stats.active_orders.saturating_sub(1);
        }
        
        self.stats.update_success_rate();
        info!("Order {} cancelled", order_id);
        Ok(())
    }
    
    /// 处理订单拒绝
    fn handle_order_rejected(&mut self, order_id: &str, reason: String) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Reject(reason.clone())
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Rejected;
            order.updated_at = Utc::now();
            
            self.stats.rejected_orders += 1;
            self.stats.active_orders = self.stats.active_orders.saturating_sub(1);
        }
        
        self.stats.update_success_rate();
        warn!("Order {} rejected: {}", order_id, reason);
        Ok(())
    }
    
    /// 处理订单过期
    fn handle_order_expired(&mut self, order_id: &str) -> Result<()> {
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Expire
        )?;
        
        if let Some(order) = self.order_book.orders_by_client_id.get_mut(order_id) {
            order.state = OrderState::Expired;
            order.updated_at = Utc::now();
            
            self.stats.active_orders = self.stats.active_orders.saturating_sub(1);
        }
        
        info!("Order {} expired", order_id);
        Ok(())
    }
    
    /// 记录成交
    fn record_fill(&mut self, order_id: &str, report: &ExecutionReport) {
        let fill = Fill {
            order_id: order_id.to_string(),
            trade_id: format!("TRD_{}", uuid::Uuid::new_v4()), // 生成交易ID
            symbol: format!("{:?}", report.symbol),  // 使用 Debug trait 转换 Symbol
            side: report.side,
            price: Decimal::from_f64(report.price).unwrap_or(Decimal::ZERO),
            quantity: Decimal::from_f64(report.filled_quantity).unwrap_or(Decimal::ZERO),
            fee: Decimal::ZERO, // 默认手续费为0
            fee_currency: "USDT".to_string(),
            timestamp: Utc::now(),
        };
        
        self.fills
            .entry(order_id.to_string())
            .or_insert_with(Vec::new)
            .push(fill.clone());
        
        self.stats.total_fees += fill.fee;
    }
    
    /// 更新平均成交时间
    fn update_avg_fill_time(&mut self, new_time_ms: i64) {
        let n = self.stats.filled_orders as i64;
        if n > 0 {
            self.stats.avg_fill_time_ms = 
                (self.stats.avg_fill_time_ms * (n - 1) + new_time_ms) / n;
        }
    }
    
    /// 取消订单
    pub fn cancel_order(&mut self, order_id: &str) -> Result<()> {
        let order = self.order_book
            .get_by_client_id(order_id)
            .ok_or_else(|| anyhow::anyhow!("Order {} not found", order_id))?;
        
        if !order.state.can_cancel() {
            bail!("Order {} cannot be cancelled in state {:?}", order_id, order.state);
        }
        
        self.state_manager.transition_order(
            order_id,
            StateTransitionEvent::Cancel
        )?;
        
        info!("Cancel request sent for order {}", order_id);
        Ok(())
    }
    
    /// 获取订单状态
    pub fn get_order_status(&self, order_id: &str) -> Option<OrderState> {
        self.order_book
            .get_by_client_id(order_id)
            .map(|o| o.state)
    }
    
    /// 获取订单
    pub fn get_order(&self, order_id: &str) -> Option<&Order> {
        self.order_book.get_by_client_id(order_id)
    }
    
    /// 获取活跃订单
    pub fn get_active_orders(&self) -> Vec<&Order> {
        self.order_book.get_active_orders()
    }
    
    /// 获取订单成交记录
    pub fn get_fills(&self, order_id: &str) -> Option<&Vec<Fill>> {
        self.fills.get(order_id)
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> OrderStats {
        self.stats.clone()
    }
    
    /// 清理已完成订单（定期调用）
    pub fn cleanup_completed_orders(&mut self, keep_hours: i64) {
        let cutoff_time = Utc::now() - chrono::Duration::hours(keep_hours);
        
        let mut to_remove = Vec::new();
        for (order_id, order) in &self.order_book.orders_by_client_id {
            if order.is_completed() && order.updated_at < cutoff_time {
                to_remove.push(order_id.clone());
            }
        }
        
        for order_id in to_remove {
            self.order_book.orders_by_client_id.remove(&order_id);
            self.fills.remove(&order_id);
            debug!("Cleaned up order {}", order_id);
        }
        
        // 清理状态管理器中的终态订单
        self.state_manager.cleanup_terminal_orders(false);
    }
}