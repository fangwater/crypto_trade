use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub iceoryx_topics: Vec<String>,
    pub zmq_endpoints: Vec<String>,
    pub output_topic: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        // TODO: 从配置文件加载
        Ok(Self::default())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            iceoryx_topics: vec![
                "signals/adaptive_spread".to_string(),
                "signals/funding_rate".to_string(),
            ],
            zmq_endpoints: vec![
                "tcp://127.0.0.1:5555".to_string(),
                "tcp://127.0.0.1:5556".to_string(),
            ],
            output_topic: "events/trading".to_string(),
        }
    }
}