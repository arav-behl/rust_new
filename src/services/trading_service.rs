use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use crossbeam_channel::{Receiver, Sender};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::types::{execution::*, order::*, market::*};
use crate::utils::latency::{LatencyDistribution, SharedLatencyTracker};
use crate::Result;
use rust_decimal::prelude::*;

// Type aliases for clarity
type ExecutionReport = SimpleExecutionReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperTrade {
    pub id: u64,
    pub order_id: OrderId,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Decimal,
    pub timestamp: DateTime<Utc>,
    pub exchange: String,
    pub latency_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: Decimal, // positive for long, negative for short
    pub average_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub last_update: DateTime<Utc>,
}

impl Position {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            quantity: Decimal::ZERO,
            average_price: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            last_update: Utc::now(),
        }
    }

    pub fn update_from_fill(&mut self, side: OrderSide, quantity: Decimal, price: Decimal) {
        let trade_quantity = match side {
            OrderSide::Buy => quantity,
            OrderSide::Sell => -quantity,
        };

        if self.quantity.is_zero() {
            // New position
            self.quantity = trade_quantity;
            self.average_price = price;
        } else if (self.quantity > Decimal::ZERO) == (trade_quantity > Decimal::ZERO) {
            // Adding to existing position
            let total_cost = self.quantity * self.average_price + trade_quantity * price;
            self.quantity += trade_quantity;
            if !self.quantity.is_zero() {
                self.average_price = total_cost / self.quantity;
            }
        } else {
            // Reducing or reversing position
            let reduction = trade_quantity.abs().min(self.quantity.abs());
            let realized_pnl = match side {
                OrderSide::Buy => (self.average_price - price) * reduction,
                OrderSide::Sell => (price - self.average_price) * reduction,
            };
            self.realized_pnl += realized_pnl;

            self.quantity += trade_quantity;

            if self.quantity.is_zero() {
                self.average_price = Decimal::ZERO;
            } else if self.quantity.signum() != (self.quantity - trade_quantity).signum() {
                // Position reversed
                self.average_price = price;
            }
        }

        self.last_update = Utc::now();
    }

    pub fn update_unrealized_pnl(&mut self, current_price: Decimal) {
        if !self.quantity.is_zero() {
            self.unrealized_pnl = (current_price - self.average_price) * self.quantity;
        }
        self.last_update = Utc::now();
    }

    pub fn total_pnl(&self) -> Decimal {
        self.realized_pnl + self.unrealized_pnl
    }
}

pub struct TradingService {
    orders: Arc<RwLock<HashMap<OrderId, Order>>>,
    positions: Arc<RwLock<HashMap<String, Position>>>,
    trades: Arc<RwLock<Vec<PaperTrade>>>,
    order_updates_tx: Sender<ExecutionReport>,
    market_data_rx: Receiver<MarketEvent>,
    trade_id_counter: Arc<AtomicU64>,
    latency_tracker: SharedLatencyTracker,
    current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl TradingService {
    pub fn new(
        order_updates_tx: Sender<ExecutionReport>,
        market_data_rx: Receiver<MarketEvent>,
    ) -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            positions: Arc::new(RwLock::new(HashMap::new())),
            trades: Arc::new(RwLock::new(Vec::new())),
            order_updates_tx,
            market_data_rx,
            trade_id_counter: Arc::new(AtomicU64::new(1)),
            latency_tracker: SharedLatencyTracker::new(10000),
            current_prices: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Trading service is already running".to_string()));
        }

