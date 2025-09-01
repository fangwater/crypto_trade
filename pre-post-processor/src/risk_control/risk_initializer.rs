use chrono::{Utc, TimeZone};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use tracing::{info, debug};
use prost::Message;

use common::risk_proto::risk_control::{
    RiskInitRequest, RiskInitResponse, RiskStateSummary,
    GlobalRiskConfig, SymbolInitState, InitialPosition,
    RiskRulesConfig, HistoricalData,
};
use common::types::{Signal, SignalData};

use super::risk_state::{RiskState, SymbolRiskState};
use super::risk_rules::{RiskRules, PositionRule, FrequencyRule, PnLRule, MarketRule, TimeRule};
use super::risk_calculator::RiskCalculator;

/// 风控初始化器
pub struct RiskInitializer {
    risk_state: RiskState,
    risk_rules: RiskRules,
    risk_calculator: RiskCalculator,
}

impl RiskInitializer {
    pub fn new() -> Self {
        Self {
            risk_state: RiskState::new(),
            risk_rules: RiskRules::new(),
            risk_calculator: RiskCalculator::new(10000), // 默认历史窗口大小
        }
    }
    
    /// 处理风控初始化信号
    pub fn process_init_signal(&mut self, signal: &Signal) -> Result<RiskInitResponse, String> {
        // 从信号中提取protobuf数据
        let protobuf_data = match &signal.data {
            SignalData::RiskControlInit { protobuf_data } => protobuf_data,
            _ => return Err("Invalid signal type for risk control init".to_string()),
        };
        
        // 解析protobuf消息
        let init_request = RiskInitRequest::decode(&protobuf_data[..])
            .map_err(|e| format!("Failed to decode protobuf: {}", e))?;
        
        // 执行初始化
        self.initialize_from_request(init_request)
    }
    
    /// 从protobuf请求初始化风控系统
    pub fn initialize_from_request(&mut self, request: RiskInitRequest) -> Result<RiskInitResponse, String> {
        info!("Initializing risk control system from protobuf request");
        
        // 初始化全局配置
        if let Some(global_config) = request.global_config {
            self.initialize_global_config(global_config)?;
        }
        
        // 初始化品种状态
        for symbol_state in request.symbol_states {
            self.initialize_symbol_state(symbol_state)?;
        }
        
        // 初始化持仓
        for position in request.positions {
            self.initialize_position(position)?;
        }
        
        // 初始化风控规则
        if let Some(rules_config) = request.rules_config {
            self.initialize_rules(rules_config)?;
        }
        
        // 恢复历史数据
        if let Some(historical_data) = request.historical_data {
            self.restore_historical_data(historical_data)?;
        }
        
        // 生成响应
        let response = self.create_init_response();
        
        info!("Risk control system initialized successfully");
        Ok(response)
    }
    
    /// 初始化全局配置
    fn initialize_global_config(&mut self, config: GlobalRiskConfig) -> Result<(), String> {
        debug!("Initializing global risk config");
        
        let _global_state = &mut self.risk_state.global_state;
        
        // 设置全局限制
        if config.enable_global_risk_control {
            // 这里可以根据配置设置全局状态的初始值
            // 比如设置最大敞口限制等
            debug!("Global risk control enabled");
        }
        
        // 更新风控规则中的全局限制
        self.risk_rules.update_global_limits(
            config.max_total_exposure_ratio,
            config.max_position_symbols as usize,
            config.max_daily_trades as usize,
            Decimal::from_f64(config.max_daily_loss).unwrap_or(Decimal::ZERO),
            Decimal::from_f64(config.total_capital).unwrap_or(Decimal::ZERO),
        );
        
        Ok(())
    }
    
