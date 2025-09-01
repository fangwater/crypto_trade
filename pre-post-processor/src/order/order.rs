use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::types::{Signal, Side, OrderType, TimeInForce, SignalType};
use crate::order::order_state::OrderState;

/// 订单结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    // 订单标识
    pub client_order_id: String,     // 客户端订单ID（幂等）
    pub exchange_order_id: Option<String>, // 交易所订单ID
    pub signal_id: String,            // 源信号ID
    
    // 订单基本信息
    pub symbol: String,               // 交易对
    pub side: Side,                   // 买卖方向
    pub order_type: OrderType,        // 订单类型
    pub time_in_force: TimeInForce,  // 有效期类型
    
    // 价格和数量
    pub price: Decimal,               // 订单价格
    pub quantity: Decimal,            // 订单数量
    pub executed_quantity: Decimal,   // 已执行数量
    pub executed_price: Decimal,      // 平均执行价格
    pub remaining_quantity: Decimal,  // 剩余数量
    
    // 状态和时间
    pub state: OrderState,            // 订单状态
    pub created_at: DateTime<Utc>,    // 创建时间
    pub updated_at: DateTime<Utc>,    // 更新时间
    pub submitted_at: Option<DateTime<Utc>>, // 提交时间
    pub filled_at: Option<DateTime<Utc>>,    // 完全成交时间
    
    // 风控和优先级
    pub priority: u8,                 // 优先级（0-10，10最高）
    pub max_retry: u8,                // 最大重试次数
    pub retry_count: u8,              // 当前重试次数
    
    // 套利和对冲标记
    pub arbitrage_id: Option<String>, // 套利组合ID
    pub hedge_order_id: Option<String>, // 对冲订单ID
    pub is_hedge: bool,               // 是否是对冲订单
    
    // 元数据
    pub metadata: OrderMetadata,      // 订单元数据
}

/// 订单元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderMetadata {
    pub strategy: String,             // 策略名称
    pub exchange: String,             // 交易所
    pub account: String,              // 账户
    pub tags: Vec<String>,            // 标签
    pub notes: Option<String>,        // 备注
}

impl Order {
    /// 从信号创建订单
    pub fn from_signal(signal: &Signal) -> Self {
        let client_order_id = Self::generate_client_order_id(&signal.id);
        
        Self {
            client_order_id: client_order_id.clone(),
            exchange_order_id: None,
            signal_id: signal.id.clone(),
            symbol: signal.symbol.clone(),
            side: signal.side.unwrap_or(Side::Buy), // 默认为买入
            order_type: OrderType::Market, // 默认为市价单
            time_in_force: TimeInForce::IOC, // 默认为立即执行或取消
            price: signal.price
                .and_then(|p| Decimal::from_f64(p))
                .unwrap_or(Decimal::ZERO),
            quantity: signal.quantity
                .and_then(|q| Decimal::from_f64(q))
                .unwrap_or(Decimal::ZERO),
            executed_quantity: Decimal::ZERO,
            executed_price: Decimal::ZERO,
            remaining_quantity: signal.quantity
                .and_then(|q| Decimal::from_f64(q))
                .unwrap_or(Decimal::ZERO),
            state: OrderState::Created,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            submitted_at: None,
            filled_at: None,
            priority: 5,
            max_retry: 3,
            retry_count: 0,
            arbitrage_id: signal.metadata.get("arbitrage_id").cloned(),
            hedge_order_id: None,
            is_hedge: signal.signal_type == SignalType::Hedge,
            metadata: OrderMetadata {
                strategy: signal.source.clone(),
                exchange: signal.exchange.clone(),
                account: signal.metadata.get("account")
                    .cloned()
                    .unwrap_or_else(|| "default".to_string()),
                tags: Vec::new(),
                notes: None,
            },
        }
    }
    
    /// 生成客户端订单ID（保证幂等性）
    fn generate_client_order_id(signal_id: &str) -> String {
        // 使用信号ID的哈希来生成确定性的订单ID
        let uuid = Uuid::new_v4();
        format!("ORD_{}", uuid.simple())
    }
    
    /// 是否可以重试
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retry && 
        matches!(self.state, OrderState::Rejected | OrderState::Failed)
    }
    
    /// 增加重试次数
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.updated_at = Utc::now();
    }
    
    /// 是否已完成
    pub fn is_completed(&self) -> bool {
        matches!(
            self.state, 
            OrderState::Filled | OrderState::Cancelled | OrderState::Expired
        )
    }
    
    /// 是否活跃
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            OrderState::Submitting | OrderState::Submitted | 
            OrderState::Acknowledged | OrderState::PartiallyFilled
        )
    }
    
    /// 更新执行信息
    pub fn update_execution(&mut self, executed_qty: Decimal, executed_price: Decimal) {
        // 更新平均执行价格
        let total_executed = self.executed_quantity + executed_qty;
        if total_executed > Decimal::ZERO {
            self.executed_price = (self.executed_price * self.executed_quantity 
                + executed_price * executed_qty) / total_executed;
        }
        
        self.executed_quantity = total_executed;
        self.remaining_quantity = self.quantity - total_executed;
        self.updated_at = Utc::now();
        
        // 更新状态
        if self.remaining_quantity <= Decimal::ZERO {
            self.state = OrderState::Filled;
            self.filled_at = Some(Utc::now());
        } else if self.executed_quantity > Decimal::ZERO {
            self.state = OrderState::PartiallyFilled;
        }
    }
    
    /// 设置交易所订单ID
    pub fn set_exchange_order_id(&mut self, exchange_id: String) {
        self.exchange_order_id = Some(exchange_id);
        self.submitted_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
    
    /// 计算订单价值
    pub fn calculate_value(&self) -> Decimal {
        self.price * self.quantity
    }
    
    /// 计算已执行价值
    pub fn calculate_executed_value(&self) -> Decimal {
        self.executed_price * self.executed_quantity
    }
    
    /// 获取订单摘要
    pub fn summary(&self) -> String {
        format!(
            "Order {} {:?} {} {} @ {} (state: {:?})",
            self.client_order_id,
            self.side,
            self.quantity,
            self.symbol,
            self.price,
            self.state
        )
    }
}

