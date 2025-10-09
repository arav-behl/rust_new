use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crossbeam_channel::{Receiver, Sender};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::*;
use crate::Result;
use rust_decimal::prelude::*;

// Type aliases for clarity
type ExecutionReport = SimpleExecutionReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub account_id: String,
    pub positions: HashMap<String, Position>,
    pub cash_balance: Decimal,
    pub total_equity: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub margin_used: Decimal,
    pub margin_available: Decimal,
    pub last_update: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal, // positive for long, negative for short
    pub market_value: Decimal,
    pub average_cost: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub side: PositionSide,
    pub first_trade_time: DateTime<Utc>,
    pub last_trade_time: DateTime<Utc>,
    pub trade_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
    Flat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub trade_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub portfolio_value: Decimal,
    pub total_exposure: Decimal,
    pub leverage_ratio: f64,
    pub largest_position_pct: f64,
    pub var_1d_95: Decimal, // Value at Risk 1-day 95%
    pub sharpe_ratio: f64,
    pub max_drawdown: Decimal,
    pub positions_count: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub account_id: String,
    pub total_value: Decimal,
    pub cash_balance: Decimal,
    pub total_pnl: Decimal,
    pub day_pnl: Decimal,
    pub positions_count: usize,
    pub buying_power: Decimal,
    pub margin_utilization: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioHistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub total_value: Decimal,
    pub total_pnl: Decimal,
    pub cash_balance: Decimal,
    pub positions_count: usize,
}

pub struct PortfolioService {
    portfolios: Arc<RwLock<HashMap<String, Portfolio>>>,
    execution_reports_rx: Receiver<ExecutionReport>,
    market_data_rx: Receiver<MarketEvent>,
    position_updates_tx: Sender<PositionUpdate>,
    current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
    portfolio_history: Arc<RwLock<Vec<PortfolioHistoryEntry>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl PortfolioService {
    pub fn new(
        execution_reports_rx: Receiver<ExecutionReport>,
        market_data_rx: Receiver<MarketEvent>,
        position_updates_tx: Sender<PositionUpdate>,
    ) -> Self {
        Self {
            portfolios: Arc::new(RwLock::new(HashMap::new())),
            execution_reports_rx,
            market_data_rx,
            position_updates_tx,
            current_prices: Arc::new(RwLock::new(HashMap::new())),
            portfolio_history: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Portfolio service is already running".to_string()));
        }

        // Start execution report processing
        let execution_reports_rx = self.execution_reports_rx.clone();
        let portfolios = Arc::clone(&self.portfolios);
        let position_updates_tx = self.position_updates_tx.clone();
        let is_running_1 = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::execution_reports_loop(execution_reports_rx, portfolios, position_updates_tx, is_running_1).await;
        });

