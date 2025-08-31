use anyhow::Result;
use tracing::{info, error, debug};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use iceoryx2::prelude::*;
use chrono::Utc;
use common::signals::*;
use common::config::MarketConfig;
use rand::Rng;
use core::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()
            .add_directive(tracing::Level::DEBUG.into()))
        .init();

    info!("Starting IceOryx Signal Generator");
    
    // 加载市场配置
    let config_path = if std::path::Path::new("config").exists() {
        "config"
    } else if std::path::Path::new("../config").exists() {
        "../config"
    } else {
        "../../config"
    };
    
    let market_config = MarketConfig::load(config_path)?;
    market_config.debug_print();
    
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
    
    let mut rng = rand::thread_rng();
    
    // 获取币安现货和期货的交易所ID
    let binance_spot_id = market_config.get_exchange_id("binance_spot")
        .expect("Binance spot exchange not found");
    let binance_futures_id = market_config.get_exchange_id("binance_futures")
        .expect("Binance futures exchange not found");
    
    // 获取可用的符号列表
    let spot_symbols = market_config.get_symbols(binance_spot_id)
        .expect("No symbols found for Binance spot");
    let futures_symbols = market_config.get_symbols(binance_futures_id)
        .expect("No symbols found for Binance futures");
    
    info!("Found {} spot symbols and {} futures symbols", 
          spot_symbols.len(), futures_symbols.len());
    
    const CYCLE_TIME: Duration = Duration::from_secs(2);
    
    info!("Starting main loop");
    loop {
        match node.wait(CYCLE_TIME) {
            NodeEvent::Tick => {
                // 随机选择一个交易所和符号
                let use_spot = rng.gen_bool(0.5);
                let (exchange_id, symbols) = if use_spot {
                    (binance_spot_id, spot_symbols)
                } else {
                    (binance_futures_id, futures_symbols)
                };
                
                // 随机选择一个符号
                let symbol_index = rng.gen_range(0..symbols.len());
                let symbol = &symbols[symbol_index];
                
                debug!("Generating signals for Exchange {} Symbol {} ({})", 
                       exchange_id, symbol.id, symbol.symbol);
                
                // 生成自适应价差信号
                let spread_variation = rng.gen_range(-1.0..1.0);
                let adaptive_signal = Signal::AdaptiveSpreadDeviation(AdaptiveSpreadDeviationSignal {
                    exchange_id,
                    symbol_id: symbol.id,
                    spread_percentile: 0.85 + 0.1 * spread_variation,
                    current_spread: 0.0015 + 0.0005 * spread_variation,
                    threshold_percentile: 0.8,
                    timestamp: Utc::now(),
                });
                
                let bytes = adaptive_signal.to_bytes();
                debug!("Adaptive signal serialized to {} bytes", bytes.len());
                
                if bytes.len() <= 1024 {
                    // 创建固定大小的数组
                    let mut buffer = [0u8; 1024];
                    buffer[..bytes.len()].copy_from_slice(&bytes);
                    
                    // 使用loan_uninit模式
                    match adaptive_publisher.loan_uninit() {
                        Ok(sample) => {
                            let sample = sample.write_payload(buffer);
                            match sample.send() {
                                Ok(_) => info!("发送自适应价差信号 - Exchange:{} Symbol:{} ({}) - {} bytes", 
                                             exchange_id, symbol.id, symbol.symbol, bytes.len()),
                                Err(e) => error!("发送失败: {:?}", e),
                            }
                        }
                        Err(e) => error!("Failed to loan sample: {:?}", e),
                    }
                } else {
                    error!("Signal too large: {} bytes", bytes.len());
                }
                
                // 生成资金费率方向信号
                let funding_direction = if rng.gen_bool(0.6) {
                    FundingDirection::Positive
                } else {
                    FundingDirection::Negative
                };
                
                let funding_variation = rng.gen_range(-2.0..3.0);
                let funding_signal = Signal::FundingRateDirection(FundingRateDirectionSignal {
                    exchange_id,
                    symbol_id: symbol.id,
                    funding_rate: 0.0002 * funding_variation,
                    direction: funding_direction,
                    timestamp: Utc::now(),
                });
                
                let bytes = funding_signal.to_bytes();
                debug!("Funding signal serialized to {} bytes", bytes.len());
                
                if bytes.len() <= 1024 {
                    // 创建固定大小的数组
                    let mut buffer = [0u8; 1024];
                    buffer[..bytes.len()].copy_from_slice(&bytes);
                    
                    // 使用loan_uninit模式
                    match funding_publisher.loan_uninit() {
                        Ok(sample) => {
                            let sample = sample.write_payload(buffer);
                            match sample.send() {
                                Ok(_) => info!("发送资金费率信号 - Exchange:{} Symbol:{} ({}) Direction:{:?} - {} bytes", 
                                             exchange_id, symbol.id, symbol.symbol, funding_direction, bytes.len()),
                                Err(e) => error!("发送失败: {:?}", e),
                            }
                        }
                        Err(e) => error!("Failed to loan sample: {:?}", e),
                    }
                } else {
                    error!("Signal too large: {} bytes", bytes.len());
                }
            }
            NodeEvent::TerminationRequest | NodeEvent::InterruptSignal => {
                info!("Received termination signal");
                break;
            }
        }
    }
    
    info!("Signal generator shutting down");
    Ok(())
}