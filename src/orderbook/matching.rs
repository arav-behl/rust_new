use std::sync::Arc;
use std::time::Instant;

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::RwLock;
use rust_decimal::Decimal;

use crate::types::{orders::*, execution::*, Symbol, Price as TypePrice, Quantity};
use crate::utils::latency::LatencyDistribution;
use crate::engine::PriceLevel;
use crate::Result;
use super::book::OrderBook;

#[derive(Debug, Clone)]
pub enum MatchingMessage {
    NewOrder(Order),
    CancelOrder(OrderId, String), // order_id, client_id
    ModifyOrder(OrderId, Decimal, Option<Decimal>), // order_id, new_quantity, new_price
    GetOrderStatus(OrderId),
    GetMarketData(Symbol),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum MatchingResponse {
    ExecutionReports(Vec<ExecutionReport>),
    OrderStatus(Option<Order>),
    MarketData(Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>), // bids, asks
    Error(String),
    Acknowledgment,
}

pub struct MatchingEngine {
    orderbooks: Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
    message_sender: Sender<MatchingMessage>,
    response_receiver: Receiver<MatchingResponse>,
    metrics: Arc<AtomicMetrics>,
    latency_tracker: SharedLatencyTracker,
}

impl MatchingEngine {
    pub fn new() -> Self {
        let (msg_tx, msg_rx) = unbounded::<MatchingMessage>();
        let (resp_tx, resp_rx) = unbounded::<MatchingResponse>();

        let orderbooks = Arc::new(RwLock::new(dashmap::DashMap::new()));
        let metrics = Arc::new(AtomicMetrics::new());
        let latency_tracker = SharedLatencyTracker::new(100000); // Track up to 100k samples

        // Spawn the matching engine worker
        let engine_orderbooks = Arc::clone(&orderbooks);
        let engine_metrics = Arc::clone(&metrics);
        let engine_latency_tracker = latency_tracker.clone();

        tokio::spawn(async move {
            Self::run_matching_loop(
                msg_rx,
                resp_tx,
                engine_orderbooks,
                engine_metrics,
                engine_latency_tracker,
            ).await;
        });

        Self {
            orderbooks,
            message_sender: msg_tx,
            response_receiver: resp_rx,
            metrics,
            latency_tracker,
        }
    }

    pub async fn submit_order(&self, order: Order) -> Result<Vec<ExecutionReport>> {
        let measurement = LatencyMeasurement::start("submit_order_total");

        self.message_sender
            .send(MatchingMessage::NewOrder(order))
            .map_err(|e| crate::EngineError::Internal(format!("Failed to send order: {}", e)))?;

        match self.response_receiver.recv() {
            Ok(MatchingResponse::ExecutionReports(reports)) => {
                self.latency_tracker.record_latency(measurement.elapsed_nanos());
                Ok(reports)
            }
            Ok(MatchingResponse::Error(err)) => {
                Err(crate::EngineError::Internal(err))
            }
            Ok(response) => {
                Err(crate::EngineError::Internal(format!("Unexpected response: {:?}", response)))
            }
            Err(e) => {
                Err(crate::EngineError::Internal(format!("Failed to receive response: {}", e)))
            }
        }
    }

    pub async fn cancel_order(&self, order_id: OrderId, client_id: String) -> Result<Option<ExecutionReport>> {
        self.message_sender
            .send(MatchingMessage::CancelOrder(order_id, client_id))
            .map_err(|e| crate::EngineError::Internal(format!("Failed to send cancel: {}", e)))?;

        match self.response_receiver.recv() {
            Ok(MatchingResponse::ExecutionReports(reports)) => {
                Ok(reports.into_iter().next())
            }
            Ok(MatchingResponse::Error(err)) => {
                Err(crate::EngineError::Internal(err))
            }
            Ok(_) => Ok(None),
            Err(e) => {
                Err(crate::EngineError::Internal(format!("Failed to receive response: {}", e)))
            }
        }
    }

    pub async fn get_order_status(&self, order_id: OrderId) -> Result<Option<Order>> {
        self.message_sender
            .send(MatchingMessage::GetOrderStatus(order_id))
            .map_err(|e| crate::EngineError::Internal(format!("Failed to send status request: {}", e)))?;

        match self.response_receiver.recv() {
            Ok(MatchingResponse::OrderStatus(order)) => Ok(order),
            Ok(MatchingResponse::Error(err)) => {
                Err(crate::EngineError::Internal(err))
            }
            Ok(_) => Ok(None),
            Err(e) => {
                Err(crate::EngineError::Internal(format!("Failed to receive response: {}", e)))
            }
        }
    }

