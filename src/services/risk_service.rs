use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crossbeam_channel::{Receiver, Sender};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::*;
use crate::services::portfolio_service::Portfolio;
use crate::Result;
use rust_decimal::prelude::*;
use chrono::Timelike;

// Type aliases for clarity
type ExecutionReport = SimpleExecutionReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub account_id: String,
    pub max_position_size: Decimal,        // Max position size per symbol
    pub max_portfolio_value: Decimal,      // Max total portfolio value
    pub max_daily_loss: Decimal,           // Max daily loss limit
    pub max_concentration_pct: f64,        // Max % of portfolio in single position
    pub max_leverage: f64,                 // Max leverage ratio
    pub max_orders_per_minute: u32,        // Rate limiting
    pub blocked_symbols: Vec<String>,      // Symbols not allowed to trade
    pub allowed_order_types: Vec<OrderType>, // Allowed order types
    pub trading_hours_start: u32,          // Trading hours (24h format)
    pub trading_hours_end: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskCheck {
    pub check_type: RiskCheckType,
    pub passed: bool,
    pub reason: String,
    pub risk_score: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskCheckType {
    PositionSize,
    PortfolioValue,
    DailyLoss,
    Concentration,
    Leverage,
    RateLimit,
    SymbolBlocked,
    OrderType,
    TradingHours,
    MarketConditions,
    Volatility,
    Liquidity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreTradeRiskResult {
    pub allowed: bool,
    pub risk_checks: Vec<RiskCheck>,
    pub overall_risk_score: f64,
    pub warnings: Vec<String>,
    pub suggested_adjustments: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    pub alert_id: String,
    pub account_id: String,
    pub alert_type: RiskAlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskAlertType {
    PositionLimitBreach,
    LossLimitBreach,
    ConcentrationRisk,
    HighLeverage,
    LiquidityRisk,
    VolatilitySpike,
    SystemRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub account_id: String,
    pub var_1d: Decimal,           // Value at Risk 1-day
    pub var_1w: Decimal,           // Value at Risk 1-week
    pub sharpe_ratio: f64,
    pub max_drawdown: Decimal,
    pub beta: f64,                 // Market beta
    pub volatility: f64,           // Portfolio volatility
    pub correlation_btc: f64,      // Correlation with BTC
    pub leverage_ratio: f64,
    pub largest_position_pct: f64,
    pub active_positions: usize,
    pub timestamp: DateTime<Utc>,
}

pub struct RiskService {
    risk_limits: Arc<RwLock<HashMap<String, RiskLimits>>>,
    order_counts: Arc<RwLock<HashMap<String, Vec<DateTime<Utc>>>>>, // Rate limiting tracking
    daily_pnl: Arc<RwLock<HashMap<String, Decimal>>>,
    risk_alerts: Arc<RwLock<Vec<RiskAlert>>>,
    execution_reports_rx: Receiver<ExecutionReport>,
    market_data_rx: Receiver<MarketEvent>,
    risk_alerts_tx: Sender<RiskAlert>,
    current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
    volatility_data: Arc<RwLock<HashMap<String, f64>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl RiskService {
    pub fn new(
        execution_reports_rx: Receiver<ExecutionReport>,
        market_data_rx: Receiver<MarketEvent>,
        risk_alerts_tx: Sender<RiskAlert>,
    ) -> Self {
        Self {
            risk_limits: Arc::new(RwLock::new(HashMap::new())),
            order_counts: Arc::new(RwLock::new(HashMap::new())),
            daily_pnl: Arc::new(RwLock::new(HashMap::new())),
            risk_alerts: Arc::new(RwLock::new(Vec::new())),
            execution_reports_rx,
            market_data_rx,
            risk_alerts_tx,
            current_prices: Arc::new(RwLock::new(HashMap::new())),
            volatility_data: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Risk service is already running".to_string()));
        }

        // Start execution report monitoring
        let execution_reports_rx = self.execution_reports_rx.clone();
        let daily_pnl = Arc::clone(&self.daily_pnl);
        let order_counts = Arc::clone(&self.order_counts);
        let is_running_1 = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::execution_monitoring_loop(execution_reports_rx, daily_pnl, order_counts, is_running_1).await;
        });

        // Start market data monitoring
        let market_data_rx = self.market_data_rx.clone();
        let current_prices = Arc::clone(&self.current_prices);
        let volatility_data = Arc::clone(&self.volatility_data);
        let is_running_2 = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::market_monitoring_loop(market_data_rx, current_prices, volatility_data, is_running_2).await;
        });

        // Start periodic risk monitoring
        let risk_limits = Arc::clone(&self.risk_limits);
        let risk_alerts = Arc::clone(&self.risk_alerts);
        let risk_alerts_tx = self.risk_alerts_tx.clone();
        let is_running_3 = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::periodic_risk_monitoring(risk_limits, risk_alerts, risk_alerts_tx, is_running_3).await;
        });

        tracing::info!("Risk service started");
        Ok(())
    }

    pub fn stop(&self) {
        use std::sync::atomic::Ordering;
        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Risk service stopped");
    }

    async fn execution_monitoring_loop(
        execution_reports_rx: Receiver<ExecutionReport>,
        daily_pnl: Arc<RwLock<HashMap<String, Decimal>>>,
        order_counts: Arc<RwLock<HashMap<String, Vec<DateTime<Utc>>>>>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::Ordering;

        while is_running.load(Ordering::Relaxed) {
            match execution_reports_rx.try_recv() {
                Ok(execution_report) => {
                    let account_id = "default".to_string(); // In real system, extract from order

                    // Track order rate
                    let mut order_counts_write = order_counts.write().await;
                    let timestamps = order_counts_write.entry(account_id.clone())
                        .or_insert_with(Vec::new);
                    timestamps.push(execution_report.timestamp);

                    // Keep only last hour of timestamps
                    let cutoff = Utc::now() - chrono::Duration::hours(1);
                    timestamps.retain(|&t| t > cutoff);
                    drop(order_counts_write);

                    // Track daily PnL (simplified - would need fill price vs cost basis)
                    if execution_report.status == OrderStatus::Filled {
                        // This is simplified - real PnL calculation would be more complex
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tracing::warn!("Execution reports channel disconnected in risk service");
                    break;
                }
            }
        }
    }

    async fn market_monitoring_loop(
        market_data_rx: Receiver<MarketEvent>,
        current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
        volatility_data: Arc<RwLock<HashMap<String, f64>>>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::Ordering;

        while is_running.load(Ordering::Relaxed) {
            match market_data_rx.try_recv() {
                Ok(MarketEvent::Trade(trade)) => {
                    // Update current prices
                    current_prices.write().await.insert(trade.symbol.0.clone(), trade.price);

                    // Calculate simple volatility (would be more sophisticated in real system)
                    let vol_estimate = 0.02; // 2% daily volatility as placeholder
                    volatility_data.write().await.insert(trade.symbol.0.clone(), vol_estimate);
                }
                Ok(_) => {
                    // Handle other market events if needed
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tracing::warn!("Market data channel disconnected in risk service");
                    break;
                }
            }
        }
    }

    async fn periodic_risk_monitoring(
        _risk_limits: Arc<RwLock<HashMap<String, RiskLimits>>>,
        _risk_alerts: Arc<RwLock<Vec<RiskAlert>>>,
        _risk_alerts_tx: Sender<RiskAlert>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::Ordering;
        use tokio::time::{interval, Duration};

        let mut interval = interval(Duration::from_secs(30)); // Check every 30 seconds

        while is_running.load(Ordering::Relaxed) {
            interval.tick().await;

            // Perform periodic risk checks
            // This would include portfolio-wide risk monitoring
            // For now, it's a placeholder
        }
    }

    pub async fn set_risk_limits(&self, account_id: String, limits: RiskLimits) -> Result<()> {
        self.risk_limits.write().await.insert(account_id.clone(), limits);
        tracing::info!("Updated risk limits for account: {}", account_id);
        Ok(())
    }

    pub async fn get_risk_limits(&self, account_id: &str) -> Option<RiskLimits> {
        self.risk_limits.read().await.get(account_id).cloned()
    }

    pub async fn pre_trade_risk_check(
        &self,
        order: &Order,
        portfolio: &Portfolio,
    ) -> Result<PreTradeRiskResult> {
        let account_id = &order.client_id;
        let mut risk_checks = Vec::new();
        let mut warnings = Vec::new();
        let mut suggested_adjustments = Vec::new();
        let mut overall_risk_score = 0.0;

        // Get risk limits for this account
        let limits = match self.risk_limits.read().await.get(account_id).cloned() {
            Some(limits) => limits,
            None => {
                // Use default limits if none set
                self.get_default_risk_limits(account_id.clone())
            }
        };

        // Check 1: Position Size Limit
        let position_check = self.check_position_size_limit(order, portfolio, &limits).await;
        overall_risk_score += if position_check.passed { 0.0 } else { 2.0 };
        risk_checks.push(position_check);

        // Check 2: Portfolio Value Limit
        let portfolio_check = self.check_portfolio_value_limit(order, portfolio, &limits).await;
        overall_risk_score += if portfolio_check.passed { 0.0 } else { 3.0 };
        risk_checks.push(portfolio_check);

        // Check 3: Daily Loss Limit
        let loss_check = self.check_daily_loss_limit(order, portfolio, &limits).await;
        overall_risk_score += if loss_check.passed { 0.0 } else { 4.0 };
        risk_checks.push(loss_check);

        // Check 4: Concentration Risk
        let concentration_check = self.check_concentration_risk(order, portfolio, &limits).await;
        overall_risk_score += if concentration_check.passed { 0.0 } else { 2.5 };
        risk_checks.push(concentration_check);

        // Check 5: Rate Limiting
        let rate_check = self.check_rate_limit(account_id, &limits).await;
        overall_risk_score += if rate_check.passed { 0.0 } else { 1.0 };
        risk_checks.push(rate_check);

        // Check 6: Symbol Blocked
        let symbol_check = self.check_blocked_symbols(order, &limits).await;
        overall_risk_score += if symbol_check.passed { 0.0 } else { 5.0 };
        risk_checks.push(symbol_check);

        // Check 7: Order Type Allowed
        let order_type_check = self.check_allowed_order_types(order, &limits).await;
        overall_risk_score += if order_type_check.passed { 0.0 } else { 1.0 };
        risk_checks.push(order_type_check);

        // Check 8: Trading Hours
        let trading_hours_check = self.check_trading_hours(&limits).await;
        overall_risk_score += if trading_hours_check.passed { 0.0 } else { 2.0 };
        risk_checks.push(trading_hours_check);

        // Check 9: Market Conditions
        let market_check = self.check_market_conditions(order).await;
        overall_risk_score += market_check.risk_score;
        risk_checks.push(market_check);

        // Generate warnings and suggestions
        if overall_risk_score > 5.0 {
            warnings.push("High risk trade detected".to_string());
        }

        if overall_risk_score > 3.0 {
            suggested_adjustments.push("Consider reducing position size".to_string());
        }

        let allowed = risk_checks.iter().all(|check| check.passed);

        Ok(PreTradeRiskResult {
            allowed,
            risk_checks,
            overall_risk_score,
            warnings,
            suggested_adjustments,
            timestamp: Utc::now(),
        })
    }

    fn get_default_risk_limits(&self, account_id: String) -> RiskLimits {
        RiskLimits {
            account_id,
            max_position_size: Decimal::from(100000),      // $100k max position
            max_portfolio_value: Decimal::from(1000000),   // $1M max portfolio
            max_daily_loss: Decimal::from(10000),          // $10k max daily loss
            max_concentration_pct: 20.0,                   // 20% max concentration
            max_leverage: 3.0,                             // 3x max leverage
            max_orders_per_minute: 60,                     // 60 orders/minute
            blocked_symbols: vec![],                       // No blocked symbols
            allowed_order_types: vec![OrderType::Limit, OrderType::Market],
            trading_hours_start: 0,                        // 24/7 trading
            trading_hours_end: 24,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    async fn check_position_size_limit(
        &self,
        order: &Order,
        portfolio: &Portfolio,
        limits: &RiskLimits,
    ) -> RiskCheck {
        let current_position_size = portfolio.positions
            .get(&order.symbol)
            .map(|p| p.market_value.abs())
            .unwrap_or(Decimal::ZERO);

        let order_value = if let Some(price) = order.price {
            order.quantity * price
        } else {
            // For market orders, estimate with current market price
            let current_price = self.current_prices.read().await
                .get(&order.symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            order.quantity * current_price
        };

        let new_position_size = current_position_size + order_value;
        let passed = new_position_size <= limits.max_position_size;

        RiskCheck {
            check_type: RiskCheckType::PositionSize,
            passed,
            reason: if passed {
                "Position size within limits".to_string()
            } else {
                format!(
                    "Position size {} exceeds limit {}",
                    new_position_size,
                    limits.max_position_size
                )
            },
            risk_score: if passed { 0.0 } else { 2.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_portfolio_value_limit(
        &self,
        _order: &Order,
        portfolio: &Portfolio,
        limits: &RiskLimits,
    ) -> RiskCheck {
        let passed = portfolio.total_equity <= limits.max_portfolio_value;

        RiskCheck {
            check_type: RiskCheckType::PortfolioValue,
            passed,
            reason: if passed {
                "Portfolio value within limits".to_string()
            } else {
                format!(
                    "Portfolio value {} exceeds limit {}",
                    portfolio.total_equity,
                    limits.max_portfolio_value
                )
            },
            risk_score: if passed { 0.0 } else { 3.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_daily_loss_limit(
        &self,
        _order: &Order,
        portfolio: &Portfolio,
        limits: &RiskLimits,
    ) -> RiskCheck {
        let daily_loss = if portfolio.realized_pnl + portfolio.unrealized_pnl < Decimal::ZERO {
            (portfolio.realized_pnl + portfolio.unrealized_pnl).abs()
        } else {
            Decimal::ZERO
        };

        let passed = daily_loss <= limits.max_daily_loss;

        RiskCheck {
            check_type: RiskCheckType::DailyLoss,
            passed,
            reason: if passed {
                "Daily loss within limits".to_string()
            } else {
                format!(
                    "Daily loss {} exceeds limit {}",
                    daily_loss,
                    limits.max_daily_loss
                )
            },
            risk_score: if passed { 0.0 } else { 4.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_concentration_risk(
        &self,
        order: &Order,
        portfolio: &Portfolio,
        limits: &RiskLimits,
    ) -> RiskCheck {
        if portfolio.total_equity <= Decimal::ZERO {
            return RiskCheck {
                check_type: RiskCheckType::Concentration,
                passed: true,
                reason: "No portfolio value to check concentration".to_string(),
                risk_score: 0.0,
                timestamp: Utc::now(),
            };
        }

        let order_value = if let Some(price) = order.price {
            order.quantity * price
        } else {
            let current_price = self.current_prices.read().await
                .get(&order.symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            order.quantity * current_price
        };

        let concentration_pct = (order_value / portfolio.total_equity).to_f64().unwrap_or(0.0) * 100.0;
        let passed = concentration_pct <= limits.max_concentration_pct;

        RiskCheck {
            check_type: RiskCheckType::Concentration,
            passed,
            reason: if passed {
                "Position concentration within limits".to_string()
            } else {
                format!(
                    "Position concentration {:.1}% exceeds limit {:.1}%",
                    concentration_pct,
                    limits.max_concentration_pct
                )
            },
            risk_score: if passed { 0.0 } else { 2.5 },
            timestamp: Utc::now(),
        }
    }

    async fn check_rate_limit(&self, account_id: &str, limits: &RiskLimits) -> RiskCheck {
        let order_counts = self.order_counts.read().await;
        let recent_orders = order_counts.get(account_id)
            .map(|orders| {
                let cutoff = Utc::now() - chrono::Duration::minutes(1);
                orders.iter().filter(|&&t| t > cutoff).count()
            })
            .unwrap_or(0);

        let passed = recent_orders < limits.max_orders_per_minute as usize;

        RiskCheck {
            check_type: RiskCheckType::RateLimit,
            passed,
            reason: if passed {
                "Order rate within limits".to_string()
            } else {
                format!(
                    "Order rate {} exceeds limit {}",
                    recent_orders,
                    limits.max_orders_per_minute
                )
            },
            risk_score: if passed { 0.0 } else { 1.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_blocked_symbols(&self, order: &Order, limits: &RiskLimits) -> RiskCheck {
        let passed = !limits.blocked_symbols.contains(&order.symbol);

        RiskCheck {
            check_type: RiskCheckType::SymbolBlocked,
            passed,
            reason: if passed {
                "Symbol is allowed for trading".to_string()
            } else {
                format!("Symbol {} is blocked", order.symbol)
            },
            risk_score: if passed { 0.0 } else { 5.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_allowed_order_types(&self, order: &Order, limits: &RiskLimits) -> RiskCheck {
        let passed = limits.allowed_order_types.contains(&order.order_type);

        RiskCheck {
            check_type: RiskCheckType::OrderType,
            passed,
            reason: if passed {
                "Order type is allowed".to_string()
            } else {
                format!("Order type {:?} is not allowed", order.order_type)
            },
            risk_score: if passed { 0.0 } else { 1.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_trading_hours(&self, limits: &RiskLimits) -> RiskCheck {
        let current_hour = Utc::now().hour();
        let passed = current_hour >= limits.trading_hours_start && current_hour < limits.trading_hours_end;

        RiskCheck {
            check_type: RiskCheckType::TradingHours,
            passed,
            reason: if passed {
                "Within trading hours".to_string()
            } else {
                format!(
                    "Outside trading hours ({}-{})",
                    limits.trading_hours_start,
                    limits.trading_hours_end
                )
            },
            risk_score: if passed { 0.0 } else { 2.0 },
            timestamp: Utc::now(),
        }
    }

    async fn check_market_conditions(&self, order: &Order) -> RiskCheck {
        // Check volatility and liquidity conditions
        let volatility = self.volatility_data.read().await
            .get(&order.symbol)
            .copied()
            .unwrap_or(0.02); // Default 2% volatility

        let high_volatility = volatility > 0.05; // 5% threshold
        let risk_score = if high_volatility { 1.5 } else { 0.0 };

        RiskCheck {
            check_type: RiskCheckType::MarketConditions,
            passed: !high_volatility,
            reason: if high_volatility {
                format!("High volatility detected: {:.1}%", volatility * 100.0)
            } else {
                "Market conditions normal".to_string()
            },
            risk_score,
            timestamp: Utc::now(),
        }
    }

    pub async fn calculate_portfolio_risk_metrics(
        &self,
        account_id: &str,
        portfolio: &Portfolio,
    ) -> RiskMetrics {
        let mut largest_position_pct = 0.0;
        let active_positions = portfolio.positions.values()
            .filter(|p| !p.quantity.is_zero())
            .count();

        if portfolio.total_equity > Decimal::ZERO {
            for position in portfolio.positions.values() {
                if !position.quantity.is_zero() {
                    let position_pct = (position.market_value / portfolio.total_equity)
                        .to_f64().unwrap_or(0.0) * 100.0;

                    if position_pct > largest_position_pct {
                        largest_position_pct = position_pct;
                    }
                }
            }
        }

        RiskMetrics {
            account_id: account_id.to_string(),
            var_1d: portfolio.total_equity * Decimal::from_f64(0.02).unwrap_or_default(), // 2% VaR
            var_1w: portfolio.total_equity * Decimal::from_f64(0.05).unwrap_or_default(), // 5% VaR
            sharpe_ratio: 1.2, // Placeholder
            max_drawdown: Decimal::ZERO, // Would need historical data
            beta: 1.0, // Placeholder
            volatility: 0.15, // 15% annualized
            correlation_btc: 0.8, // High correlation with BTC
            leverage_ratio: 1.0, // No leverage in this simplified system
            largest_position_pct,
            active_positions,
            timestamp: Utc::now(),
        }
    }

    pub async fn get_risk_alerts(&self, account_id: Option<&str>) -> Vec<RiskAlert> {
        let alerts = self.risk_alerts.read().await;

        if let Some(account) = account_id {
            alerts.iter()
                .filter(|alert| alert.account_id == account)
                .cloned()
                .collect()
        } else {
            alerts.clone()
        }
    }

    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<()> {
        let mut alerts = self.risk_alerts.write().await;

        if let Some(alert) = alerts.iter_mut().find(|a| a.alert_id == alert_id) {
            alert.acknowledged = true;
            tracing::info!("Acknowledged risk alert: {}", alert_id);
            Ok(())
        } else {
            Err(crate::EngineError::Internal(format!("Alert not found: {}", alert_id)))
        }
    }
}

impl Default for RiskLimits {
    fn default() -> Self {
        Self {
            account_id: "default".to_string(),
            max_position_size: Decimal::from(100000),
            max_portfolio_value: Decimal::from(1000000),
            max_daily_loss: Decimal::from(10000),
            max_concentration_pct: 20.0,
            max_leverage: 3.0,
            max_orders_per_minute: 60,
            blocked_symbols: vec![],
            allowed_order_types: vec![OrderType::Limit, OrderType::Market],
            trading_hours_start: 0,
            trading_hours_end: 24,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[tokio::test]
    async fn test_risk_service() {
        let (exec_tx, exec_rx) = unbounded();
        let (market_tx, market_rx) = unbounded();
        let (alert_tx, _alert_rx) = unbounded();

        let risk_service = RiskService::new(exec_rx, market_rx, alert_tx);
        risk_service.start().await.unwrap();

        // Set custom risk limits
        let limits = RiskLimits {
            account_id: "test_account".to_string(),
            max_position_size: Decimal::from(1000),
            ..Default::default()
        };

        risk_service.set_risk_limits("test_account".to_string(), limits).await.unwrap();

        let retrieved_limits = risk_service.get_risk_limits("test_account").await.unwrap();
        assert_eq!(retrieved_limits.max_position_size, Decimal::from(1000));

        risk_service.stop();
    }
}