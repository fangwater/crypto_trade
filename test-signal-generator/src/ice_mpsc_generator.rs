use anyhow::Result;
use tracing::{info, error, debug, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use iceoryx2::prelude::*;
use iceoryx2::port::publisher::Publisher;
use chrono::Utc;
use common::signals::*;
use common::config::MarketConfig;
use tokio::sync::mpsc;
use tokio::select;
use core::time::Duration;
use std::sync::Arc;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// 定义不同的事件类型
#[derive(Debug, Clone)]
pub enum DataEvent {
    Market(MarketEvent),
    Funding(FundingEvent),
    OrderBook(OrderBookEvent),
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct MarketEvent {
    pub source: String,  // 数据源标识
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub bid: f64,
    pub ask: f64,
    pub spread: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct FundingEvent {
    pub source: String,
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub funding_rate: f64,
    pub next_funding_time: i64,
}

#[derive(Debug, Clone)]
pub struct OrderBookEvent {
    pub source: String,
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub bid_depth: f64,
    pub ask_depth: f64,
    pub imbalance: f64,  // (bid_depth - ask_depth) / (bid_depth + ask_depth)
}

// 数据源管理器
struct DataSourceManager {
    market_config: Arc<MarketConfig>,
}

impl DataSourceManager {
    fn new(market_config: Arc<MarketConfig>) -> Self {
        Self { market_config }
    }
    
    // 启动多个数据源，全部发送到同一个 channel
    async fn start_all_sources(&self, tx: mpsc::Sender<DataEvent>) {
        // 启动市场数据源（模拟WebSocket 1）
        self.start_market_source("binance_ws", tx.clone()).await;
        
        // 启动资金费率数据源（模拟WebSocket 2）
        self.start_funding_source("funding_api", tx.clone()).await;
        
        // 启动订单簿数据源（模拟WebSocket 3）
        self.start_orderbook_source("orderbook_ws", tx).await;
    }
    
    async fn start_market_source(&self, source_name: &str) -> mpsc::Sender<DataEvent> {
        let (tx, _) = mpsc::channel::<DataEvent>(1000);
        let tx_clone = tx.clone();
        let market_config = self.market_config.clone();
        let source = source_name.to_string();
        
        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();
            let binance_spot_id = market_config.get_exchange_id("binance_spot").unwrap_or(1);
            let binance_futures_id = market_config.get_exchange_id("binance_futures").unwrap_or(2);
            
            info!("[{}] Market data source started", source);
            
            loop {
                let exchange_id = if rng.gen_bool(0.5) {
                    binance_spot_id
                } else {
                    binance_futures_id
                };
                
                if let Some(symbols) = market_config.get_symbols(exchange_id) {
                    if !symbols.is_empty() {
                        let symbol = &symbols[rng.gen_range(0..symbols.len())];
                        
                        let base_price = 100.0 + rng.gen_range(-10.0..10.0);
                        let spread = rng.gen_range(0.0005..0.002);
                        let mid = base_price;
                        
                        let event = DataEvent::Market(MarketEvent {
                            source: source.clone(),
                            exchange_id,
                            symbol_id: symbol.id,
                            bid: mid - spread / 2.0,
                            ask: mid + spread / 2.0,
                            spread,
                            timestamp: Utc::now().timestamp_millis(),
                        });
                        
                        if tx_clone.send(event).await.is_err() {
                            break;
                        }
                    }
                }
                
                // 高频市场数据 (10-50ms)
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    rng.gen_range(10..50)
                )).await;
            }
        });
        
        tx
    }
    
    async fn start_funding_source(&self, source_name: &str) -> mpsc::Sender<DataEvent> {
        let (tx, _) = mpsc::channel::<DataEvent>(100);
        let tx_clone = tx.clone();
        let market_config = self.market_config.clone();
        let source = source_name.to_string();
        
        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();
            let binance_futures_id = market_config.get_exchange_id("binance_futures").unwrap_or(2);
            
            info!("[{}] Funding rate source started", source);
            
            loop {
                if let Some(symbols) = market_config.get_symbols(binance_futures_id) {
                    if !symbols.is_empty() {
                        let symbol = &symbols[rng.gen_range(0..symbols.len())];
                        
                        let event = DataEvent::Funding(FundingEvent {
                            source: source.clone(),
                            exchange_id: binance_futures_id,
                            symbol_id: symbol.id,
                            funding_rate: rng.gen_range(-0.002..0.002),
                            next_funding_time: Utc::now().timestamp_millis() + 8 * 3600 * 1000,
                        });
                        
                        if tx_clone.send(event).await.is_err() {
                            break;
                        }
                    }
                }
                
                // 资金费率更新较慢 (1-5秒)
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    rng.gen_range(1000..5000)
                )).await;
            }
        });
        
        tx
    }
    
    async fn start_orderbook_source(&self, source_name: &str) -> mpsc::Sender<DataEvent> {
        let (tx, _) = mpsc::channel::<DataEvent>(500);
        let tx_clone = tx.clone();
        let market_config = self.market_config.clone();
        let source = source_name.to_string();
        
        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();
            let exchanges = vec![
                market_config.get_exchange_id("binance_spot").unwrap_or(1),
                market_config.get_exchange_id("binance_futures").unwrap_or(2),
            ];
            
            info!("[{}] OrderBook source started", source);
            
            loop {
                let exchange_id = exchanges[rng.gen_range(0..exchanges.len())];
                
                if let Some(symbols) = market_config.get_symbols(exchange_id) {
                    if !symbols.is_empty() {
                        let symbol = &symbols[rng.gen_range(0..symbols.len())];
                        
                        let bid_depth = rng.gen_range(100.0..10000.0);
                        let ask_depth = rng.gen_range(100.0..10000.0);
                        let imbalance = (bid_depth - ask_depth) / (bid_depth + ask_depth);
                        
                        let event = DataEvent::OrderBook(OrderBookEvent {
                            source: source.clone(),
                            exchange_id,
                            symbol_id: symbol.id,
                            bid_depth,
                            ask_depth,
                            imbalance,
                        });
                        
                        if tx_clone.send(event).await.is_err() {
                            break;
                        }
                    }
                }
                
                // 订单簿更新频率中等 (50-200ms)
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    rng.gen_range(50..200)
                )).await;
            }
        });
        
        tx
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Starting Low-Latency MPSC Multi-Source Signal Generator");
    
    // 加载配置
    let config_path = if std::path::Path::new("config").exists() {
        "config"
    } else if std::path::Path::new("../config").exists() {
        "../config"
    } else {
        "../../config"
    };
    
    let market_config = Arc::new(MarketConfig::load(config_path)?);
    market_config.debug_print();
    
    // 创建数据源管理器
    let source_manager = DataSourceManager::new(market_config.clone());
    
    // 创建接收通道 (合并所有数据源)
    let (merged_tx, mut merged_rx) = mpsc::channel::<DataEvent>(2000);
    
    // 启动所有数据源
    let sources = source_manager.start_all_sources().await;
    
    // 直接使用返回的 sender，它们都发送到各自的 receiver
    // 数据源已经在 spawn 的任务中运行
    
    // 创建 IceOryx2 Node
    let node_name = format!("gen_{}", std::process::id());
    info!("Creating node with name: {}", node_name);
    let node = NodeBuilder::new()
        .name(&NodeName::new(&node_name)?)
        .create::<ipc::Service>()?;
    
    // 创建发布者
    let adaptive_spread_service = node
        .service_builder(&ServiceName::new("signals/adaptive_spread")?)
        .publish_subscribe::<[u8; 1024]>()
        .open_or_create()?;
    let adaptive_publisher = adaptive_spread_service.publisher_builder().create()?;
    
    let funding_rate_service = node
        .service_builder(&ServiceName::new("signals/funding_rate")?)
        .publish_subscribe::<[u8; 1024]>()
        .open_or_create()?;
    let funding_publisher = funding_rate_service.publisher_builder().create()?;
    
    // 统计
    let mut stats = Statistics::new();
    let mut stats_timer = tokio::time::interval(tokio::time::Duration::from_secs(5));
    
    // 用于检查节点状态的定时器
    let mut node_check_timer = tokio::time::interval(tokio::time::Duration::from_millis(100));
    
    info!("Starting main event loop");
    
    loop {
        select! {
            // 优先处理数据事件（低延迟）
            Some(event) = merged_rx.recv() => {
                let start = std::time::Instant::now();
                
                match event {
                    DataEvent::Market(market) => {
                        stats.market_events += 1;
                        process_market_event(&market, &adaptive_publisher, &market_config)?;
                    }
                    DataEvent::Funding(funding) => {
                        stats.funding_events += 1;
                        process_funding_event(&funding, &funding_publisher, &market_config)?;
                    }
                    DataEvent::OrderBook(orderbook) => {
                        stats.orderbook_events += 1;
                        process_orderbook_event(&orderbook, &adaptive_publisher, &market_config)?;
                    }
                    DataEvent::Shutdown => {
                        info!("Received shutdown signal");
                        break;
                    }
                }
                
                let latency = start.elapsed();
                stats.update_latency(latency);
            }
            
            // 定期输出统计
            _ = stats_timer.tick() => {
                stats.print_and_reset();
            }
            
            // 检查节点状态
            _ = node_check_timer.tick() => {
                match node.wait(Duration::ZERO) {
                    NodeEvent::TerminationRequest | NodeEvent::InterruptSignal => {
                        info!("Received termination signal");
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
    
    Ok(())
}

fn process_market_event<T: iceoryx2::service::Service>(
    event: &MarketEvent,
    publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<()> {
    let symbol_info = market_config.get_symbol(event.exchange_id, event.symbol_id);
    let symbol_name = symbol_info.map(|s| s.symbol.as_str()).unwrap_or("UNKNOWN");
    
    // 只在价差超过阈值时发送信号
    if event.spread > 0.001 {
        let signal = Signal::AdaptiveSpreadDeviation(AdaptiveSpreadDeviationSignal {
            exchange_id: event.exchange_id,
            symbol_id: event.symbol_id,
            spread_percentile: calculate_spread_percentile(event.spread),
            current_spread: event.spread,
            threshold_percentile: 0.8,
            timestamp: Utc::now(),
        });
        
        send_signal(&signal, publisher, symbol_name)?;
    }
    
    Ok(())
}

fn process_funding_event<T: iceoryx2::service::Service>(
    event: &FundingEvent,
    publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<()> {
    let symbol_info = market_config.get_symbol(event.exchange_id, event.symbol_id);
    let symbol_name = symbol_info.map(|s| s.symbol.as_str()).unwrap_or("UNKNOWN");
    
    let direction = if event.funding_rate > 0.0 {
        FundingDirection::Positive
    } else if event.funding_rate < 0.0 {
        FundingDirection::Negative
    } else {
        FundingDirection::Neutral
    };
    
    let signal = Signal::FundingRateDirection(FundingRateDirectionSignal {
        exchange_id: event.exchange_id,
        symbol_id: event.symbol_id,
        funding_rate: event.funding_rate,
        direction,
        timestamp: Utc::now(),
    });
    
    send_signal(&signal, publisher, symbol_name)?;
    Ok(())
}

fn process_orderbook_event<T: iceoryx2::service::Service>(
    event: &OrderBookEvent,
    publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<()> {
    // 基于订单簿不平衡生成信号
    if event.imbalance.abs() > 0.3 {
        let symbol_info = market_config.get_symbol(event.exchange_id, event.symbol_id);
        let symbol_name = symbol_info.map(|s| s.symbol.as_str()).unwrap_or("UNKNOWN");
        
        // 这里可以生成基于订单簿的特殊信号
        debug!("[{}] OrderBook imbalance: {} for {}", 
               event.source, event.imbalance, symbol_name);
    }
    
    Ok(())
}

fn send_signal<T: iceoryx2::service::Service>(
    signal: &Signal,
    publisher: &Publisher<T, [u8; 1024], ()>,
    symbol_name: &str,
) -> Result<()> {
    let bytes = signal.to_bytes();
    
    if bytes.len() <= 1024 {
        let mut buffer = [0u8; 1024];
        buffer[..bytes.len()].copy_from_slice(&bytes);
        
        match publisher.loan_uninit() {
            Ok(sample) => {
                let sample = sample.write_payload(buffer);
                match sample.send() {
                    Ok(_) => {
                        debug!("Sent signal for {} - {} bytes", symbol_name, bytes.len());
                    }
                    Err(e) => error!("Send failed: {:?}", e),
                }
            }
            Err(e) => error!("Failed to loan sample: {:?}", e),
        }
    }
    
    Ok(())
}

fn calculate_spread_percentile(spread: f64) -> f64 {
    match spread {
        s if s <= 0.0005 => 0.1,
        s if s <= 0.001 => 0.3,
        s if s <= 0.0015 => 0.5,
        s if s <= 0.002 => 0.7,
        s if s <= 0.003 => 0.9,
        _ => 0.95,
    }
}

// 统计信息
struct Statistics {
    market_events: u64,
    funding_events: u64,
    orderbook_events: u64,
    total_latency_us: u64,
    max_latency_us: u64,
    min_latency_us: u64,
    event_count: u64,
}

impl Statistics {
    fn new() -> Self {
        Self {
            market_events: 0,
            funding_events: 0,
            orderbook_events: 0,
            total_latency_us: 0,
            max_latency_us: 0,
            min_latency_us: u64::MAX,
            event_count: 0,
        }
    }
    
    fn update_latency(&mut self, latency: std::time::Duration) {
        let us = latency.as_micros() as u64;
        self.total_latency_us += us;
        self.max_latency_us = self.max_latency_us.max(us);
        self.min_latency_us = self.min_latency_us.min(us);
        self.event_count += 1;
    }
    
    fn print_and_reset(&mut self) {
        if self.event_count > 0 {
            let avg_latency = self.total_latency_us / self.event_count;
            info!(
                "Stats [5s]: Market:{} Funding:{} OrderBook:{} | Latency(μs) avg:{} min:{} max:{}",
                self.market_events,
                self.funding_events,
                self.orderbook_events,
                avg_latency,
                self.min_latency_us,
                self.max_latency_us
            );
        }
        
        // 重置统计
        self.market_events = 0;
        self.funding_events = 0;
        self.orderbook_events = 0;
        self.total_latency_us = 0;
        self.max_latency_us = 0;
        self.min_latency_us = u64::MAX;
        self.event_count = 0;
    }
}