        // Start market data processing
        let market_data_rx = self.market_data_rx.clone();
        let current_prices = Arc::clone(&self.current_prices);
        let portfolios_2 = Arc::clone(&self.portfolios);
        let is_running_2 = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::market_data_loop(market_data_rx, current_prices, portfolios_2, is_running_2).await;
        });

        // Start periodic portfolio snapshots
        let portfolio_history = Arc::clone(&self.portfolio_history);
        let portfolios_3 = Arc::clone(&self.portfolios);
        let is_running_3 = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::snapshot_loop(portfolio_history, portfolios_3, is_running_3).await;
        });

        tracing::info!("Portfolio service started");
        Ok(())
    }

    pub fn stop(&self) {
        use std::sync::atomic::Ordering;
        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Portfolio service stopped");
    }

    async fn execution_reports_loop(
        execution_reports_rx: Receiver<ExecutionReport>,
        portfolios: Arc<RwLock<HashMap<String, Portfolio>>>,
        position_updates_tx: Sender<PositionUpdate>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::Ordering;

        while is_running.load(Ordering::Relaxed) {
            match execution_reports_rx.try_recv() {
                Ok(execution_report) => {
                    if execution_report.status == OrderStatus::Filled && execution_report.quantity > Decimal::ZERO {
                        if let Some(price) = execution_report.price {
                            let account_id = "default".to_string(); // In real system, extract from order

                            // Update portfolio
                            let mut portfolios_write = portfolios.write().await;
                            let portfolio = portfolios_write.entry(account_id.clone())
                                .or_insert_with(|| Portfolio::new(account_id));

                            Self::update_position_from_execution(portfolio, &execution_report, price).await;
                            drop(portfolios_write);

                            // Send position update
                            let position_update = PositionUpdate {
                                symbol: execution_report.symbol.clone(),
                                side: execution_report.side,
                                quantity: execution_report.quantity,
                                price,
                                timestamp: execution_report.timestamp,
                                trade_id: Some(format!("trade_{}", execution_report.order_id.0)),
                            };

                            if let Err(e) = position_updates_tx.try_send(position_update) {
                                tracing::warn!("Failed to send position update: {}", e);
                            }
                        }
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tracing::warn!("Execution reports channel disconnected");
                    break;
                }
            }
        }
    }

    async fn market_data_loop(
        market_data_rx: Receiver<MarketEvent>,
        current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
        portfolios: Arc<RwLock<HashMap<String, Portfolio>>>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::Ordering;

        while is_running.load(Ordering::Relaxed) {
            match market_data_rx.try_recv() {
                Ok(MarketEvent::Trade(trade)) => {
                    // Update current price
                    current_prices.write().await.insert(trade.symbol.0.clone(), trade.price);

                    // Update portfolio positions with new market prices
                    let mut portfolios_write = portfolios.write().await;
                    for portfolio in portfolios_write.values_mut() {
                        if let Some(position) = portfolio.positions.get_mut(&trade.symbol.0) {
                            Self::update_position_market_value(position, trade.price);
                        }
                        portfolio.calculate_totals();
                    }
                }
                Ok(MarketEvent::BookSnapshot(snapshot)) => {
                    // Use mid price
                    if let (Some(best_bid), Some(best_ask)) = (
                        snapshot.bids.first(),
                        snapshot.asks.first(),
                    ) {
                        let mid_price = (best_bid.price + best_ask.price) / Decimal::TWO;
                        current_prices.write().await.insert(snapshot.symbol.0.clone(), mid_price);

                        let mut portfolios_write = portfolios.write().await;
                        for portfolio in portfolios_write.values_mut() {
                            if let Some(position) = portfolio.positions.get_mut(&snapshot.symbol.0) {
                                Self::update_position_market_value(position, mid_price);
                            }
                            portfolio.calculate_totals();
                        }
                    }
                }
                Ok(_) => {
                    // Handle other market events if needed
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tracing::warn!("Market data channel disconnected");
                    break;
                }
            }
        }
    }

    async fn snapshot_loop(
        portfolio_history: Arc<RwLock<Vec<PortfolioHistoryEntry>>>,
        portfolios: Arc<RwLock<HashMap<String, Portfolio>>>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        use std::sync::atomic::Ordering;
        use tokio::time::{interval, Duration};

        let mut interval = interval(Duration::from_secs(60)); // Snapshot every minute

        while is_running.load(Ordering::Relaxed) {
            interval.tick().await;

            let portfolios_read = portfolios.read().await;
            let mut history_write = portfolio_history.write().await;

            for portfolio in portfolios_read.values() {
                let entry = PortfolioHistoryEntry {
                    timestamp: Utc::now(),
                    total_value: portfolio.total_equity,
                    total_pnl: portfolio.realized_pnl + portfolio.unrealized_pnl,
                    cash_balance: portfolio.cash_balance,
                    positions_count: portfolio.positions.len(),
                };

                history_write.push(entry);
            }

            // Keep only last 1440 entries (24 hours of minute snapshots)
            if history_write.len() > 1440 {
                history_write.drain(0..(history_write.len() - 1440));
            }
        }
    }

    async fn update_position_from_execution(
        portfolio: &mut Portfolio,
        execution: &ExecutionReport,
        fill_price: Decimal,
    ) {
        let position = portfolio.positions.entry(execution.symbol.clone())
            .or_insert_with(|| Position::new(execution.symbol.clone()));

        let trade_value = execution.quantity * fill_price;
        let is_buy = execution.side == OrderSide::Buy;

        // Update position quantity and average cost
        if position.quantity.is_zero() {
            // New position
            position.quantity = if is_buy { execution.quantity } else { -execution.quantity };
            position.average_cost = fill_price;
            position.market_value = trade_value;
            position.side = if is_buy { PositionSide::Long } else { PositionSide::Short };
            position.first_trade_time = execution.timestamp;
        } else {
            // Existing position
            let old_quantity = position.quantity;
            let new_quantity_delta = if is_buy { execution.quantity } else { -execution.quantity };

            if (old_quantity > Decimal::ZERO) == (new_quantity_delta > Decimal::ZERO) {
                // Adding to position
                let total_cost = position.quantity * position.average_cost + trade_value;
                position.quantity += new_quantity_delta;
                if !position.quantity.is_zero() {
                    position.average_cost = total_cost / position.quantity.abs();
                }
            } else {
                // Reducing or reversing position
                let reduction_quantity = new_quantity_delta.abs().min(old_quantity.abs());
                let realized_pnl = if is_buy {
                    (position.average_cost - fill_price) * reduction_quantity
                } else {
                    (fill_price - position.average_cost) * reduction_quantity
                };

                position.realized_pnl += realized_pnl;
                portfolio.realized_pnl += realized_pnl;

                position.quantity += new_quantity_delta;

                if position.quantity.is_zero() {
                    position.side = PositionSide::Flat;
                    position.average_cost = Decimal::ZERO;
                } else if position.quantity.signum() != old_quantity.signum() {
                    // Position reversed
                    position.average_cost = fill_price;
                    position.side = if position.quantity > Decimal::ZERO {
                        PositionSide::Long
                    } else {
                        PositionSide::Short
                    };
                }
            }
        }

        position.last_trade_time = execution.timestamp;
        position.trade_count += 1;

        // Update cash balance
        let cash_impact = if is_buy { -trade_value } else { trade_value };
        portfolio.cash_balance += cash_impact;

        portfolio.last_update = execution.timestamp;
    }

    fn update_position_market_value(position: &mut Position, current_price: Decimal) {
        if !position.quantity.is_zero() {
            position.market_value = position.quantity.abs() * current_price;
            position.unrealized_pnl = (current_price - position.average_cost) * position.quantity;
        }
    }

    pub async fn get_portfolio(&self, account_id: &str) -> Option<Portfolio> {
        self.portfolios.read().await.get(account_id).cloned()
    }

    pub async fn get_portfolio_summary(&self, account_id: &str) -> Option<PortfolioSummary> {
        let portfolios = self.portfolios.read().await;
        portfolios.get(account_id).map(|portfolio| {
            let total_pnl = portfolio.realized_pnl + portfolio.unrealized_pnl;

            PortfolioSummary {
                account_id: account_id.to_string(),
                total_value: portfolio.total_equity,
                cash_balance: portfolio.cash_balance,
                total_pnl,
                day_pnl: total_pnl, // Simplified - would need day start tracking in real system
                positions_count: portfolio.positions.len(),
                buying_power: portfolio.margin_available,
                margin_utilization: if portfolio.margin_available > Decimal::ZERO {
                    (portfolio.margin_used / (portfolio.margin_used + portfolio.margin_available)).to_f64().unwrap_or(0.0)
                } else {
                    0.0
                },
                timestamp: Utc::now(),
            }
        })
    }

    pub async fn get_position(&self, account_id: &str, symbol: &str) -> Option<Position> {
        self.portfolios
            .read()
            .await
            .get(account_id)?
            .positions
            .get(symbol)
            .cloned()
    }

    pub async fn get_all_positions(&self, account_id: &str) -> HashMap<String, Position> {
        self.portfolios
            .read()
            .await
            .get(account_id)
            .map(|p| p.positions.clone())
            .unwrap_or_default()
    }

    pub async fn calculate_risk_metrics(&self, account_id: &str) -> Option<RiskMetrics> {
        let portfolios = self.portfolios.read().await;
        let portfolio = portfolios.get(account_id)?;

        let mut total_exposure = Decimal::ZERO;
        let mut largest_position_value = Decimal::ZERO;

        for position in portfolio.positions.values() {
            if !position.quantity.is_zero() {
                let position_exposure = position.market_value;
                total_exposure += position_exposure;

                if position_exposure > largest_position_value {
                    largest_position_value = position_exposure;
                }
            }
        }

        let portfolio_value = portfolio.total_equity;
        let leverage_ratio = if portfolio_value > Decimal::ZERO {
            (total_exposure / portfolio_value).to_f64().unwrap_or(0.0)
        } else {
            0.0
        };

        let largest_position_pct = if portfolio_value > Decimal::ZERO {
            (largest_position_value / portfolio_value).to_f64().unwrap_or(0.0) * 100.0
        } else {
            0.0
        };

        Some(RiskMetrics {
            portfolio_value,
            total_exposure,
            leverage_ratio,
            largest_position_pct,
            var_1d_95: portfolio_value * Decimal::from_f64(0.02).unwrap_or_default(), // Simplified VaR
            sharpe_ratio: 1.5, // Placeholder - would need returns history
            max_drawdown: Decimal::ZERO, // Placeholder - would need historical analysis
            positions_count: portfolio.positions.len(),
            timestamp: Utc::now(),
        })
    }

    pub async fn get_portfolio_history(&self, limit: Option<usize>) -> Vec<PortfolioHistoryEntry> {
        let history = self.portfolio_history.read().await;
        let mut result = history.clone();

        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Most recent first

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        result
    }

    pub async fn create_portfolio(&self, account_id: String, initial_cash: Decimal) -> Result<()> {
        let mut portfolios = self.portfolios.write().await;

        if portfolios.contains_key(&account_id) {
            return Err(crate::EngineError::Internal(
                format!("Portfolio already exists for account: {}", account_id)
            ));
        }

        let mut portfolio = Portfolio::new(account_id.clone());
        portfolio.cash_balance = initial_cash;
        portfolio.total_equity = initial_cash;
        portfolio.margin_available = initial_cash;

        portfolios.insert(account_id, portfolio);

        tracing::info!("Created new portfolio with {} initial cash", initial_cash);
        Ok(())
    }
}