/// 订单填充信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub order_id: String,             // 订单ID
    pub trade_id: String,             // 成交ID
    pub symbol: String,               // 交易对
    pub side: Side,                   // 方向
    pub price: Decimal,               // 成交价格
    pub quantity: Decimal,            // 成交数量
    pub fee: Decimal,                 // 手续费
    pub fee_currency: String,         // 手续费币种
    pub timestamp: DateTime<Utc>,     // 成交时间
}

impl Fill {
    /// 计算成交金额
    pub fn calculate_amount(&self) -> Decimal {
        self.price * self.quantity
    }
    
    /// 计算净金额（扣除手续费）
    pub fn calculate_net_amount(&self) -> Decimal {
        let amount = self.calculate_amount();
        if self.fee_currency == "USDT" {
            amount - self.fee
        } else {
            amount  // 其他币种的手续费暂不计算
        }
    }
}

/// 订单簿
#[derive(Debug, Clone)]
pub struct OrderBook {
    // 按client_order_id索引
    pub orders_by_client_id: HashMap<String, Order>,
    
    // 按exchange_order_id索引
    pub orders_by_exchange_id: HashMap<String, String>, // exchange_id -> client_id
    
    // 按symbol索引
    pub orders_by_symbol: HashMap<String, Vec<String>>, // symbol -> client_ids
    
    // 活跃订单
    pub active_orders: Vec<String>, // client_ids
    
    // 待提交订单队列
    pub pending_orders: Vec<String>, // client_ids
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            orders_by_client_id: HashMap::new(),
            orders_by_exchange_id: HashMap::new(),
            orders_by_symbol: HashMap::new(),
            active_orders: Vec::new(),
            pending_orders: Vec::new(),
        }
    }
    
    /// 添加订单
    pub fn add_order(&mut self, order: Order) {
        let client_id = order.client_order_id.clone();
        let symbol = order.symbol.clone();
        
        // 添加到主索引
        self.orders_by_client_id.insert(client_id.clone(), order.clone());
        
        // 添加到symbol索引
        self.orders_by_symbol
            .entry(symbol)
            .or_insert_with(Vec::new)
            .push(client_id.clone());
        
        // 如果有交易所ID，添加到exchange索引
        if let Some(ref exchange_id) = order.exchange_order_id {
            self.orders_by_exchange_id.insert(exchange_id.clone(), client_id.clone());
        }
        
        // 根据状态添加到相应列表
        if order.state == OrderState::Created || order.state == OrderState::Validated {
            self.pending_orders.push(client_id);
        } else if order.is_active() {
            self.active_orders.push(client_id);
        }
    }
    
    /// 更新订单
    pub fn update_order(&mut self, order: Order) {
        let client_id = order.client_order_id.clone();
        
        // 更新主索引
        self.orders_by_client_id.insert(client_id.clone(), order.clone());
        
        // 更新exchange索引
        if let Some(ref exchange_id) = order.exchange_order_id {
            self.orders_by_exchange_id.insert(exchange_id.clone(), client_id.clone());
        }
        
        // 更新活跃列表
        if order.is_active() && !self.active_orders.contains(&client_id) {
            self.active_orders.push(client_id.clone());
            self.pending_orders.retain(|id| id != &client_id);
        } else if order.is_completed() {
            self.active_orders.retain(|id| id != &client_id);
            self.pending_orders.retain(|id| id != &client_id);
        }
    }
    
    /// 根据客户端ID获取订单
    pub fn get_by_client_id(&self, client_id: &str) -> Option<&Order> {
        self.orders_by_client_id.get(client_id)
    }
    
    /// 根据交易所ID获取订单
    pub fn get_by_exchange_id(&self, exchange_id: &str) -> Option<&Order> {
        self.orders_by_exchange_id
            .get(exchange_id)
            .and_then(|client_id| self.orders_by_client_id.get(client_id))
    }
    
    /// 获取某个品种的所有订单
    pub fn get_by_symbol(&self, symbol: &str) -> Vec<&Order> {
        self.orders_by_symbol
            .get(symbol)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.orders_by_client_id.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// 获取所有活跃订单
    pub fn get_active_orders(&self) -> Vec<&Order> {
        self.active_orders
            .iter()
            .filter_map(|id| self.orders_by_client_id.get(id))
            .collect()
    }
    
    /// 获取待提交订单
    pub fn get_pending_orders(&self) -> Vec<&Order> {
        self.pending_orders
            .iter()
            .filter_map(|id| self.orders_by_client_id.get(id))
            .collect()
    }
}

use std::collections::HashMap;