use serde::{Deserialize, Serialize};
use std::fmt;
use anyhow::{Result, bail};
use tracing::{debug, warn};

/// 订单状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderState {
    // 初始状态
    Created,         // 已创建
    Validated,       // 已验证（通过风控）
    
    // 提交中
    Submitting,      // 提交中
    Submitted,       // 已提交到交易所
    
    // 活跃状态
    Acknowledged,    // 已被交易所确认
    PartiallyFilled, // 部分成交
    
    // 终态
    Filled,          // 完全成交
    Cancelled,       // 已取消
    Rejected,        // 被拒绝
    Expired,         // 已过期
    Failed,          // 失败（系统错误）
}

impl fmt::Display for OrderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderState::Created => write!(f, "Created"),
            OrderState::Validated => write!(f, "Validated"),
            OrderState::Submitting => write!(f, "Submitting"),
            OrderState::Submitted => write!(f, "Submitted"),
            OrderState::Acknowledged => write!(f, "Acknowledged"),
            OrderState::PartiallyFilled => write!(f, "PartiallyFilled"),
            OrderState::Filled => write!(f, "Filled"),
            OrderState::Cancelled => write!(f, "Cancelled"),
            OrderState::Rejected => write!(f, "Rejected"),
            OrderState::Expired => write!(f, "Expired"),
            OrderState::Failed => write!(f, "Failed"),
        }
    }
}

impl OrderState {
    /// 是否为终态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderState::Filled | 
            OrderState::Cancelled | 
            OrderState::Rejected | 
            OrderState::Expired | 
            OrderState::Failed
        )
    }
    
    /// 是否为活跃状态
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            OrderState::Submitting | 
            OrderState::Submitted | 
            OrderState::Acknowledged | 
            OrderState::PartiallyFilled
        )
    }
    
    /// 是否可以取消
    pub fn can_cancel(&self) -> bool {
        matches!(
            self,
            OrderState::Created | 
            OrderState::Validated | 
            OrderState::Submitting | 
            OrderState::Submitted | 
            OrderState::Acknowledged | 
            OrderState::PartiallyFilled
        )
    }
    
    /// 是否可以重试
    pub fn can_retry(&self) -> bool {
        matches!(
            self,
            OrderState::Rejected | 
            OrderState::Failed
        )
    }
}

/// 状态转换事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateTransitionEvent {
    // 验证和提交
    Validate,                    // 验证通过
    Submit,                      // 开始提交
    SubmitSuccess(String),       // 提交成功，返回exchange_order_id
    SubmitFailed(String),        // 提交失败，返回错误信息
    
    // 交易所确认
    Acknowledge,                 // 交易所确认
    Reject(String),              // 交易所拒绝，返回原因
    
    // 成交
    PartialFill(rust_decimal::Decimal, rust_decimal::Decimal), // 部分成交(数量, 价格)
    Fill,                        // 完全成交
    
    // 取消和过期
    Cancel,                      // 取消请求
    CancelConfirmed,             // 取消确认
    Expire,                      // 订单过期
    
    // 系统错误
    SystemError(String),         // 系统错误
}

/// 订单状态机
pub struct OrderStateMachine {
    current_state: OrderState,
}

impl OrderStateMachine {
    /// 创建新的状态机
    pub fn new() -> Self {
        Self {
            current_state: OrderState::Created,
        }
    }
    
    /// 获取当前状态
    pub fn current_state(&self) -> OrderState {
        self.current_state
    }
    
    /// 处理状态转换
    pub fn transition(&mut self, event: StateTransitionEvent) -> Result<OrderState> {
        let new_state = self.get_next_state(event)?;
        
        debug!(
            "Order state transition: {:?} -> {:?}", 
            self.current_state, 
            new_state
        );
        
        self.current_state = new_state;
        Ok(new_state)
    }
    