impl Portfolio {
    pub fn new(account_id: String) -> Self {
        Self {
            account_id,
            positions: HashMap::new(),
            cash_balance: Decimal::ZERO,
            total_equity: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            margin_used: Decimal::ZERO,
            margin_available: Decimal::ZERO,
            last_update: Utc::now(),
        }
    }

    pub fn calculate_totals(&mut self) {
        let mut total_unrealized_pnl = Decimal::ZERO;
        let mut total_market_value = Decimal::ZERO;

        for position in self.positions.values() {
            if !position.quantity.is_zero() {
                total_unrealized_pnl += position.unrealized_pnl;
                total_market_value += position.market_value;
            }
        }

        self.unrealized_pnl = total_unrealized_pnl;
        self.total_equity = self.cash_balance + total_market_value;
        self.last_update = Utc::now();
    }
}

impl Position {
    pub fn new(symbol: String) -> Self {
        let now = Utc::now();
        Self {
            symbol,
            quantity: Decimal::ZERO,
            market_value: Decimal::ZERO,
            average_cost: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            side: PositionSide::Flat,
            first_trade_time: now,
            last_trade_time: now,
            trade_count: 0,
        }
    }

    pub fn total_pnl(&self) -> Decimal {
        self.realized_pnl + self.unrealized_pnl
    }