        // Start market data processing
        let market_data_rx = self.market_data_rx.clone();
        let current_prices = Arc::clone(&self.current_prices);
        let positions = Arc::clone(&self.positions);
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            Self::market_data_loop(market_data_rx, current_prices, positions, is_running).await;
        });

        tracing::info!("Trading service started");
        Ok(())
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Trading service stopped");
    }

    async fn market_data_loop(
        market_data_rx: Receiver<MarketEvent>,
        current_prices: Arc<RwLock<HashMap<String, Decimal>>>,
        positions: Arc<RwLock<HashMap<String, Position>>>,
        is_running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        while is_running.load(Ordering::Relaxed) {
            match market_data_rx.try_recv() {
                Ok(MarketEvent::Trade(trade)) => {
                    // Update current price
                    current_prices.write().await.insert(trade.symbol.0.clone(), trade.price);

                    // Update unrealized PnL for all positions of this symbol
                    if let Some(position) = positions.write().await.get_mut(&trade.symbol.0) {
                        position.update_unrealized_pnl(trade.price);
                    }
                }
                Ok(MarketEvent::BookSnapshot(snapshot)) => {
                    // Use mid price from order book
                    if let (Some(best_bid), Some(best_ask)) = (
                        snapshot.bids.first(),
                        snapshot.asks.first(),
                    ) {
                        let mid_price = (best_bid.price + best_ask.price) / Decimal::TWO;
                        current_prices.write().await.insert(snapshot.symbol.0.clone(), mid_price);

                        if let Some(position) = positions.write().await.get_mut(&snapshot.symbol.0) {
                            position.update_unrealized_pnl(mid_price);
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

    pub async fn submit_order(&self, mut order: Order) -> Result<ExecutionReport> {
        let start_time = std::time::Instant::now();

        // Validate order
        if order.quantity <= Decimal::ZERO {
            return Err(crate::EngineError::InvalidOrder("Quantity must be positive".to_string()));
        }

        if order.order_type == OrderType::Limit && order.price.is_none() {
            return Err(crate::EngineError::InvalidOrder("Limit orders must have a price".to_string()));
        }

        // Store order
        let order_id = order.id;
        self.orders.write().await.insert(order_id, order.clone());

        // Try to fill the order immediately (paper trading simulation)
        let execution_result = self.attempt_fill(&mut order).await?;

        // Update stored order
        self.orders.write().await.insert(order_id, order.clone());

        // Record latency
        let latency_ns = start_time.elapsed().as_nanos() as u64;
        self.latency_tracker.record_latency(latency_ns);

        // Send execution report
        let execution_report = ExecutionReport {
            order_id,
            symbol: order.symbol.clone(),
            side: order.side,
            quantity: execution_result.filled_quantity,
            price: execution_result.fill_price,
            timestamp: Utc::now(),
            status: order.status,
            latency_ns,
        };

        if let Err(e) = self.order_updates_tx.try_send(execution_report.clone()) {
            tracing::warn!("Failed to send execution report: {}", e);
        }

        Ok(execution_report)
    }

    async fn attempt_fill(&self, order: &mut Order) -> Result<FillResult> {
        let current_price = self.get_current_price(&order.symbol).await?;

        // Simple paper trading logic - fill at current market price
        let can_fill = match order.order_type {
            OrderType::Market => true,
            OrderType::Limit => {
                if let Some(limit_price) = order.price {
                    match order.side {
                        OrderSide::Buy => current_price <= limit_price,
                        OrderSide::Sell => current_price >= limit_price,
                    }
                } else {
                    false
                }
            }
        };

        if can_fill {
            let fill_price = match order.order_type {
                OrderType::Market => current_price,
                OrderType::Limit => order.price.unwrap_or(current_price),
            };

            let fill_quantity = order.remaining_quantity();
            order.filled_quantity = order.quantity;
            order.status = OrderStatus::Filled;

            // Create paper trade
            let trade_id = self.trade_id_counter.fetch_add(1, Ordering::Relaxed);
            let paper_trade = PaperTrade {
                id: trade_id,
                order_id: order.id,
                symbol: order.symbol.clone(),
                side: order.side,
                quantity: fill_quantity,
                price: fill_price,
                timestamp: Utc::now(),
                exchange: "paper".to_string(),
                latency_ns: 0, // Will be set by caller
            };

            // Update position
            self.update_position(&order.symbol, order.side, fill_quantity, fill_price).await;

            // Store trade
            self.trades.write().await.push(paper_trade);

            tracing::info!("Paper trade executed: {} {} {} @ {}",
                order.side.to_string(), fill_quantity, order.symbol, fill_price);

            Ok(FillResult {
                filled_quantity: fill_quantity,
                fill_price,
            })
        } else {
            // Order not fillable at current market conditions
            order.status = OrderStatus::Pending;
            Ok(FillResult {
                filled_quantity: Decimal::ZERO,
                fill_price: current_price,
            })
        }
    }

    async fn get_current_price(&self, symbol: &str) -> Result<Decimal> {
        self.current_prices
            .read()
            .await
            .get(symbol)
            .copied()
            .ok_or_else(|| crate::EngineError::Internal(
                format!("No current price available for {}", symbol)
            ))
    }

    async fn update_position(&self, symbol: &str, side: OrderSide, quantity: Decimal, price: Decimal) {
        let mut positions = self.positions.write().await;
        let position = positions.entry(symbol.to_string())
            .or_insert_with(|| Position::new(symbol.to_string()));

        position.update_from_fill(side, quantity, price);
    }

    pub async fn get_order(&self, order_id: &OrderId) -> Option<Order> {
        self.orders.read().await.get(order_id).cloned()
    }

    pub async fn cancel_order(&self, order_id: &OrderId) -> Result<ExecutionReport> {
        let start_time = std::time::Instant::now();

        let mut orders = self.orders.write().await;
        if let Some(order) = orders.get_mut(order_id) {
            if order.status == OrderStatus::Pending {
                order.status = OrderStatus::Cancelled;

                let latency_ns = start_time.elapsed().as_nanos() as u64;
                self.latency_tracker.record_latency(latency_ns);

                let execution_report = ExecutionReport {
                    order_id: *order_id,
                    symbol: order.symbol.clone(),
                    side: order.side,
                    quantity: Decimal::ZERO,
                    price: None,
                    timestamp: Utc::now(),
                    status: OrderStatus::Cancelled,
                    latency_ns,
                };

                if let Err(e) = self.order_updates_tx.try_send(execution_report.clone()) {
                    tracing::warn!("Failed to send cancellation report: {}", e);
                }

                Ok(execution_report)
            } else {
                Err(crate::EngineError::InvalidOrder("Cannot cancel non-pending order".to_string()))
            }
        } else {
            Err(crate::EngineError::InvalidOrder("Order not found".to_string()))
        }
    }

    pub async fn get_position(&self, symbol: &str) -> Option<Position> {
        self.positions.read().await.get(symbol).cloned()
    }

    pub async fn get_all_positions(&self) -> HashMap<String, Position> {
        self.positions.read().await.clone()
    }

    pub async fn get_trades(&self, symbol: Option<&str>, limit: Option<usize>) -> Vec<PaperTrade> {
        let trades = self.trades.read().await;
        let mut filtered_trades: Vec<PaperTrade> = if let Some(sym) = symbol {
            trades.iter().filter(|t| t.symbol == sym).cloned().collect()
        } else {
            trades.clone()
        };

        // Sort by timestamp (most recent first)
        filtered_trades.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = limit {
            filtered_trades.truncate(limit);
        }

        filtered_trades
    }

    pub fn get_latency_stats(&self) -> LatencyDistribution {
        self.latency_tracker.get_distribution()
    }

    pub async fn get_portfolio_summary(&self) -> PortfolioSummary {
        let positions = self.positions.read().await;
        let mut total_value = Decimal::ZERO;
        let mut total_pnl = Decimal::ZERO;
        let mut position_count = 0;

        for position in positions.values() {
            if !position.quantity.is_zero() {
                position_count += 1;
                total_value += position.quantity * position.average_price;
                total_pnl += position.total_pnl();
            }
        }

        PortfolioSummary {
            total_value,
            total_pnl,
            position_count,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug)]
struct FillResult {
    filled_quantity: Decimal,
    fill_price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub total_value: Decimal,
    pub total_pnl: Decimal,
    pub position_count: usize,
    pub timestamp: DateTime<Utc>,
}

impl ToString for OrderSide {
    fn to_string(&self) -> String {
        match self {
            OrderSide::Buy => "BUY".to_string(),
            OrderSide::Sell => "SELL".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[tokio::test]
    async fn test_position_updates() {
        let mut position = Position::new("BTCUSDT".to_string());

        // First buy
        position.update_from_fill(OrderSide::Buy, Decimal::from(1), Decimal::from(50000));
        assert_eq!(position.quantity, Decimal::from(1));
        assert_eq!(position.average_price, Decimal::from(50000));

        // Add to position
        position.update_from_fill(OrderSide::Buy, Decimal::from(1), Decimal::from(52000));
        assert_eq!(position.quantity, Decimal::from(2));
        assert_eq!(position.average_price, Decimal::from(51000));

        // Partial sell
        position.update_from_fill(OrderSide::Sell, Decimal::from(1), Decimal::from(53000));
        assert_eq!(position.quantity, Decimal::from(1));
        assert_eq!(position.realized_pnl, Decimal::from(2000)); // (53000 - 51000) * 1
    }

    #[tokio::test]
    async fn test_paper_trading() {
        let (order_tx, _order_rx) = unbounded();
        let (market_tx, market_rx) = unbounded();

        let trading_service = TradingService::new(order_tx, market_rx);
        trading_service.start().await.unwrap();

        // Send market data first
        let market_event = MarketEvent::Trade(Trade {
            symbol: Symbol::new("BTCUSDT".to_string()),
            exchange: "binance".to_string(),
            price: Decimal::from(50000),
            quantity: Decimal::from(1),
            timestamp: Utc::now(),
            is_buyer_maker: false,
            sequence: 1,
        });
        market_tx.send(market_event).unwrap();

        // Wait for market data to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Submit market order
        let order = Order::new(
            "client1".to_string(),
            "BTCUSDT".to_string(),
            OrderSide::Buy,
            OrderType::Market,
            Decimal::from(1),
            None,
        );

        let execution_report = trading_service.submit_order(order).await.unwrap();
        assert_eq!(execution_report.status, OrderStatus::Filled);
        assert_eq!(execution_report.quantity, Decimal::from(1));

        // Check position
        let position = trading_service.get_position("BTCUSDT").await.unwrap();
        assert_eq!(position.quantity, Decimal::from(1));

        trading_service.stop();
    }
}