use anyhow::Result;
use std::rc::Rc;
use std::cell::RefCell;
use tracing::{debug, instrument};

use crate::pipeline::shared_state::SharedState;
use common::types::{ExecutionReport, Order, Signal};

pub trait Pipeline<T> {
    fn pipe<U, F>(self, f: F) -> U
    where
        F: FnOnce(T) -> U;
        
    fn pipe_ref<U, F>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
        T: Clone;
        
    fn pipe_mut<F>(self, f: F) -> T
    where
        F: FnOnce(&mut T),
        T: Sized;
}

impl<T> Pipeline<T> for T {
    #[inline(always)]
    fn pipe<U, F>(self, f: F) -> U
    where
        F: FnOnce(T) -> U,
    {
        f(self)
    }
    
    #[inline(always)]
    fn pipe_ref<U, F>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
        T: Clone,
    {
        f(self)
    }
    
    #[inline(always)]
    fn pipe_mut<F>(mut self, f: F) -> T
    where
        F: FnOnce(&mut T),
        T: Sized,
    {
        f(&mut self);
        self
    }
}

#[derive(Debug, Clone)]
pub struct PreProcessContext {
    pub signal: Signal,
    pub shared_state: Rc<RefCell<SharedState>>,
    pub should_continue: bool,
    pub priority: u8,
    pub order: Option<Order>,
}

impl PreProcessContext {
    pub fn new(signal: Signal, shared_state: Rc<RefCell<SharedState>>) -> Self {
        Self {
            signal,
            shared_state,
            should_continue: true,
            priority: 5,
            order: None,
        }
    }
    
    #[inline(always)]
    pub fn stop(mut self) -> Self {
        self.should_continue = false;
        self
    }
    
    #[inline(always)]
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
    
    #[inline(always)]
    pub fn with_order(mut self, order: Order) -> Self {
        self.order = Some(order);
        self
    }
}

#[derive(Debug, Clone)]
pub struct PostProcessContext {
    pub report: ExecutionReport,
    pub shared_state: Rc<RefCell<SharedState>>,
    pub should_continue: bool,
}

impl PostProcessContext {
    pub fn new(report: ExecutionReport, shared_state: Rc<RefCell<SharedState>>) -> Self {
        Self {
            report,
            shared_state,
            should_continue: true,
        }
    }
    
    #[inline(always)]
    pub fn stop(mut self) -> Self {
        self.should_continue = false;
        self
    }
}

pub type PreProcessResult = Result<Option<Order>>;
pub type PostProcessResult = Result<()>;

#[instrument(skip_all, fields(signal_id = %ctx.signal.id))]
pub async fn execute_pre_pipeline(ctx: PreProcessContext) -> PreProcessResult {
    debug!("Starting pre-process pipeline");
    
    let result = ctx
        .pipe(check_signal_age)
        .pipe(check_risk_control)
        .pipe(check_position_limit)
        .pipe(construct_order)
        .pipe(assign_priority);
    
    if result.should_continue && result.order.is_some() {
        Ok(result.order)
    } else {
        Ok(None)
    }
}

#[instrument(skip_all, fields(order_id = %ctx.report.order_id))]
pub async fn execute_post_pipeline(ctx: PostProcessContext) -> PostProcessResult {
    debug!("Starting post-process pipeline");
    
    let _ = ctx
        .pipe(update_position)
        .pipe(update_risk_quota)
        .pipe(check_hedge_trigger)
        .pipe(calculate_pnl)
        .pipe(persist_state);
    
    Ok(())
}

#[inline(always)]
fn check_signal_age(mut ctx: PreProcessContext) -> PreProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let age_ms = chrono::Utc::now()
        .signed_duration_since(ctx.signal.timestamp)
        .num_milliseconds();
    
    if age_ms > 100 {
        debug!("Signal too old: {}ms", age_ms);
        ctx.should_continue = false;
    }
    
    ctx
}

#[inline(always)]
fn check_risk_control(ctx: PreProcessContext) -> PreProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let state = ctx.shared_state.borrow();
    if !state.risk_check(&ctx.signal) {
        debug!("Risk control check failed");
        return ctx.stop();
    }
    
    ctx
}

#[inline(always)]
fn check_position_limit(ctx: PreProcessContext) -> PreProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let state = ctx.shared_state.borrow();
    if !state.position_check(&ctx.signal.symbol, ctx.signal.quantity) {
        debug!("Position limit check failed");
        return ctx.stop();
    }
    
    ctx
}

#[inline(always)]
fn construct_order(mut ctx: PreProcessContext) -> PreProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let order = Order::from_signal(&ctx.signal);
    ctx.order = Some(order);
    ctx
}

#[inline(always)]
fn assign_priority(mut ctx: PreProcessContext) -> PreProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let priority = match ctx.signal.signal_type {
        common::types::SignalType::Arbitrage => 10,
        common::types::SignalType::Market => 5,
        common::types::SignalType::Hedge => 8,
    };
    
    ctx.priority = priority;
    if let Some(ref mut order) = ctx.order {
        order.priority = priority;
    }
    
    ctx
}

#[inline(always)]
fn update_position(ctx: PostProcessContext) -> PostProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let mut state = ctx.shared_state.borrow_mut();
    state.update_position(&ctx.report);
    ctx
}

#[inline(always)]
fn update_risk_quota(ctx: PostProcessContext) -> PostProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let mut state = ctx.shared_state.borrow_mut();
    state.update_risk_quota(&ctx.report);
    ctx
}

#[inline(always)]
fn check_hedge_trigger(ctx: PostProcessContext) -> PostProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let state = ctx.shared_state.borrow();
    if state.should_trigger_hedge(&ctx.report.symbol) {
        debug!("Hedge trigger detected for {}", ctx.report.symbol);
    }
    
    ctx
}

#[inline(always)]
fn calculate_pnl(ctx: PostProcessContext) -> PostProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let mut state = ctx.shared_state.borrow_mut();
    state.calculate_pnl(&ctx.report);
    ctx
}

#[inline(always)]
fn persist_state(ctx: PostProcessContext) -> PostProcessContext {
    if !ctx.should_continue {
        return ctx;
    }
    
    let state = ctx.shared_state.borrow();
    state.persist();
    ctx
}