    /// 初始化品种状态
    fn initialize_symbol_state(&mut self, state: SymbolInitState) -> Result<(), String> {
        debug!("Initializing symbol state for {}", state.symbol);
        
        let mut symbol_risk_state = SymbolRiskState::new(state.symbol.clone());
        
        // 设置初始值
        symbol_risk_state.position = Decimal::from_f64(state.position).unwrap_or(Decimal::ZERO);
        symbol_risk_state.capital_used = Decimal::from_f64(state.capital_used).unwrap_or(Decimal::ZERO);
        symbol_risk_state.pending_orders = state.pending_orders as usize;
        symbol_risk_state.daily_trades = state.daily_trades as usize;
        symbol_risk_state.realized_pnl = Decimal::from_f64(state.realized_pnl).unwrap_or(Decimal::ZERO);
        symbol_risk_state.unrealized_pnl = Decimal::from_f64(state.unrealized_pnl).unwrap_or(Decimal::ZERO);
        
        // 设置限制状态
        if state.is_restricted {
            symbol_risk_state.is_restricted = true;
            symbol_risk_state.restriction_reason = Some(state.restriction_reason.clone());
            if state.restriction_until > 0 {
                symbol_risk_state.restriction_until = Some(
                    Utc.timestamp_opt(state.restriction_until, 0).single()
                        .ok_or("Invalid restriction timestamp")?
                );
            }
        }
        
        // 处理品种特定配置
        if let Some(symbol_config) = state.symbol_config {
            // 可以将品种配置存储到规则中
            self.risk_rules.add_symbol_rule(
                state.symbol.clone(),
                Decimal::from_f64(symbol_config.max_position).unwrap_or(Decimal::ZERO),
                Decimal::from_f64(symbol_config.max_capital_used).unwrap_or(Decimal::ZERO),
                symbol_config.max_pending_orders as usize,
                symbol_config.max_trades_per_window as usize,
                symbol_config.time_window_seconds,
            );
        }
        
        // 将状态加入到风控状态中
        self.risk_state.symbol_states.insert(state.symbol, symbol_risk_state);
        
        Ok(())
    }
    
    /// 初始化持仓
    fn initialize_position(&mut self, position: InitialPosition) -> Result<(), String> {
        debug!("Initializing position for {} on {}", position.symbol, position.exchange);
        
        // 获取或创建品种状态
        let symbol_state = self.risk_state.symbol_states
            .entry(position.symbol.clone())
            .or_insert_with(|| SymbolRiskState::new(position.symbol.clone()));
        
        // 更新仓位信息
        let quantity = Decimal::from_f64(position.quantity).unwrap_or(Decimal::ZERO);
        let avg_price = Decimal::from_f64(position.avg_price).unwrap_or(Decimal::ZERO);
        
        // 根据方向更新仓位
        if position.side.to_uppercase() == "BUY" {
            symbol_state.position += quantity;
            symbol_state.capital_used += quantity * avg_price;
        } else if position.side.to_uppercase() == "SELL" {
            symbol_state.position -= quantity;
            symbol_state.capital_used -= quantity * avg_price;
        }
        
        // 更新未实现盈亏
        symbol_state.unrealized_pnl += Decimal::from_f64(position.unrealized_pnl).unwrap_or(Decimal::ZERO);
        
        Ok(())
    }
    