    pub fn is_profitable(&self) -> bool {
        self.total_pnl() > Decimal::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[tokio::test]
    async fn test_portfolio_creation() {
        let (exec_tx, exec_rx) = unbounded();
        let (market_tx, market_rx) = unbounded();
        let (pos_tx, _pos_rx) = unbounded();

        let portfolio_service = PortfolioService::new(exec_rx, market_rx, pos_tx);

        portfolio_service.create_portfolio(
            "test_account".to_string(),
            Decimal::from(100000)
        ).await.unwrap();

        let summary = portfolio_service.get_portfolio_summary("test_account").await.unwrap();
        assert_eq!(summary.total_value, Decimal::from(100000));
        assert_eq!(summary.cash_balance, Decimal::from(100000));
    }

    #[tokio::test]
    async fn test_position_updates() {
        let (exec_tx, exec_rx) = unbounded();
        let (market_tx, market_rx) = unbounded();
        let (pos_tx, _pos_rx) = unbounded();

        let portfolio_service = PortfolioService::new(exec_rx, market_rx, pos_tx);
        portfolio_service.start().await.unwrap();

        portfolio_service.create_portfolio(
            "test_account".to_string(),
            Decimal::from(100000)
        ).await.unwrap();

        // Send an execution report
        let execution_report = ExecutionReport {
            order_id: OrderId::new(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            quantity: Decimal::from(1),
            price: Some(Decimal::from(50000)),
            timestamp: Utc::now(),
            status: OrderStatus::Filled,
            latency_ns: 5000,
        };

        exec_tx.send(execution_report).unwrap();

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let position = portfolio_service.get_position("test_account", "BTCUSDT").await.unwrap();
        assert_eq!(position.quantity, Decimal::from(1));
        assert_eq!(position.side, PositionSide::Long);

        portfolio_service.stop();
    }
}