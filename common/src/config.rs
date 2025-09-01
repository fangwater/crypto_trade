use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};

/// 交易所配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExchangeConfig {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub exchange_type: String,
    pub description: String,
    pub symbols_file: String,
}

/// 符号配置
#[derive(Debug, Clone)]
pub struct SymbolConfig {
    pub id: u32,
    pub symbol: String,
    pub exchange_id: u32,
}

/// 市场配置管理器
#[derive(Debug)]
pub struct MarketConfig {
    /// 交易所配置列表
    exchanges: Vec<ExchangeConfig>,
    /// 交易所ID到交易所配置的映射
    exchange_by_id: HashMap<u32, ExchangeConfig>,
    /// 交易所名称到ID的映射
    exchange_id_by_name: HashMap<String, u32>,
    /// 每个交易所的符号列表
    symbols_by_exchange: HashMap<u32, Vec<SymbolConfig>>,
    /// 全局符号索引 (exchange_id, symbol_id) -> SymbolConfig
    symbol_index: HashMap<(u32, u32), SymbolConfig>,
}

impl MarketConfig {
    /// 从配置文件加载市场配置
    pub fn load(config_dir: &str) -> Result<Self> {
        let exchanges_file = Path::new(config_dir).join("exchanges.toml");
        let content = fs::read_to_string(&exchanges_file)
            .with_context(|| format!("Failed to read exchanges config: {:?}", exchanges_file))?;
        
        #[derive(Deserialize)]
        struct ExchangesConfig {
            exchange: Vec<ExchangeConfig>,
        }
        
        let config: ExchangesConfig = toml::from_str(&content)
            .context("Failed to parse exchanges.toml")?;
        
        let mut market_config = MarketConfig {
            exchanges: config.exchange.clone(),
            exchange_by_id: HashMap::new(),
            exchange_id_by_name: HashMap::new(),
            symbols_by_exchange: HashMap::new(),
            symbol_index: HashMap::new(),
        };
        
        // 构建交易所索引
        for exchange in &config.exchange {
            market_config.exchange_by_id.insert(exchange.id, exchange.clone());
            market_config.exchange_id_by_name.insert(exchange.name.clone(), exchange.id);
            
            // 加载符号CSV文件
            let symbols_path = Path::new(config_dir).join(&exchange.symbols_file);
            if symbols_path.exists() {
                let symbols = Self::load_symbols_csv(symbols_path.to_str().unwrap(), exchange.id)?;
                
                // 构建符号索引
                for symbol in &symbols {
                    market_config.symbol_index.insert(
                        (exchange.id, symbol.id),
                        symbol.clone()
                    );
                }
                
                market_config.symbols_by_exchange.insert(exchange.id, symbols);
            }
        }
        
        Ok(market_config)
    }
    
    /// 从CSV文件加载符号列表
    fn load_symbols_csv(file_path: &str, exchange_id: u32) -> Result<Vec<SymbolConfig>> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read symbols file: {}", file_path))?;
        
        let mut symbols = Vec::new();
        let mut lines = content.lines();
        
        // 跳过标题行
        lines.next();
        
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 2 {
                let id = parts[0].trim().parse::<u32>()
                    .with_context(|| format!("Invalid symbol ID: {}", parts[0]))?;
                let symbol = parts[1].trim().to_string();
                
                symbols.push(SymbolConfig {
                    id,
                    symbol,
                    exchange_id,
                });
            }
        }
        
        Ok(symbols)
    }
    
    /// 获取交易所信息
    pub fn get_exchange(&self, exchange_id: u32) -> Option<&ExchangeConfig> {
        self.exchange_by_id.get(&exchange_id)
    }
    
    /// 通过名称获取交易所ID
    pub fn get_exchange_id(&self, name: &str) -> Option<u32> {
        self.exchange_id_by_name.get(name).copied()
    }
    
    /// 根据ID获取交易所名称
    pub fn get_exchange_name(&self, exchange_id: u32) -> Option<String> {
        self.exchange_by_id.get(&exchange_id).map(|e| e.name.clone())
    }
    
    /// 获取交易所的所有符号
    pub fn get_symbols(&self, exchange_id: u32) -> Option<&Vec<SymbolConfig>> {
        self.symbols_by_exchange.get(&exchange_id)
    }
    
    /// 获取特定符号信息
    pub fn get_symbol(&self, exchange_id: u32, symbol_id: u32) -> Option<&SymbolConfig> {
        self.symbol_index.get(&(exchange_id, symbol_id))
    }
    
    /// 通过符号名称查找符号ID
    pub fn find_symbol_id(&self, exchange_id: u32, symbol_name: &str) -> Option<u32> {
        self.symbols_by_exchange.get(&exchange_id)
            .and_then(|symbols| {
                symbols.iter()
                    .find(|s| s.symbol == symbol_name)
                    .map(|s| s.id)
            })
    }
    
    /// 获取所有交易所列表
    pub fn get_exchanges(&self) -> &Vec<ExchangeConfig> {
        &self.exchanges
    }
    
    /// 调试输出配置信息
    pub fn debug_print(&self) {
        println!("=== Market Configuration ===");
        for exchange in &self.exchanges {
            println!("Exchange {}: {} ({}) - {}", 
                     exchange.id, exchange.name, exchange.exchange_type, exchange.description);
            
            if let Some(symbols) = self.symbols_by_exchange.get(&exchange.id) {
                println!("  Symbols: {} total", symbols.len());
                for (i, symbol) in symbols.iter().enumerate() {
                    if i < 3 {
                        println!("    {} - {}", symbol.id, symbol.symbol);
                    } else if i == 3 {
                        println!("    ... and {} more", symbols.len() - 3);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_config() {
        let config = MarketConfig::load("config").unwrap();
        
        // 验证交易所加载
        assert_eq!(config.get_exchanges().len(), 6);
        
        // 验证币安现货
        let binance_spot_id = config.get_exchange_id("binance_spot").unwrap();
        assert_eq!(binance_spot_id, 1);
        
        // 验证符号加载
        let symbols = config.get_symbols(binance_spot_id).unwrap();
        assert!(symbols.len() > 0);
        
        // 查找特定符号
        let btc_id = config.find_symbol_id(binance_spot_id, "BTCDOMUSDT");
        assert!(btc_id.is_some());
    }
}