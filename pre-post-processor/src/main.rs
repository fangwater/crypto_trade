mod pipeline;
mod risk_control;
mod order;

use std::rc::Rc;
use std::cell::RefCell;
use tokio::sync::mpsc;
use tokio::select;
use tokio::time::{interval, Duration};
use tracing::{info, error, debug};
use anyhow::Result;

use iceoryx2::prelude::*;
use iceoryx2::port::subscriber::Subscriber;
use common::types::{Signal, ExecutionReport};
use common::ipc::{IPC_SERVICE_SIGNAL, IPC_SERVICE_EXECUTION};

use crate::pipeline::{
    pipeline::{PreProcessContext, PostProcessContext, execute_pre_pipeline, execute_post_pipeline},
    shared_state::SharedState,
};
use crate::risk_control::{
    risk_state::RiskState,
    risk_initializer::RiskInitializer,
};
use crate::order::order_manager::OrderManager;

/// Pre/Post Processor 主进程
pub struct PrePostProcessor {
    // 共享状态（单线程，使用Rc<RefCell>）
    shared_state: Rc<RefCell<SharedState>>,
    
    // 风控状态
    risk_state: RiskState,
    
    // 风控初始化器
    risk_initializer: RiskInitializer,
    
    // 订单管理器
    order_manager: OrderManager,
    
    // 内部队列
    pre_queue_rx: mpsc::UnboundedReceiver<Signal>,
    pre_queue_tx: mpsc::UnboundedSender<Signal>,
    post_queue_rx: mpsc::UnboundedReceiver<ExecutionReport>,
    post_queue_tx: mpsc::UnboundedSender<ExecutionReport>,
    
    // 统计信息
    processed_signals: usize,
    processed_reports: usize,
}

impl PrePostProcessor {
    pub fn new() -> Self {
        let (pre_tx, pre_rx) = mpsc::unbounded_channel();
        let (post_tx, post_rx) = mpsc::unbounded_channel();
        
        Self {
            shared_state: Rc::new(RefCell::new(SharedState::new())),
            risk_state: RiskState::new(),
            risk_initializer: RiskInitializer::new(),
            order_manager: OrderManager::new(),
            pre_queue_rx: pre_rx,
            pre_queue_tx: pre_tx,
            post_queue_rx: post_rx,
            post_queue_tx: post_tx,
            processed_signals: 0,
            processed_reports: 0,
        }
    }
    
    /// 启动处理器
    pub async fn run(mut self) -> Result<()> {
        info!("Starting Pre/Post Processor");
        
        // 初始化IceOryx2订阅
        let signal_subscriber = self.setup_signal_subscriber()?;
        let execution_subscriber = self.setup_execution_subscriber()?;
        
        // 创建定时器
        let mut stats_timer = interval(Duration::from_secs(60));
        let mut cleanup_timer = interval(Duration::from_secs(3600)); // 每小时清理
        
        loop {
            select! {
                // 处理信号订阅
                _ = Self::poll_signals(&signal_subscriber, &self.pre_queue_tx) => {
                    // 信号已放入队列
                }
                
                // 处理执行报告订阅
                _ = Self::poll_executions(&execution_subscriber, &self.post_queue_tx) => {
                    // 执行报告已放入队列
                }
                
                // 处理Pre-process队列
                Some(signal) = self.pre_queue_rx.recv() => {
                    self.process_signal(signal).await?;
                }
                
                // 处理Post-process队列
                Some(report) = self.post_queue_rx.recv() => {
                    self.process_execution_report(report).await?;
                }
                
                // 定时输出统计
                _ = stats_timer.tick() => {
                    self.print_statistics();
                }
                
                // 定时清理
                _ = cleanup_timer.tick() => {
                    self.cleanup();
                }
            }
        }
    }
    
    /// 设置信号订阅
    fn setup_signal_subscriber(&self) -> Result<Subscriber<iceoryx2::service::ipc::Service, Signal, ()>> {
        let node = NodeBuilder::new().create::<iceoryx2::service::ipc::Service>()?;
        
        let service = node
            .service_builder(&iceoryx2::prelude::ServiceName::new(IPC_SERVICE_SIGNAL)?)
            .publish_subscribe::<Signal>()
            .open_or_create()?;
        
        let subscriber = service
            .subscriber_builder()
            .create()?;
        
        info!("Signal subscriber created");
        Ok(subscriber)
    }
    
    /// 设置执行报告订阅
    fn setup_execution_subscriber(&self) -> Result<Subscriber<iceoryx2::service::ipc::Service, ExecutionReport, ()>> {
        let node = NodeBuilder::new().create::<iceoryx2::service::ipc::Service>()?;
        
        let service = node
            .service_builder(&iceoryx2::prelude::ServiceName::new(IPC_SERVICE_EXECUTION)?)
            .publish_subscribe::<ExecutionReport>()
            .open_or_create()?;
        
        let subscriber = service
            .subscriber_builder()
            .create()?;
        
        info!("Execution report subscriber created");
        Ok(subscriber)
    }
    
