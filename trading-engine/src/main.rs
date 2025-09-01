mod config;
mod ws_pool;
mod executor;
mod adapters;
mod health;
mod ipc;

use config::TradingEngineConfig;
use executor::OrderExecutor;
use health::{ConnectionSelector, HealthTracker};
use ipc::IpcManager;
use ws_pool::WsPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("trading_engine=info".parse()?))
        .init();
    
    info!("Starting Trading Engine");
    
    // Load configuration
    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config/trading_engine.toml".to_string());
    let config = TradingEngineConfig::from_file(&config_path)?;
    
    // Create channels
    let (command_tx, mut command_rx) = mpsc::unbounded_channel();
    let (response_tx, response_rx) = mpsc::unbounded_channel();
    
    // Initialize components
    let health_tracker = Arc::new(HealthTracker::new());
    let connection_selector = Arc::new(ConnectionSelector::new(
        health_tracker.clone(),
        health::connection_selector::SelectionStrategy::HealthScore,
    ));
    
    // Initialize WebSocket pool
    let mut ws_pool = WsPool::new(config.clone());
    ws_pool.start().await?;
    
    // Get message receiver before moving ws_pool into Arc
    let message_rx = ws_pool.take_message_receiver();
    let ws_pool = Arc::new(ws_pool);
    
    // Initialize order executor
    let executor = Arc::new(OrderExecutor::new(
        config.executor.clone(),
        ws_pool.clone(),
        health_tracker.clone(),
        connection_selector.clone(),
    ));
    
    // Register signers for each exchange
    for (exchange_name, exchange_config) in &config.exchanges {
        if exchange_config.enabled {
            executor.register_signer(
                exchange_name.clone(),
                exchange_config.api_key.clone(),
                exchange_config.secret_key.clone(),
            );
        }
    }
    
    // Initialize IPC manager
    let mut ipc_manager = IpcManager::new(config.ipc.clone(), command_tx)?;
    ipc_manager.initialize()?;
    ipc_manager.start().await?;
    
    // Use message receiver from WebSocket pool
    if let Some(mut message_rx) = message_rx {
        // Start message forwarding task
        let response_tx_clone = response_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = message_rx.recv().await {
                // Forward WebSocket messages to IPC output
                if let Err(e) = response_tx_clone.send(msg) {
                    error!("Failed to forward WebSocket message: {}", e);
                }
            }
        });
    }
    
    // Main execution loop
    info!("Trading Engine started successfully");
    
    loop {
        tokio::select! {
            Some(command) = command_rx.recv() => {
                let executor = executor.clone();
                let response_tx = response_tx.clone();
                tokio::spawn(async move {
                    info!("Executing command: {:?}", command.id);
                    let result = executor.execute(command).await;
                    
                    if result.success {
                        info!("Command executed successfully: {:?}", result.command_id);
                    } else {
                        error!("Command execution failed: {:?} - {:?}", result.command_id, result.error);
                    }
                    
                    // Send result through IPC if needed
                    if let Ok(result_bytes) = serde_json::to_vec(&result) {
                        // Send through response channel
                        let _ = response_tx.send(bytes::Bytes::from(result_bytes));
                    }
                });
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
                break;
            }
        }
    }
    
    // Cleanup
    info!("Shutting down Trading Engine");
    ws_pool.shutdown().await;
    
    Ok(())
}