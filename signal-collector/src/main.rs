use anyhow::Result;
use tokio::sync::mpsc;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod signal_manager;
mod trigger;
mod event_generator;
mod ipc_subscriber;
mod ipc_publisher;
mod config;

use signal_manager::SignalManager;
use trigger::TriggerRegistry;
use event_generator::EventGenerator;
use ipc_subscriber::{IceOryxSubscriber, ZmqSubscriber};
use ipc_publisher::IpcPublisher;
use config::Config;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env()
            .add_directive(tracing::Level::DEBUG.into()))
        .init();

    info!("Starting Signal Collector Process");

    let config = Config::load()?;
    
    let (signal_tx, mut signal_rx) = mpsc::channel(1024);
    let (event_tx, event_rx) = mpsc::channel(1024);

    let mut signal_manager = SignalManager::new();
    let mut trigger_registry = TriggerRegistry::new();
    let mut event_generator = EventGenerator::new(event_tx.clone());
    
    // 注册默认触发器并设置信号到触发器的映射
    let trigger_mappings = trigger_registry.register_default_triggers();
    for (trigger_idx, signal_indices) in trigger_mappings {
        for signal_idx in signal_indices {
            signal_manager.register_trigger(signal_idx, trigger_idx);
        }
    }

    // 启动IceOryx订阅者线程
    IceOryxSubscriber::spawn_subscribers(signal_tx.clone(), config.iceoryx_topics.clone());
    
    // 启动ZMQ订阅者线程
    ZmqSubscriber::spawn_subscribers(signal_tx.clone(), config.zmq_endpoints.clone());
    
    // 等待一下让订阅者先创建节点
    std::thread::sleep(std::time::Duration::from_millis(100));
    
    // 启动IceOryx发布者线程
    IpcPublisher::spawn_iceoryx_publisher(event_rx, config.output_topic.clone());

    info!("All subscribers and publishers started");

    while let Some(signal_msg) = signal_rx.recv().await {
        let signal_type = signal_msg.signal.signal_type();
        
        signal_manager.update_signal(signal_msg.signal.clone());
        
        // 获取该信号关联的所有触发器索引
        let trigger_indices = signal_manager.get_trigger_indices_for_signal(signal_type);
        
        for trigger_idx in trigger_indices {
            if let Some(trigger) = trigger_registry.get_trigger(trigger_idx) {
                if let Some(event) = trigger.evaluate(&signal_manager, &signal_msg.signal) {
                    event_generator.send_event(event).await?;
                }
            }
        }
    }

    Ok(())
}