use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use tracing::debug;

/// 风险指标计算器
pub struct RiskCalculator {
    // 历史数据窗口
    pnl_history: VecDeque<PnLPoint>,      // 盈亏历史
    exposure_history: VecDeque<ExposurePoint>, // 敞口历史
    
    // 窗口大小
    max_history_size: usize,
    
    // 缓存的计算结果
    cached_metrics: Option<RiskMetrics>,
    last_calculation: DateTime<Utc>,
}

/// 盈亏数据点
#[derive(Debug, Clone)]
struct PnLPoint {
    timestamp: DateTime<Utc>,
    value: Decimal,
    symbol: String,
}

/// 敞口数据点
#[derive(Debug, Clone)]
struct ExposurePoint {
    timestamp: DateTime<Utc>,
    value: Decimal,
}

/// 风险指标
#[derive(Debug, Clone)]
pub struct RiskMetrics {
    // 盈亏指标
    pub total_pnl: Decimal,           // 总盈亏
    pub daily_pnl: Decimal,           // 日盈亏
    pub win_rate: Decimal,            // 胜率
    pub profit_factor: Decimal,       // 盈亏比
    
    // 风险指标
    pub max_drawdown: Decimal,        // 最大回撤
    pub max_drawdown_duration: i64,   // 最大回撤持续时间（秒）
    pub sharpe_ratio: Decimal,        // 夏普比率
    pub sortino_ratio: Decimal,       // 索提诺比率
    
    // 敞口指标
    pub avg_exposure: Decimal,        // 平均敞口
    pub max_exposure: Decimal,        // 最大敞口
    pub current_exposure: Decimal,    // 当前敞口
    pub exposure_utilization: Decimal, // 敞口利用率
    
    // VaR指标
    pub var_95: Decimal,              // 95% VaR
    pub var_99: Decimal,              // 99% VaR
    pub cvar_95: Decimal,             // 95% CVaR (Expected Shortfall)
    
    // 计算时间
    pub calculated_at: DateTime<Utc>,
}

impl RiskMetrics {
    pub fn new() -> Self {
        Self {
            total_pnl: Decimal::ZERO,
            daily_pnl: Decimal::ZERO,
            win_rate: Decimal::ZERO,
            profit_factor: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            max_drawdown_duration: 0,
            sharpe_ratio: Decimal::ZERO,
            sortino_ratio: Decimal::ZERO,
            avg_exposure: Decimal::ZERO,
            max_exposure: Decimal::ZERO,
            current_exposure: Decimal::ZERO,
            exposure_utilization: Decimal::ZERO,
            var_95: Decimal::ZERO,
            var_99: Decimal::ZERO,
            cvar_95: Decimal::ZERO,
            calculated_at: Utc::now(),
        }
    }
    
    /// 重置日内指标
    pub fn reset_daily_metrics(&mut self) {
        self.daily_pnl = Decimal::ZERO;
        debug!("Daily metrics reset");
    }
}

