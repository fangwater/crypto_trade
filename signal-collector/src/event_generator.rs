use tokio::sync::mpsc;
use anyhow::Result;
use chrono::Utc;
use common::events::TradingEvent;
use common::messages::EventMessage;

pub struct EventGenerator {
    event_tx: mpsc::Sender<EventMessage>,
    sequence_id: u64,
}

impl EventGenerator {
    pub fn new(event_tx: mpsc::Sender<EventMessage>) -> Self {
        Self {
            event_tx,
            sequence_id: 0,
        }
    }

    pub async fn send_event(&mut self, event: TradingEvent) -> Result<()> {
        let message = EventMessage {
            event,
            sequence_id: self.sequence_id,
            timestamp: Utc::now(),
        };
        
        self.sequence_id += 1;
        self.event_tx.send(message).await?;
        Ok(())
    }
}