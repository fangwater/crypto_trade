use super::connection::{BaseConnection, ConnectionCommand, ConnectionState, WsConnectionRunner};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

pub struct BinanceConnection {
    pub base: BaseConnection,
    delay_interval: Duration,
    ping_interval: Duration,
}

impl BinanceConnection {
    pub fn new(base: BaseConnection) -> Self {
        Self {
            base,
            delay_interval: Duration::from_secs(5),
            ping_interval: Duration::from_secs(180),
        }
    }
    
    async fn handle_message(&mut self, msg: Message) -> bool {
        match msg {
            Message::Ping(payload) => {
                debug!("Received ping from Binance, will send pong");
                // Pong will be sent in the main loop
                false
            }
            Message::Text(text) => {
                let bytes = Bytes::from(text.into_bytes());
                if let Err(e) = self.base.message_tx.send(bytes) {
                    error!("Failed to send message: {}", e);
                    return true; // Should break
                }
                
                self.base.update_stats(|stats| {
                    stats.total_messages += 1;
                    stats.last_message_time = Some(Instant::now());
                    stats.success_rate = (stats.total_messages as f64) / 
                        ((stats.total_messages + stats.total_errors) as f64) * 100.0;
                });
                false
            }
            Message::Binary(data) => {
                let bytes = Bytes::from(data);
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
            Message::Pong(_) => {
                self.base.update_stats(|stats| {
                    stats.rtt_ms = 20.0; // Approximate RTT
                });
                false
            }
            _ => {
                debug!("Received other message type");
                false
            }
        }
    }
}

#[async_trait]
impl WsConnectionRunner for BinanceConnection {
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
            
            info!("Connected to Binance {} {}", self.base.exchange, self.base.market_type);
            self.base.set_state(ConnectionState::Connected);
            
            // Create command channel
            let (command_tx, mut command_rx) = mpsc::unbounded_channel();
            self.base.set_command_tx(command_tx);
            
            let mut ping_send_timer = Instant::now() + self.ping_interval + self.delay_interval;
            
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
                    
                    // Handle ping timeout
                    _ = time::sleep_until(ping_send_timer) => {
                        warn!("Binance {}: Ping timeout, reconnecting...", self.base.market_type);
                        let _ = write.send(Message::Close(None)).await;
                        break; // Reconnect
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
                                // Handle ping specially
                                if let Message::Ping(payload) = msg {
                                    if let Err(e) = write.send(Message::Pong(payload)).await {
                                        error!("Failed to send pong: {}", e);
                                        break;
                                    }
                                    ping_send_timer = Instant::now() + self.ping_interval + self.delay_interval;
                                } else {
                                    let should_break = self.handle_message(msg).await;
                                    if should_break {
                                        break;
                                    }
                                    // Reset ping timer on any message
                                    ping_send_timer = Instant::now() + self.ping_interval + self.delay_interval;
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
            info!("Binance connection disconnected, will reconnect in 5 seconds");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}