    /// 根据事件获取下一个状态
    fn get_next_state(&self, event: StateTransitionEvent) -> Result<OrderState> {
        use OrderState::*;
        use StateTransitionEvent::*;
        
        let next_state = match (self.current_state, event) {
            // Created -> Validated
            (Created, Validate) => Validated,
            
            // Validated -> Submitting
            (Validated, Submit) => Submitting,
            
            // Submitting -> Submitted/Failed
            (Submitting, SubmitSuccess(_)) => Submitted,
            (Submitting, SubmitFailed(reason)) => {
                warn!("Order submit failed: {}", reason);
                Failed
            }
            
            // Submitted -> Acknowledged/Rejected
            (Submitted, Acknowledge) => Acknowledged,
            (Submitted, Reject(reason)) => {
                warn!("Order rejected: {}", reason);
                Rejected
            }
            
            // Acknowledged -> PartiallyFilled/Filled
            (Acknowledged, PartialFill(_, _)) => PartiallyFilled,
            (Acknowledged, Fill) => Filled,
            
            // PartiallyFilled -> PartiallyFilled/Filled
            (PartiallyFilled, PartialFill(_, _)) => PartiallyFilled,
            (PartiallyFilled, Fill) => Filled,
            
            // 取消操作
            (state, Cancel) if state.can_cancel() => Cancelled,
            (Cancelled, CancelConfirmed) => Cancelled,
            
            // 过期
            (state, Expire) if state.is_active() => Expired,
            
            // 系统错误
            (state, SystemError(reason)) if !state.is_terminal() => {
                warn!("Order system error: {}", reason);
                Failed
            }
            
            // 无效转换
            (current, event) => {
                bail!(
                    "Invalid state transition: {:?} with event {:?}", 
                    current, 
                    event
                );
            }
        };
        
        Ok(next_state)
    }
    
    /// 验证状态转换是否合法
    pub fn can_transition(&self, event: &StateTransitionEvent) -> bool {
        self.get_next_state(event.clone()).is_ok()
    }
    
    /// 重置状态机
    pub fn reset(&mut self) {
        self.current_state = OrderState::Created;
    }
}

/// 状态转换历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionHistory {
    pub from_state: OrderState,
    pub to_state: OrderState,
    pub event: StateTransitionEvent,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: Option<String>,
}

/// 状态机管理器 - 跟踪多个订单的状态
pub struct StateManager {
    // 订单ID -> 状态机
    machines: HashMap<String, OrderStateMachine>,
    
    // 订单ID -> 状态历史
    histories: HashMap<String, Vec<StateTransitionHistory>>,
}

use std::collections::HashMap;

impl StateManager {
    pub fn new() -> Self {
        Self {
            machines: HashMap::new(),
            histories: HashMap::new(),
        }
    }
    
    /// 创建新订单的状态机
    pub fn create_order(&mut self, order_id: String) {
        self.machines.insert(order_id.clone(), OrderStateMachine::new());
        self.histories.insert(order_id, Vec::new());
    }
    
    /// 处理订单状态转换
    pub fn transition_order(
        &mut self, 
        order_id: &str, 
        event: StateTransitionEvent
    ) -> Result<OrderState> {
        let machine = self.machines
            .get_mut(order_id)
            .ok_or_else(|| anyhow::anyhow!("Order {} not found", order_id))?;
        
        let from_state = machine.current_state();
        let to_state = machine.transition(event.clone())?;
        
        // 记录历史
        if let Some(history) = self.histories.get_mut(order_id) {
            history.push(StateTransitionHistory {
                from_state,
                to_state,
                event,
                timestamp: chrono::Utc::now(),
                details: None,
            });
        }
        
        Ok(to_state)
    }
    
    /// 获取订单当前状态
    pub fn get_state(&self, order_id: &str) -> Option<OrderState> {
        self.machines.get(order_id).map(|m| m.current_state())
    }
    
    /// 获取订单状态历史
    pub fn get_history(&self, order_id: &str) -> Option<&Vec<StateTransitionHistory>> {
        self.histories.get(order_id)
    }
    
    /// 批量获取活跃订单
    pub fn get_active_orders(&self) -> Vec<String> {
        self.machines
            .iter()
            .filter(|(_, machine)| machine.current_state().is_active())
            .map(|(id, _)| id.clone())
            .collect()
    }
    
    /// 批量获取终态订单
    pub fn get_terminal_orders(&self) -> Vec<String> {
        self.machines
            .iter()
            .filter(|(_, machine)| machine.current_state().is_terminal())
            .map(|(id, _)| id.clone())
            .collect()
    }
    
    /// 清理终态订单（可选）
    pub fn cleanup_terminal_orders(&mut self, keep_history: bool) {
        let terminal_orders = self.get_terminal_orders();
        
        for order_id in terminal_orders {
            self.machines.remove(&order_id);
            if !keep_history {
                self.histories.remove(&order_id);
            }
        }
    }
}