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
    pub source: String,
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
    pub imbalance: f64,
}

// 启动市场数据源
fn spawn_market_source(
    source_name: String,
    tx: mpsc::Sender<DataEvent>,
    market_config: Arc<MarketConfig>,
) {
    tokio::spawn(async move {
        let mut rng = StdRng::from_entropy();
        let binance_spot_id = market_config.get_exchange_id("binance_spot").unwrap_or(1);
        let binance_futures_id = market_config.get_exchange_id("binance_futures").unwrap_or(2);
        
        info!("[{}] Market data source started", source_name);
        
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
                        source: source_name.clone(),
                        exchange_id,
                        symbol_id: symbol.id,
                        bid: mid - spread / 2.0,
                        ask: mid + spread / 2.0,
                        spread,
                        timestamp: Utc::now().timestamp_millis(),
                    });
                    
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
            
            // 超高频市场数据 - 几乎不休眠，达到最大吞吐
            if rng.gen_bool(0.001) {  // 0.1% 概率休眠1ms，避免完全占满CPU
                tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
            }
        }
        
        info!("[{}] Market data source stopped", source_name);
    });
}

// 启动资金费率数据源
fn spawn_funding_source(
    source_name: String,
    tx: mpsc::Sender<DataEvent>,
    market_config: Arc<MarketConfig>,
) {
    tokio::spawn(async move {
        let mut rng = StdRng::from_entropy();
        let binance_futures_id = market_config.get_exchange_id("binance_futures").unwrap_or(2);
        
        info!("[{}] Funding rate source started", source_name);
        
        loop {
            if let Some(symbols) = market_config.get_symbols(binance_futures_id) {
                if !symbols.is_empty() {
                    let symbol = &symbols[rng.gen_range(0..symbols.len())];
                    
                    let event = DataEvent::Funding(FundingEvent {
                        source: source_name.clone(),
                        exchange_id: binance_futures_id,
                        symbol_id: symbol.id,
                        funding_rate: rng.gen_range(-0.002..0.002),
                        next_funding_time: Utc::now().timestamp_millis() + 8 * 3600 * 1000,
                    });
                    
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
            
            // 资金费率也提高频率用于测试
            tokio::time::sleep(tokio::time::Duration::from_millis(
                rng.gen_range(10..100)
            )).await;
        }
        
        info!("[{}] Funding rate source stopped", source_name);
    });
}

// 启动订单簿数据源
fn spawn_orderbook_source(
    source_name: String,
    tx: mpsc::Sender<DataEvent>,
    market_config: Arc<MarketConfig>,
) {
    tokio::spawn(async move {
        let mut rng = StdRng::from_entropy();
        let exchanges = vec![
            market_config.get_exchange_id("binance_spot").unwrap_or(1),
            market_config.get_exchange_id("binance_futures").unwrap_or(2),
        ];
        
        info!("[{}] OrderBook source started", source_name);
        
        loop {
            let exchange_id = exchanges[rng.gen_range(0..exchanges.len())];
            
            if let Some(symbols) = market_config.get_symbols(exchange_id) {
                if !symbols.is_empty() {
                    let symbol = &symbols[rng.gen_range(0..symbols.len())];
                    
                    let bid_depth = rng.gen_range(100.0..10000.0);
                    let ask_depth = rng.gen_range(100.0..10000.0);
                    let imbalance = (bid_depth - ask_depth) / (bid_depth + ask_depth);
                    
                    let event = DataEvent::OrderBook(OrderBookEvent {
                        source: source_name.clone(),
                        exchange_id,
                        symbol_id: symbol.id,
                        bid_depth,
                        ask_depth,
                        imbalance,
                    });
                    
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
            
            // 订单簿也高频更新
            tokio::time::sleep(tokio::time::Duration::from_millis(
                rng.gen_range(1..10)
            )).await;
        }
        
        info!("[{}] OrderBook source stopped", source_name);
    });
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
    
    // 创建单一的 mpsc channel，所有数据源都发送到这里
    let (tx, mut rx) = mpsc::channel::<DataEvent>(2000);
    
    // 启动多个数据源以达到高吞吐
    for i in 0..10 {  // 10个市场数据源
        spawn_market_source(format!("market_ws_{}", i), tx.clone(), market_config.clone());
    }
    
    for i in 0..3 {  // 3个资金费率源
        spawn_funding_source(format!("funding_api_{}", i), tx.clone(), market_config.clone());
    }
    
    for i in 0..5 {  // 5个订单簿源
        spawn_orderbook_source(format!("orderbook_ws_{}", i), tx.clone(), market_config.clone());
    }
    
    info!("All data sources started");
    
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
    let mut stats_timer = tokio::time::interval(tokio::time::Duration::from_secs(3));
    
    // 用于检查节点状态的定时器
    let mut node_check_timer = tokio::time::interval(tokio::time::Duration::from_millis(100));
    
    info!("Starting main event loop");
    
    loop {
        select! {
            // 优先处理数据事件（低延迟）
            Some(event) = rx.recv() => {
                let start = std::time::Instant::now();
                
                match event {
                    DataEvent::Market(market) => {
                        stats.market_events += 1;
                        let sent = process_market_event(&market, &adaptive_publisher, &market_config)?;
                        if sent { stats.signals_sent += 1; }
                    }
                    DataEvent::Funding(funding) => {
                        stats.funding_events += 1;
                        let sent = process_funding_event(&funding, &funding_publisher, &market_config)?;
                        if sent { stats.signals_sent += 1; }
                    }
                    DataEvent::OrderBook(orderbook) => {
                        stats.orderbook_events += 1;
                        let sent = process_orderbook_event(&orderbook, &adaptive_publisher, &market_config)?;
                        if sent { stats.signals_sent += 1; }
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
                        let _ = tx.send(DataEvent::Shutdown).await;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }
    
    info!("Signal generator shutting down");
    Ok(())
}

fn process_market_event<T: iceoryx2::service::Service>(
    event: &MarketEvent,
    publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<bool> {
    let symbol_info = market_config.get_symbol(event.exchange_id, event.symbol_id);
    let symbol_name = symbol_info.map(|s| s.symbol.as_str()).unwrap_or("UNKNOWN");
    
    // 降低阈值以增加发送频率
    if event.spread > 0.0005 {
        let signal = Signal::AdaptiveSpreadDeviation(AdaptiveSpreadDeviationSignal {
            exchange_id: event.exchange_id,
            symbol_id: event.symbol_id,
            spread_percentile: calculate_spread_percentile(event.spread),
            current_spread: event.spread,
            threshold_percentile: 0.8,
            timestamp: Utc::now(),
        });
        
        send_signal(&signal, publisher, symbol_name)?;
        return Ok(true);
    }
    
    Ok(false)
}

fn process_funding_event<T: iceoryx2::service::Service>(
    event: &FundingEvent,
    publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<bool> {
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
    Ok(true)
}

fn process_orderbook_event<T: iceoryx2::service::Service>(
    event: &OrderBookEvent,
    _publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<bool> {
    // 基于订单簿不平衡生成信号
    if event.imbalance.abs() > 0.3 {
        let symbol_info = market_config.get_symbol(event.exchange_id, event.symbol_id);
        let symbol_name = symbol_info.map(|s| s.symbol.as_str()).unwrap_or("UNKNOWN");
        
        debug!("[{}] OrderBook imbalance: {:.3} for {}", 
               event.source, event.imbalance, symbol_name);
        return Ok(true);
    }
    
    Ok(false)
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
    signals_sent: u64,  // IceOryx 发送的信号数
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
            signals_sent: 0,
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
            let total_events = self.market_events + self.funding_events + self.orderbook_events;
            let events_per_sec = total_events / 3;  // 3秒统计周期
            let signals_per_sec = self.signals_sent / 3;
            
            info!(
                "Stats [3s]: Events/s:{} IceOryx/s:{} | Market:{} Fund:{} Order:{} | Latency(μs) avg:{} min:{} max:{}",
                events_per_sec,
                signals_per_sec,
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
        self.signals_sent = 0;
        self.total_latency_us = 0;
        self.max_latency_us = 0;
        self.min_latency_us = u64::MAX;
        self.event_count = 0;
    }
}