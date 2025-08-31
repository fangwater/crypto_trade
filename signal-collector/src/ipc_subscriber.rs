use tokio::sync::mpsc;
use anyhow::Result;
use tracing::{info, error, warn, debug};
use common::messages::SignalMessage;
use common::signals::Signal;
use bytes::Bytes;
use chrono::Utc;
use core::time::Duration;

pub struct IceOryxSubscriber;

impl IceOryxSubscriber {
    pub fn spawn_subscribers(tx: mpsc::Sender<SignalMessage>, topics: Vec<String>) {
        // 创建单个线程处理所有订阅
        std::thread::spawn(move || {
            if let Err(e) = Self::run_all_subscribers(tx, topics) {
                error!("Subscriber thread error: {}", e);
            }
        });
    }
    
    fn run_all_subscribers(tx: mpsc::Sender<SignalMessage>, topics: Vec<String>) -> Result<()> {
        use iceoryx2::prelude::*;
        
        info!("Starting IceOryx subscriber for {} topics", topics.len());
        
        // 创建单个节点
        let node_name = format!("sub{}", std::process::id());
        info!("Creating subscriber node with name: {}", node_name);
        let node = NodeBuilder::new()
            .name(&NodeName::new(&node_name)?)
            .create::<ipc::Service>()?;
        
        // 为每个主题创建订阅者
        let mut subscribers = Vec::new();
        for topic in &topics {
            let service_name = ServiceName::new(topic)?;
            let service = node
                .service_builder(&service_name)
                .publish_subscribe::<[u8; 1024]>()
                .open_or_create()?;
            
            let subscriber = service.subscriber_builder().create()?;
            info!("Subscriber ready for topic: {}", topic);
            subscribers.push((topic.clone(), subscriber));
        }
        
        const CYCLE_TIME: Duration = Duration::from_millis(100);
        let mut msg_counts: Vec<u64> = vec![0; topics.len()];
        
        loop {
            match node.wait(CYCLE_TIME) {
                NodeEvent::Tick => {
                    // 检查每个订阅者的消息
                    for (idx, (topic, subscriber)) in subscribers.iter().enumerate() {
                        while let Some(sample) = subscriber.receive()? {
                            msg_counts[idx] += 1;
                            let data = sample.payload();
                            debug!("Received message #{} on topic {}, raw size: {} bytes", msg_counts[idx], topic, data.len());
                            debug!("First 50 bytes: {:?}", &data[..data.len().min(50)]);
                            
                            // 解析信号
                            if data.len() >= 4 {
                                let signal_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                let expected_len = match signal_type {
                                    0 => 44, // AdaptiveSpreadDeviation
                                    1 => 36, // FixedSpreadDeviation  
                                    2 => 32, // FundingRateDirection
                                    3 => 40, // RealTimeFundingRisk
                                    _ => {
                                        error!("Unknown signal type: {}", signal_type);
                                        continue;
                                    }
                                };
                                
                                if data.len() >= expected_len {
                                    debug!("Attempting to deserialize {} bytes for signal type {}", expected_len, signal_type);
                                    match Signal::from_bytes(Bytes::copy_from_slice(&data[..expected_len])) {
                                        Ok(signal) => {
                                            info!("Successfully deserialized signal from topic {}", topic);
                                            let msg = SignalMessage {
                                                signal,
                                                source: topic.clone(),
                                                timestamp: Utc::now(),
                                            };
                                            
                                            if let Err(e) = tx.blocking_send(msg) {
                                                error!("Failed to send signal: {}", e);
                                                return Ok(());
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to deserialize signal from {}: {}", topic, e);
                                            error!("Data that failed (hex): {:02x?}", &data[..expected_len.min(50)]);
                                        }
                                    }
                                } else {
                                    warn!("Not enough data for signal type {}: have {} bytes, need {} bytes", signal_type, data.len(), expected_len);
                                }
                            } else {
                                warn!("Received message too small: {} bytes", data.len());
                            }
                        }
                    }
                }
                NodeEvent::TerminationRequest | NodeEvent::InterruptSignal => {
                    info!("Subscriber received termination signal");
                    break;
                }
            }
        }
        
        info!("Subscriber thread shutting down");
        Ok(())
    }
}

pub struct ZmqSubscriber;

impl ZmqSubscriber {
    pub fn spawn_subscribers(tx: mpsc::Sender<SignalMessage>, endpoints: Vec<String>) {
        info!("Starting ZMQ subscribers for endpoints: {:?}", endpoints);
        
        let ctx = zmq::Context::new();
        
        for endpoint in endpoints {
            let socket = match ctx.socket(zmq::SUB) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create ZMQ socket: {}", e);
                    continue;
                }
            };
            
            if let Err(e) = socket.connect(&endpoint) {
                error!("Failed to connect to {}: {}", endpoint, e);
                continue;
            }
            
            if let Err(e) = socket.set_subscribe(b"") {
                error!("Failed to subscribe: {}", e);
                continue;
            }
            
            let tx = tx.clone();
            
            std::thread::spawn(move || {
                info!("ZMQ subscriber ready for endpoint: {}", endpoint);
                loop {
                    let msg = match socket.recv_bytes(0) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            error!("ZMQ receive error: {}", e);
                            continue;
                        }
                    };
                    
                    match Signal::from_bytes(Bytes::from(msg)) {
                        Ok(signal) => {
                            let signal_msg = SignalMessage {
                                signal,
                                source: endpoint.clone(),
                                timestamp: Utc::now(),
                            };
                            
                            if let Err(e) = tx.blocking_send(signal_msg) {
                                error!("Failed to send signal: {}", e);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("Failed to deserialize signal from {}: {}", endpoint, e);
                        }
                    }
                }
            });
        }
    }
}