impl RiskCalculator {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            pnl_history: VecDeque::with_capacity(max_history_size),
            exposure_history: VecDeque::with_capacity(max_history_size),
            max_history_size,
            cached_metrics: None,
            last_calculation: Utc::now(),
        }
    }
    
    /// 添加盈亏数据点
    pub fn add_pnl(&mut self, symbol: String, value: Decimal) {
        let point = PnLPoint {
            timestamp: Utc::now(),
            value,
            symbol,
        };
        
        self.pnl_history.push_back(point);
        
        // 保持窗口大小
        if self.pnl_history.len() > self.max_history_size {
            self.pnl_history.pop_front();
        }
        
        // 使缓存失效
        self.cached_metrics = None;
    }
    
    /// 添加敞口数据点
    pub fn add_exposure(&mut self, value: Decimal) {
        let point = ExposurePoint {
            timestamp: Utc::now(),
            value,
        };
        
        self.exposure_history.push_back(point);
        
        // 保持窗口大小
        if self.exposure_history.len() > self.max_history_size {
            self.exposure_history.pop_front();
        }
        
        // 使缓存失效
        self.cached_metrics = None;
    }
    
    /// 计算所有风险指标
    pub fn calculate_metrics(&mut self) -> RiskMetrics {
        // 检查缓存
        if let Some(ref metrics) = self.cached_metrics {
            let elapsed = Utc::now()
                .signed_duration_since(self.last_calculation)
                .num_seconds();
            
            // 缓存有效期5秒
            if elapsed < 5 {
                return metrics.clone();
            }
        }
        
        let mut metrics = RiskMetrics::new();
        
        // 计算盈亏指标
        self.calculate_pnl_metrics(&mut metrics);
        
        // 计算回撤
        self.calculate_drawdown(&mut metrics);
        
        // 计算比率指标
        self.calculate_ratios(&mut metrics);
        
        // 计算敞口指标
        self.calculate_exposure_metrics(&mut metrics);
        
        // 计算VaR
        self.calculate_var(&mut metrics);
        
        metrics.calculated_at = Utc::now();
        
        // 更新缓存
        self.cached_metrics = Some(metrics.clone());
        self.last_calculation = Utc::now();
        
        metrics
    }
    
    /// 计算盈亏指标
    fn calculate_pnl_metrics(&self, metrics: &mut RiskMetrics) {
        if self.pnl_history.is_empty() {
            return;
        }
        
        let now = Utc::now();
        let day_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
        let day_start_utc = DateTime::<Utc>::from_naive_utc_and_offset(day_start, Utc);
        
        let mut total_pnl = Decimal::ZERO;
        let mut daily_pnl = Decimal::ZERO;
        let mut wins = 0;
        let mut losses = 0;
        let mut total_profit = Decimal::ZERO;
        let mut total_loss = Decimal::ZERO;
        
        for point in &self.pnl_history {
            total_pnl += point.value;
            
            // 今日盈亏
            if point.timestamp >= day_start_utc {
                daily_pnl += point.value;
            }
            
            // 统计胜率和盈亏比
            if point.value > Decimal::ZERO {
                wins += 1;
                total_profit += point.value;
            } else if point.value < Decimal::ZERO {
                losses += 1;
                total_loss += point.value.abs();
            }
        }
        
        metrics.total_pnl = total_pnl;
        metrics.daily_pnl = daily_pnl;
        
        // 计算胜率
        let total_trades = wins + losses;
        if total_trades > 0 {
            metrics.win_rate = Decimal::from(wins) / Decimal::from(total_trades);
        }
        
        // 计算盈亏比
        if total_loss > Decimal::ZERO {
            metrics.profit_factor = total_profit / total_loss;
        }
    }
    
    /// 计算最大回撤
    fn calculate_drawdown(&self, metrics: &mut RiskMetrics) {
        if self.pnl_history.len() < 2 {
            return;
        }
        
        let mut cumulative_pnl = Decimal::ZERO;
        let mut peak = Decimal::ZERO;
        let mut max_drawdown = Decimal::ZERO;
        let mut drawdown_start: Option<DateTime<Utc>> = None;
        let mut max_duration = 0i64;
        
        for point in &self.pnl_history {
            cumulative_pnl += point.value;
            
            if cumulative_pnl > peak {
                peak = cumulative_pnl;
                drawdown_start = None;  // 新高点，结束回撤
            } else {
                let drawdown = peak - cumulative_pnl;
                
                if drawdown > max_drawdown {
                    max_drawdown = drawdown;
                }
                
                // 记录回撤开始时间
                if drawdown_start.is_none() && drawdown > Decimal::ZERO {
                    drawdown_start = Some(point.timestamp);
                }
                
                // 计算回撤持续时间
                if let Some(start) = drawdown_start {
                    let duration = point.timestamp
                        .signed_duration_since(start)
                        .num_seconds();
                    
                    if duration > max_duration {
                        max_duration = duration;
                    }
                }
            }
        }
        
        metrics.max_drawdown = max_drawdown;
        metrics.max_drawdown_duration = max_duration;
    }
    
    /// 计算夏普比率和索提诺比率
    fn calculate_ratios(&self, metrics: &mut RiskMetrics) {
        if self.pnl_history.len() < 30 {
            return;  // 数据不足
        }
        
        // 计算收益率序列
        let returns: Vec<Decimal> = self.pnl_history
            .iter()
            .map(|p| p.value)
            .collect();
        
        // 计算平均收益
        let mean_return: Decimal = returns.iter().sum::<Decimal>() / Decimal::from(returns.len());
        
        // 计算标准差
        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean_return;
                diff * diff
            })
            .sum::<Decimal>() / Decimal::from(returns.len());
        
        // 简化处理：不计算标准差，直接使用方差
        let std_dev = variance;
        
        // 夏普比率（假设无风险利率为0）
        if std_dev > Decimal::ZERO {
            metrics.sharpe_ratio = mean_return / std_dev * Decimal::from(16); // sqrt(252) ≈ 16
        }
        
        // 索提诺比率（只考虑下行风险）
        let downside_returns: Vec<Decimal> = returns
            .iter()
            .filter(|&&r| r < Decimal::ZERO)
            .copied()
            .collect();
        
        if !downside_returns.is_empty() {
            let downside_variance: Decimal = downside_returns
                .iter()
                .map(|r| r * r)
                .sum::<Decimal>() / Decimal::from(downside_returns.len());
            
            let downside_dev = downside_variance; // 使用方差代替标准差
            
            if downside_dev > Decimal::ZERO {
                metrics.sortino_ratio = mean_return / downside_dev * Decimal::from(16); // sqrt(252) ≈ 16
            }
        }
    }
    
    /// 计算敞口指标
    fn calculate_exposure_metrics(&self, metrics: &mut RiskMetrics) {
        if self.exposure_history.is_empty() {
            return;
        }
        
        let exposures: Vec<Decimal> = self.exposure_history
            .iter()
            .map(|p| p.value)
            .collect();
        
        // 平均敞口
        metrics.avg_exposure = exposures.iter().sum::<Decimal>() / Decimal::from(exposures.len());
        
        // 最大敞口
        metrics.max_exposure = exposures.iter().max().copied().unwrap_or(Decimal::ZERO);
        
        // 当前敞口
        metrics.current_exposure = self.exposure_history
            .back()
            .map(|p| p.value)
            .unwrap_or(Decimal::ZERO);
        
        // 敞口利用率（相对于最大允许敞口0.03）
        let max_allowed = Decimal::from_f64(0.03).unwrap();
        metrics.exposure_utilization = metrics.current_exposure / max_allowed;
    }
    
    /// 计算VaR（Value at Risk）
    fn calculate_var(&self, metrics: &mut RiskMetrics) {
        if self.pnl_history.len() < 100 {
            return;  // 数据不足
        }
        
        // 收集损失数据（负收益）
        let mut losses: Vec<Decimal> = self.pnl_history
            .iter()
            .map(|p| p.value)
            .filter(|&v| v < Decimal::ZERO)
            .map(|v| v.abs())
            .collect();
        
        if losses.is_empty() {
            return;
        }
        
        // 排序损失
        losses.sort();
        
        let n = losses.len();
        
        // 95% VaR
        let var95_idx = ((n as f64) * 0.95) as usize;
        metrics.var_95 = losses.get(var95_idx).copied().unwrap_or(Decimal::ZERO);
        
        // 99% VaR
        let var99_idx = ((n as f64) * 0.99) as usize;
        metrics.var_99 = losses.get(var99_idx).copied().unwrap_or(Decimal::ZERO);
        
        // CVaR (Expected Shortfall) - 95%尾部损失的平均值
        let tail_losses: Vec<Decimal> = losses[var95_idx..].to_vec();
        if !tail_losses.is_empty() {
            metrics.cvar_95 = tail_losses.iter().sum::<Decimal>() / Decimal::from(tail_losses.len());
        }
    }
    
    /// 获取风险评分（0-100）
    pub fn get_risk_score(&self, metrics: &RiskMetrics) -> u8 {
        let mut score = 100u8;
        
        // 根据各项指标扣分
        
        // 敞口利用率（权重30）
        let exposure_penalty = (metrics.exposure_utilization * Decimal::from(30))
            .to_u8()
            .unwrap_or(30)
            .min(30);
        score = score.saturating_sub(exposure_penalty);
        
        // 最大回撤（权重20）
        let drawdown_penalty = (metrics.max_drawdown * Decimal::from(100))
            .to_u8()
            .unwrap_or(20)
            .min(20);
        score = score.saturating_sub(drawdown_penalty);
        
        // 胜率（权重20）
        if metrics.win_rate < Decimal::from_f64(0.5).unwrap() {
            let win_rate_penalty = ((Decimal::from_f64(0.5).unwrap() - metrics.win_rate) 
                * Decimal::from(40))
                .to_u8()
                .unwrap_or(20)
                .min(20);
            score = score.saturating_sub(win_rate_penalty);
        }
        
        // 夏普比率（权重15）
        if metrics.sharpe_ratio < Decimal::ONE {
            score = score.saturating_sub(15);
        }
        
        // VaR（权重15）
        if metrics.var_95 > Decimal::from(100) {
            score = score.saturating_sub(15);
        }
        
        score
    }
}