    pub async fn get_market_data(&self, symbol: Symbol) -> Result<(Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>)> {
        self.message_sender
            .send(MatchingMessage::GetMarketData(symbol))
            .map_err(|e| crate::EngineError::Internal(format!("Failed to send market data request: {}", e)))?;

        match self.response_receiver.recv() {
            Ok(MatchingResponse::MarketData(bids, asks)) => Ok((bids, asks)),
            Ok(MatchingResponse::Error(err)) => {
                Err(crate::EngineError::Internal(err))
            }
            Ok(_) => {
                Err(crate::EngineError::Internal("Unexpected response".to_string()))
            }
            Err(e) => {
                Err(crate::EngineError::Internal(format!("Failed to receive response: {}", e)))
            }
        }
    }

    pub fn get_orderbook(&self, symbol: &Symbol) -> Option<Arc<OrderBook>> {
        self.orderbooks.read().get(symbol).map(|v| Arc::clone(&*v))
    }

    pub fn get_all_symbols(&self) -> Vec<Symbol> {
        self.orderbooks.read().iter().map(|entry| entry.key().clone()).collect()
    }

    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.get_snapshot()
    }

    pub fn get_latency_distribution(&self) -> LatencyDistribution {
        self.latency_tracker.get_distribution()
    }

    async fn run_matching_loop(
        message_receiver: Receiver<MatchingMessage>,
        response_sender: Sender<MatchingResponse>,
        orderbooks: Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
        metrics: Arc<AtomicMetrics>,
        latency_tracker: SharedLatencyTracker,
    ) {
        while let Ok(message) = message_receiver.recv() {
            let processing_start = Instant::now();

            let response = match message {
                MatchingMessage::NewOrder(order) => {
                    Self::process_new_order(order, &orderbooks, &metrics).await
                }
                MatchingMessage::CancelOrder(order_id, client_id) => {
                    Self::process_cancel_order(order_id, client_id, &orderbooks, &metrics).await
                }
                MatchingMessage::ModifyOrder(order_id, new_quantity, new_price) => {
                    Self::process_modify_order(order_id, new_quantity, new_price, &orderbooks).await
                }
                MatchingMessage::GetOrderStatus(order_id) => {
                    Self::process_order_status(order_id, &orderbooks).await
                }
                MatchingMessage::GetMarketData(symbol) => {
                    Self::process_market_data(symbol, &orderbooks).await
                }
                MatchingMessage::Shutdown => {
                    let _ = response_sender.send(MatchingResponse::Acknowledgment);
                    break;
                }
            };

            // Record processing latency
            let processing_time = processing_start.elapsed().as_nanos() as u64;
            latency_tracker.record_latency(processing_time);

            if let Err(e) = response_sender.send(response) {
                eprintln!("Failed to send response: {}", e);
                break;
            }
        }
    }

    async fn process_new_order(
        order: Order,
        orderbooks: &Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
        metrics: &Arc<AtomicMetrics>,
    ) -> MatchingResponse {
        let symbol = Symbol::new(order.symbol.0.clone());

        // Get or create orderbook for this symbol
        let orderbook = {
            let books = orderbooks.read();
            if let Some(book) = books.get(&symbol) {
                Arc::clone(&*book)
            } else {
                drop(books);
                let new_book = Arc::new(OrderBook::new(symbol.clone()));
                orderbooks.write().insert(symbol, Arc::clone(&new_book));
                new_book
            }
        };

        match orderbook.add_order(order) {
            Ok(reports) => {
                metrics.increment_orders_processed();
                MatchingResponse::ExecutionReports(reports)
            }
            Err(e) => {
                metrics.increment_orders_rejected();
                MatchingResponse::Error(format!("Failed to add order: {}", e))
            }
        }
    }

    async fn process_cancel_order(
        order_id: OrderId,
        _client_id: String,
        orderbooks: &Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
        metrics: &Arc<AtomicMetrics>,
    ) -> MatchingResponse {
        // Find the orderbook containing this order
        let books = orderbooks.read();
        for entry in books.iter() {
            let orderbook = entry.value();
            if let Some(_order) = orderbook.get_order(&order_id) {
                match orderbook.cancel_order(order_id) {
                    Ok(Some(report)) => {
                        metrics.increment_orders_cancelled();
                        return MatchingResponse::ExecutionReports(vec![report]);
                    }
                    Ok(None) => {
                        return MatchingResponse::Error("Order not found or cannot be cancelled".to_string());
                    }
                    Err(e) => {
                        return MatchingResponse::Error(format!("Failed to cancel order: {}", e));
                    }
                }
            }
        }

        MatchingResponse::Error("Order not found".to_string())
    }

