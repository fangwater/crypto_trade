use tokio::sync::mpsc;
use anyhow::Result;
use tracing::{info, error, warn};
use common::messages::EventMessage;
use core::time::Duration;

pub struct IpcPublisher;

impl IpcPublisher {
    pub fn spawn_iceoryx_publisher(rx: mpsc::Receiver<EventMessage>, topic: String) {
        std::thread::spawn(move || {
            if let Err(e) = Self::run_publisher_thread(rx, topic) {
                error!("Publisher thread error: {}", e);
            }
        });
    }
    
    fn run_publisher_thread(mut rx: mpsc::Receiver<EventMessage>, topic: String) -> Result<()> {
        use iceoryx2::prelude::*;
        
        info!("Starting IceOryx publisher thread for topic: {}", topic);
        
        // 发布者创建独立的Node，使用简单的名称避免冲突
        let node_name = format!("pub{}", std::process::id());
        let node = NodeBuilder::new()
            .name(&NodeName::new(&node_name)?)
            .create::<ipc::Service>()?;
        
        let service_name = ServiceName::new(&topic)?;
        
        // 直接使用open_or_create
        let service = node
            .service_builder(&service_name)
            .publish_subscribe::<[u8; 4096]>()
            .open_or_create()?;
        
        info!("Service ready for topic: {}", topic);
        
        let publisher = service.publisher_builder().create()?;
        
        info!("IceOryx publisher ready for topic: {}", topic);
        
        const CYCLE_TIME: Duration = Duration::from_millis(10);
        
        loop {
            // 使用非阻塞接收，以便可以定期检查node状态
            match rx.try_recv() {
                Ok(event_msg) => {
                    let bytes = event_msg.to_bytes();
                    if bytes.len() > 4096 {
                        error!("Event message too large: {} bytes", bytes.len());
                        continue;
                    }
                    
                    // 创建一个固定大小的数组
                    let mut buffer = [0u8; 4096];
                    buffer[..bytes.len()].copy_from_slice(&bytes);
                    
                    // 使用loan模式并直接写入数据
                    match publisher.loan_uninit() {
                        Ok(sample) => {
                            let sample = sample.write_payload(buffer);
                            match sample.send() {
                                Ok(_) => {
                                    info!("Published event: seq={}, priority={:?}", 
                                        event_msg.sequence_id, 
                                        event_msg.event.priority());
                                }
                                Err(e) => {
                                    error!("Failed to send event: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to loan sample: {:?}", e);
                        }
                    }
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    // 没有消息，继续等待
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    info!("Publisher channel disconnected, shutting down");
                    break;
                }
            }
            
            // 检查node状态
            match node.wait(CYCLE_TIME) {
                NodeEvent::Tick => {
                    // 正常的时间片
                }
                NodeEvent::TerminationRequest | NodeEvent::InterruptSignal => {
                    warn!("Node received termination signal");
                    break;
                }
            }
        }
        
        info!("Publisher thread for topic {} shutting down", topic);
        Ok(())
    }
}