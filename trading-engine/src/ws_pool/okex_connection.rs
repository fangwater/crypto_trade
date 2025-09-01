use super::connection::{BaseConnection, ConnectionCommand, ConnectionState, WsConnectionRunner};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

pub struct OkexConnection {
    pub base: BaseConnection,
}

impl OkexConnection {
    pub fn new(base: BaseConnection) -> Self {
        Self { base }
    }
    
    async fn handle_message(&mut self, msg: &Message) -> bool {
        match msg {
            Message::Text(text) => {
                // Skip pong messages
                if text != "pong" {
                    let bytes = Bytes::from(text.as_bytes().to_vec());
                    if let Err(e) = self.base.message_tx.send(bytes) {
                        error!("Failed to send message: {}", e);
                        return true;
                    }
                    
                    self.base.update_stats(|stats| {
                        stats.total_messages += 1;
                        stats.last_message_time = Some(Instant::now());
                        stats.success_rate = (stats.total_messages as f64) / 
                            ((stats.total_messages + stats.total_errors) as f64) * 100.0;
                    });
                }
                false
            }
            Message::Binary(data) => {
                let bytes = Bytes::from(data.clone());
                if let Err(e) = self.base.message_tx.send(bytes) {
                    error!("Failed to send message: {}", e);
                    return true;
                }
                
                self.base.update_stats(|stats| {
                    stats.total_messages += 1;
                    stats.last_message_time = Some(Instant::now());
                });
                false
            }
            Message::Close(frame) => {
                warn!("Received close frame: {:?}", frame);
                true
            }
            _ => {
                debug!("Received other message type");
                false
            }
        }
    }
}

#[async_trait]
impl WsConnectionRunner for OkexConnection {
    async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            // Connect to WebSocket
            self.base.set_state(ConnectionState::Connecting);
            
            let ws_stream = match connect_async(&self.base.url).await {
                Ok((ws, _)) => ws,
                Err(e) => {
                    error!("Failed to connect: {}", e);
                    self.base.set_state(ConnectionState::Error);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };
            
            let (mut write, mut read) = ws_stream.split();
            
            // Send subscription message
            if let Err(e) = write.send(Message::Text(self.base.sub_msg.to_string())).await {
                error!("Failed to send subscription: {}", e);
                continue;
            }
            
            info!("Connected to OKEx {} {}", self.base.exchange, self.base.market_type);
            self.base.set_state(ConnectionState::Connected);
            
            // Create command channel
            let (command_tx, mut command_rx) = mpsc::unbounded_channel();
            self.base.set_command_tx(command_tx);
            
            // OKEx requires ping every 25 seconds
            let mut reset_timer = Instant::now() + Duration::from_secs(25);
            let mut waiting_pong = false;
            
            loop {
                tokio::select! {
                    // Handle shutdown signal
                    _ = self.base.shutdown_rx.changed() => {
                        let should_close = *self.base.shutdown_rx.borrow();
                        if should_close {
                            let _ = write.send(Message::Close(None)).await;
                            return Ok(());
                        }
                    }
                    
                    // Handle timeout
                    _ = time::sleep_until(reset_timer) => {
                        if waiting_pong {
                            warn!("OKEx {}: Ping timeout, reconnecting...", self.base.market_type);
                            let _ = write.send(Message::Close(None)).await;
                            break;
                        } else {
                            // Send ping message
                            if let Err(e) = write.send(Message::Text("ping".to_string())).await {
                                error!("Failed to send ping: {:?}", e);
                                break;
                            }
                            reset_timer = Instant::now() + Duration::from_secs(25);
                            waiting_pong = true;
                            debug!("Sent ping to OKEx");
                        }
                    }
                    
                    // Handle commands
                    Some(cmd) = command_rx.recv() => {
                        match cmd {
                            ConnectionCommand::SendMessage(data) => {
                                if let Err(e) = write.send(Message::Binary(data)).await {
                                    error!("Failed to send message: {}", e);
                                    break;
                                }
                            }
                            ConnectionCommand::Disconnect => {
                                let _ = write.send(Message::Close(None)).await;
                                break;
                            }
                        }
                    }
                    
                    // Handle WebSocket messages
                    Some(msg) = read.next() => {
                        match msg {
                            Ok(msg) => {
                                // Check for pong response
                                if let Message::Text(ref text) = msg {
                                    if text == "pong" && waiting_pong {
                                        waiting_pong = false;
                                        reset_timer = Instant::now() + Duration::from_secs(25);
                                        debug!("Received pong from OKEx");
                                        continue;
                                    }
                                }
                                
                                let should_break = self.handle_message(&msg).await;
                                if should_break {
                                    break;
                                }
                                
                                // Reset timer on any message if not waiting for pong
                                if !waiting_pong {
                                    reset_timer = Instant::now() + Duration::from_secs(25);
                                }
                            }
                            Err(e) => {
                                error!("WebSocket error: {:?}", e);
                                self.base.update_stats(|stats| {
                                    stats.total_errors += 1;
                                    stats.last_error_time = Some(Instant::now());
                                });
                                break;
                            }
                        }
                    }
                }
            }
            
            self.base.set_state(ConnectionState::Disconnected);
            info!("OKEx connection disconnected, will reconnect in 5 seconds");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}