    async fn process_modify_order(
        _order_id: OrderId,
        _new_quantity: Decimal,
        _new_price: Option<Decimal>,
        _orderbooks: &Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
    ) -> MatchingResponse {
        // Order modification is typically implemented as cancel + replace
        // For this MVP, we'll return an error
        MatchingResponse::Error("Order modification not implemented".to_string())
    }

    async fn process_order_status(
        order_id: OrderId,
        orderbooks: &Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
    ) -> MatchingResponse {
        let books = orderbooks.read();
        for entry in books.iter() {
            let orderbook = entry.value();
            if let Some(order) = orderbook.get_order(&order_id) {
                return MatchingResponse::OrderStatus(Some(order));
            }
        }

        MatchingResponse::OrderStatus(None)
    }

    async fn process_market_data(
        symbol: Symbol,
        orderbooks: &Arc<RwLock<dashmap::DashMap<Symbol, Arc<OrderBook>>>>,
    ) -> MatchingResponse {
        let books = orderbooks.read();
        if let Some(orderbook) = books.get(&symbol) {
            let (bids, asks) = orderbook.get_market_depth(10); // Top 10 levels
            MatchingResponse::MarketData(bids, asks)
        } else {
            MatchingResponse::Error("Symbol not found".to_string())
        }
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// Lock-free matching algorithm for maximum performance
pub struct LockFreeMatchingAlgorithm;

impl LockFreeMatchingAlgorithm {
    pub fn find_best_matches(
        incoming_order: &Order,
        bids: &std::collections::BTreeMap<ordered_float::OrderedFloat<f64>, Arc<RwLock<PriceLevel>>>,
        asks: &std::collections::BTreeMap<ordered_float::OrderedFloat<f64>, Arc<RwLock<PriceLevel>>>,
    ) -> Vec<(OrderId, Decimal, Decimal)> { // (order_id, price, quantity)
        let mut matches = Vec::new();
        let mut remaining_quantity = incoming_order.remaining_quantity();

        match incoming_order.side {
            OrderSide::Buy => {
                // Match against asks (best price first)
                for (ask_price, level_arc) in asks.iter() {
                    if remaining_quantity <= Decimal::ZERO {
                        break;
                    }

                    let ask_price_decimal = Decimal::from_f64_retain(ask_price.into_inner())
                        .unwrap_or(Decimal::ZERO);

                    if incoming_order.can_match(ask_price_decimal) {
                        let level = level_arc.read();
                        for ask_order in &level.orders {
                            if remaining_quantity <= Decimal::ZERO {
                                break;
                            }

                            let match_quantity = std::cmp::min(
                                remaining_quantity,
                                ask_order.remaining_quantity()
                            );

                            if match_quantity > Decimal::ZERO {
                                matches.push((ask_order.id, ask_price_decimal, match_quantity));
                                remaining_quantity -= match_quantity;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
            OrderSide::Sell => {
                // Match against bids (best price first)
                for (bid_price, level_arc) in bids.iter().rev() {
                    if remaining_quantity <= Decimal::ZERO {
                        break;
                    }

                    let bid_price_decimal = Decimal::from_f64_retain(bid_price.into_inner())
                        .unwrap_or(Decimal::ZERO);

                    if incoming_order.can_match(bid_price_decimal) {
                        let level = level_arc.read();
                        for bid_order in &level.orders {
                            if remaining_quantity <= Decimal::ZERO {
                                break;
                            }

                            let match_quantity = std::cmp::min(
                                remaining_quantity,
                                bid_order.remaining_quantity()
                            );

                            if match_quantity > Decimal::ZERO {
                                matches.push((bid_order.id, bid_price_decimal, match_quantity));
                                remaining_quantity -= match_quantity;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{OrderSide, OrderType};

    #[tokio::test]
    async fn test_matching_engine_basic_flow() {
        let engine = MatchingEngine::new();

        // Create a buy order
        let buy_order = Order::new(
            "client1".to_string(),
            "BTCUSDT".to_string(),
            OrderSide::Buy,
            OrderType::Limit,
            Decimal::from(1),
            Some(Decimal::from(50000)),
        );

        let reports = engine.submit_order(buy_order).await.unwrap();
        assert!(!reports.is_empty());

        // Create a matching sell order
        let sell_order = Order::new(
            "client2".to_string(),
            "BTCUSDT".to_string(),
            OrderSide::Sell,
            OrderType::Limit,
            Decimal::from(1),
            Some(Decimal::from(50000)),
        );

        let reports = engine.submit_order(sell_order).await.unwrap();
        assert!(!reports.is_empty());

        // Check market data
        let symbol = Symbol::new("BTCUSDT".to_string());
        let (bids, asks) = engine.get_market_data(symbol).await.unwrap();

        // After matching, the order book should be empty or have remaining quantity
        println!("Bids: {:?}, Asks: {:?}", bids, asks);
    }
}