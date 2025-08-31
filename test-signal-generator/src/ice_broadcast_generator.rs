use anyhow::Result;
use tracing::{info, error, debug, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use iceoryx2::prelude::*;
use iceoryx2::port::publisher::Publisher;
use chrono::Utc;
use common::signals::*;
use common::config::MarketConfig;
use tokio::sync::broadcast;
use tokio::select;
use core::time::Duration;
use std::sync::Arc;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// 定义市场数据事件
#[derive(Debug, Clone)]
pub struct MarketEvent {
    pub exchange_id: u32,
    pub symbol_id: u32,
    pub bid: f64,
    pub ask: f64,
    pub spread: f64,
    pub funding_rate: Option<f64>,
}

// 市场数据源管理器
pub struct MarketDataSource {
    sender: broadcast::Sender<MarketEvent>,
    market_config: Arc<MarketConfig>,
}

impl MarketDataSource {
    pub fn new(market_config: Arc<MarketConfig>, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            market_config,
        }
    }
    
    pub fn get_sender(&self) -> broadcast::Sender<MarketEvent> {
        self.sender.clone()
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<MarketEvent> {
        self.sender.subscribe()
    }
    
    // 模拟WebSocket连接推送数据
    pub async fn start_mock_feed(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy();
            
            let binance_spot_id = self.market_config.get_exchange_id("binance_spot").unwrap_or(1);
            let binance_futures_id = self.market_config.get_exchange_id("binance_futures").unwrap_or(2);
            
            info!("Starting mock market data feed");
            
            loop {
                // 随机选择交易所
                let exchange_id = if rng.gen_bool(0.5) {
                    binance_spot_id
                } else {
                    binance_futures_id
                };
                
                if let Some(symbols) = self.market_config.get_symbols(exchange_id) {
                    if !symbols.is_empty() {
                        let symbol_idx = rng.gen_range(0..symbols.len());
                        let symbol = &symbols[symbol_idx];
                        
                        // 生成模拟市场数据
                        let base_price = 100.0;
                        let spread = rng.gen_range(0.001..0.003);
                        let mid = base_price + rng.gen_range(-1.0..1.0);
                        let bid = mid - spread / 2.0;
                        let ask = mid + spread / 2.0;
                        
                        let event = MarketEvent {
                            exchange_id,
                            symbol_id: symbol.id,
                            bid,
                            ask,
                            spread,
                            funding_rate: if rng.gen_bool(0.3) {
                                Some(rng.gen_range(-0.001..0.001))
                            } else {
                                None
                            },
                        };
                        
                        // 使用 broadcast 发送，如果没有接收者会返回错误
                        match self.sender.send(event) {
                            Ok(receiver_count) => {
                                debug!("Sent market event to {} receivers", receiver_count);
                            }
                            Err(_) => {
                                debug!("No active receivers for market event");
                            }
                        }
                    }
                }
                
                // 模拟不同频率的市场数据
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    rng.gen_range(100..500)
                )).await;
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()
            .add_directive(tracing::Level::DEBUG.into()))
        .init();

    info!("Starting Broadcast-based IceOryx Signal Generator");
    
    // 加载市场配置
    let config_path = if std::path::Path::new("config").exists() {
        "config"
    } else if std::path::Path::new("../config").exists() {
        "../config"
    } else {
        "../../config"
    };
    
    let market_config = Arc::new(MarketConfig::load(config_path)?);
    market_config.debug_print();
    
    // 创建市场数据源
    let data_source = Arc::new(MarketDataSource::new(market_config.clone(), 1000));
    
    // 启动模拟数据源
    data_source.clone().start_mock_feed().await;
    
    // 订阅市场数据
    let mut market_rx = data_source.subscribe();
    
    // 生成器创建独立的Node
    let node_name = format!("gen_{}", std::process::id());
    info!("Creating node with name: {}", node_name);
    let node = NodeBuilder::new()
        .name(&NodeName::new(&node_name)?)
        .create::<ipc::Service>()?;
    
    // 创建自适应价差信号发布者
    info!("Creating adaptive spread service");
    let adaptive_spread_service = node
        .service_builder(&ServiceName::new("signals/adaptive_spread")?)
        .publish_subscribe::<[u8; 1024]>()
        .open_or_create()?;
    let adaptive_publisher = adaptive_spread_service.publisher_builder().create()?;
    
    // 创建资金费率方向信号发布者
    info!("Creating funding rate service");
    let funding_rate_service = node
        .service_builder(&ServiceName::new("signals/funding_rate")?)
        .publish_subscribe::<[u8; 1024]>()
        .open_or_create()?;
    let funding_publisher = funding_rate_service.publisher_builder().create()?;
    
    // 创建一个定时器用于检查节点状态
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
    
    // 统计信息
    let mut event_count = 0u64;
    let mut last_report = std::time::Instant::now();
    
    info!("Starting event-driven main loop with broadcast channel");
    
    loop {
        select! {
            // 接收市场数据事件
            Ok(event) = market_rx.recv() => {
                event_count += 1;
                
                process_market_event(
                    &event,
                    &adaptive_publisher,
                    &funding_publisher,
                    &market_config,
                )?;
                
                // 每10秒报告一次统计
                if last_report.elapsed().as_secs() >= 10 {
                    info!("Processed {} market events in last 10 seconds", event_count);
                    event_count = 0;
                    last_report = std::time::Instant::now();
                }
            }
            
            // 定期检查节点状态
            _ = interval.tick() => {
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
}

fn process_market_event<T: iceoryx2::service::Service>(
    event: &MarketEvent,
    adaptive_publisher: &Publisher<T, [u8; 1024], ()>,
    funding_publisher: &Publisher<T, [u8; 1024], ()>,
    market_config: &MarketConfig,
) -> Result<()> {
    // 获取符号信息用于日志
    let symbol_info = market_config.get_symbol(event.exchange_id, event.symbol_id);
    let symbol_name = symbol_info.map(|s| s.symbol.as_str()).unwrap_or("UNKNOWN");
    
    debug!("Processing market event for Exchange {} Symbol {} ({})", 
           event.exchange_id, event.symbol_id, symbol_name);
    
    // 基于价差阈值决定是否发送信号
    let spread_threshold = 0.002; // 0.2%
    
    if event.spread > spread_threshold {
        // 生成自适应价差信号
        let spread_percentile = calculate_spread_percentile(event.spread);
        let adaptive_signal = Signal::AdaptiveSpreadDeviation(AdaptiveSpreadDeviationSignal {
            exchange_id: event.exchange_id,
            symbol_id: event.symbol_id,
            spread_percentile,
            current_spread: event.spread,
            threshold_percentile: 0.8,
            timestamp: Utc::now(),
        });
        
        send_signal(&adaptive_signal, adaptive_publisher, "自适应价差", 
                   event.exchange_id, event.symbol_id, symbol_name)?;
    }
    
    // 如果有资金费率且超过阈值，生成资金费率信号
    if let Some(funding_rate) = event.funding_rate {
        let funding_threshold = 0.0005; // 0.05%
        
        if funding_rate.abs() > funding_threshold {
            let direction = if funding_rate > 0.0 {
                FundingDirection::Positive
            } else if funding_rate < 0.0 {
                FundingDirection::Negative
            } else {
                FundingDirection::Neutral
            };
            
            let funding_signal = Signal::FundingRateDirection(FundingRateDirectionSignal {
                exchange_id: event.exchange_id,
                symbol_id: event.symbol_id,
                funding_rate,
                direction,
                timestamp: Utc::now(),
            });
            
            send_signal(&funding_signal, funding_publisher, "资金费率",
                       event.exchange_id, event.symbol_id, symbol_name)?;
        }
    }
    
    Ok(())
}

fn send_signal<T: iceoryx2::service::Service>(
    signal: &Signal,
    publisher: &Publisher<T, [u8; 1024], ()>,
    signal_type: &str,
    exchange_id: u32,
    symbol_id: u32,
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
                        info!("发送{}信号 - Exchange:{} Symbol:{} ({}) - {} bytes", 
                             signal_type, exchange_id, symbol_id, symbol_name, bytes.len());
                    }
                    Err(e) => error!("发送失败: {:?}", e),
                }
            }
            Err(e) => error!("Failed to loan sample: {:?}", e),
        }
    } else {
        warn!("Signal too large: {} bytes", bytes.len());
    }
    
    Ok(())
}

fn calculate_spread_percentile(spread: f64) -> f64 {
    // 基于历史数据的简单百分位计算
    // 实际应用中应该维护滑动窗口的历史数据
    let percentiles = [
        (0.0005, 0.1),
        (0.001, 0.3),
        (0.0015, 0.5),
        (0.002, 0.7),
        (0.003, 0.9),
    ];
    
    for (threshold, percentile) in percentiles.iter() {
        if spread <= *threshold {
            return *percentile;
        }
    }
    
    0.95 // 超过所有阈值
}