    /// 轮询信号
    async fn poll_signals(
        subscriber: &Subscriber<iceoryx2::service::ipc::Service, Signal, ()>,
        tx: &mpsc::UnboundedSender<Signal>
    ) {
        while let Some(sample) = subscriber.receive().unwrap() {
            let signal = sample.payload().clone();
            debug!("Received signal: {:?}", signal.id);
            
            if let Err(e) = tx.send(signal) {
                error!("Failed to queue signal: {:?}", e);
            }
        }
    }
    
    /// 轮询执行报告
    async fn poll_executions(
        subscriber: &Subscriber<iceoryx2::service::ipc::Service, ExecutionReport, ()>,
        tx: &mpsc::UnboundedSender<ExecutionReport>
    ) {
        while let Some(sample) = subscriber.receive().unwrap() {
            let report = sample.payload().clone();
            debug!("Received execution report: {:?}", report.order_id);
            
            if let Err(e) = tx.send(report) {
                error!("Failed to queue execution report: {:?}", e);
            }
        }
    }
    
    /// 处理信号（Pre-process Pipeline）
    async fn process_signal(&mut self, signal: Signal) -> Result<()> {
        debug!("Processing signal: {}", signal.id);
        
        // 检查是否为风控初始化消息
        if signal.signal_type == common::types::SignalType::RiskControlInit {
            info!("Processing risk control initialization signal");
            
            match self.risk_initializer.process_init_signal(&signal) {
                Ok(response) => {
                    if response.success {
                        // 获取初始化后的风控状态
                        self.risk_state = self.risk_initializer.get_risk_state().clone();
                        info!("Risk control initialized: {}", response.message);
                        
                        // 更新共享状态中的风控信息
                        self.shared_state.borrow_mut().update_risk_state(self.risk_state.get_summary());
                    } else {
                        error!("Risk control initialization failed: {}", response.message);
                    }
                }
                Err(e) => {
                    error!("Failed to process risk control init signal: {}", e);
                }
            }
            
            self.processed_signals += 1;
            return Ok(());
        }
        
        // 创建Pipeline上下文
        let ctx = PreProcessContext::new(signal.clone(), self.shared_state.clone());
        
        // 执行Pre-process Pipeline（链式调用）
        match execute_pre_pipeline(ctx).await {
            Ok(Some(_order)) => {
                // 创建订单
                let order = self.order_manager.create_order_from_signal(signal)?;
                
                // 验证订单
                self.order_manager.validate_order(&order.client_order_id)?;
                
                info!("Order created and validated: {}", order.client_order_id);
            }
            Ok(None) => {
                debug!("Signal {} rejected by pipeline", signal.id);
            }
            Err(e) => {
                error!("Pipeline error for signal {}: {:?}", signal.id, e);
            }
        }
        
        self.processed_signals += 1;
        Ok(())
    }
    
    /// 处理执行报告（Post-process Pipeline）
    async fn process_execution_report(&mut self, report: ExecutionReport) -> Result<()> {
        debug!("Processing execution report: {}", report.order_id);
        
        // 更新订单状态
        self.order_manager.process_execution_report(report.clone())?;
        
        // 创建Pipeline上下文
        let ctx = PostProcessContext::new(report, self.shared_state.clone());
        
        // 执行Post-process Pipeline（链式调用）
        if let Err(e) = execute_post_pipeline(ctx).await {
            error!("Post-process pipeline error: {:?}", e);
        }
        
        self.processed_reports += 1;
        Ok(())
    }
    
    /// 输出统计信息
    fn print_statistics(&self) {
        info!("=== Statistics ===");
        info!("Processed signals: {}", self.processed_signals);
        info!("Processed reports: {}", self.processed_reports);
        
        let order_stats = self.order_manager.get_stats();
        info!("Active orders: {}", order_stats.active_orders);
        info!("Filled orders: {}", order_stats.filled_orders);
        info!("Success rate: {:.2}%", order_stats.success_rate * 100.0);
        
        let risk_summary = self.risk_state.get_summary();
        info!("Risk level: {:?}", risk_summary.risk_level);
        info!("Total exposure: {}", risk_summary.total_exposure);
        info!("Daily trades: {}", risk_summary.daily_trades);
    }
    
    /// 清理任务
    fn cleanup(&mut self) {
        debug!("Running cleanup tasks");
        
        // 清理已完成订单
        self.order_manager.cleanup_completed_orders(24);
        
        // 检查并重置日内统计
        self.risk_state.check_daily_reset();
        
        // 清理共享状态中的过期数据
        let state = self.shared_state.borrow_mut();
        state.persist();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into())
        )
        .init();
    
    info!("Pre/Post Processor starting...");
    
    // 创建并运行处理器
    let processor = PrePostProcessor::new();
    
    // 运行主循环
    if let Err(e) = processor.run().await {
        error!("Pre/Post Processor error: {:?}", e);
        return Err(e);
    }
    
    Ok(())
}