    /// 初始化风控规则
    fn initialize_rules(&mut self, config: RiskRulesConfig) -> Result<(), String> {
        debug!("Initializing risk rules");
        
        // 初始化仓位规则
        if let Some(position_rules) = config.position_rules {
            if position_rules.enabled {
                self.risk_rules.position_rule = Some(PositionRule {
                    max_single_position_ratio: Decimal::from_f64(position_rules.max_single_position_ratio)
                        .unwrap_or(Decimal::ZERO),
                    max_total_position_ratio: Decimal::from_f64(position_rules.max_total_position_ratio)
                        .unwrap_or(Decimal::ZERO),
                    max_correlated_position_ratio: Decimal::from_f64(position_rules.max_correlated_position_ratio)
                        .unwrap_or(Decimal::ZERO),
                });
            }
        }
        
        // 初始化频率规则
        if let Some(frequency_rules) = config.frequency_rules {
            if frequency_rules.enabled {
                self.risk_rules.frequency_rule = Some(FrequencyRule {
                    max_trades_per_minute: frequency_rules.max_trades_per_minute as usize,
                    max_trades_per_hour: frequency_rules.max_trades_per_hour as usize,
                    max_trades_per_day: frequency_rules.max_trades_per_day as usize,
                    min_trade_interval_ms: frequency_rules.min_trade_interval_ms,
                });
            }
        }
        
        // 初始化盈亏规则
        if let Some(pnl_rules) = config.pnl_rules {
            if pnl_rules.enabled {
                self.risk_rules.pnl_rule = Some(PnLRule {
                    max_daily_loss: Decimal::from_f64(pnl_rules.max_daily_loss).unwrap_or(Decimal::ZERO),
                    max_single_loss: Decimal::from_f64(pnl_rules.max_single_loss).unwrap_or(Decimal::ZERO),
                    max_consecutive_losses: pnl_rules.max_consecutive_losses as usize,
                    max_drawdown: Decimal::from_f64(pnl_rules.max_drawdown).unwrap_or(Decimal::ZERO),
                });
            }
        }
        
        // 初始化市场规则
        if let Some(market_rules) = config.market_rules {
            if market_rules.enabled {
                self.risk_rules.market_rule = Some(MarketRule {
                    max_slippage: Decimal::from_f64(market_rules.max_slippage).unwrap_or(Decimal::ZERO),
                    min_liquidity: Decimal::from_f64(market_rules.min_liquidity).unwrap_or(Decimal::ZERO),
                    max_volatility: Decimal::from_f64(market_rules.max_volatility).unwrap_or(Decimal::ZERO),
                });
            }
        }
        
        // 初始化时间规则
        if let Some(time_rules) = config.time_rules {
            if time_rules.enabled {
                let mut trading_windows = Vec::new();
                for window in time_rules.trading_windows {
                    trading_windows.push((window.start_time, window.end_time, window.weekdays));
                }
                
                self.risk_rules.time_rule = Some(TimeRule {
                    trading_windows,
                    blackout_dates: time_rules.blackout_dates
                        .into_iter()
                        .filter_map(|ts| Utc.timestamp_opt(ts, 0).single())
                        .collect(),
                });
            }
        }
        
        Ok(())
    }
    
    /// 恢复历史数据
    fn restore_historical_data(&mut self, data: HistoricalData) -> Result<(), String> {
        debug!("Restoring historical data");
        
        // 恢复盈亏记录
        for pnl_record in &data.pnl_records {
            let value = Decimal::from_f64(pnl_record.value).unwrap_or(Decimal::ZERO);
            self.risk_calculator.add_pnl(pnl_record.symbol.clone(), value);
        }
        
        // 恢复敞口记录
        for exposure_record in &data.exposure_records {
            let exposure = Decimal::from_f64(exposure_record.total_exposure).unwrap_or(Decimal::ZERO);
            self.risk_calculator.add_exposure(exposure);
        }
        
        // 更新风控状态中的指标
        self.risk_state.metrics = self.risk_calculator.calculate_metrics();
        
        info!("Restored {} PnL records and {} exposure records", 
              data.pnl_records.len(), 
              data.exposure_records.len());
        
        Ok(())
    }
    
    /// 创建初始化响应
    fn create_init_response(&self) -> RiskInitResponse {
        let summary = self.risk_state.get_summary();
        
        let state_summary = RiskStateSummary {
            total_exposure: summary.total_exposure.to_f64().unwrap_or(0.0),
            risk_level: format!("{:?}", summary.risk_level),
            active_positions: summary.active_positions as u32,
            available_capital: 0.0, // 需要根据实际情况计算
            daily_pnl: summary.daily_pnl.to_f64().unwrap_or(0.0),
            restricted_symbols: summary.restricted_symbols,
            global_restricted: summary.global_restricted,
        };
        
        RiskInitResponse {
            success: true,
            message: "Risk control system initialized successfully".to_string(),
            state_summary: Some(state_summary),
        }
    }
    
    /// 获取当前风控状态（用于其他模块访问）
    pub fn get_risk_state(&self) -> &RiskState {
        &self.risk_state
    }
    
    /// 获取可变风控状态（用于其他模块修改）
    pub fn get_risk_state_mut(&mut self) -> &mut RiskState {
        &mut self.risk_state
    }
    
    /// 获取风控规则（用于其他模块访问）
    pub fn get_risk_rules(&self) -> &RiskRules {
        &self.risk_rules
    }
}