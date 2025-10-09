use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use ordered_float::OrderedFloat;
use parking_lot::RwLock;
use rust_decimal::Decimal;

use crate::types::{orders::*, execution::*, Symbol, Price as TypePrice, Quantity, DecimalExt};
use crate::utils::{latency::LatencyDistribution, memory::MemoryMappedRegion, sparse_vector::SparseVector};
use crate::engine::PriceLevel;
use crate::Result;

pub type Price = OrderedFloat<f64>;

#[derive(Debug)]
pub struct OrderBook {
    pub symbol: Symbol,

    // Price levels - using BTreeMap for efficient price-level iteration
    bids: Arc<RwLock<BTreeMap<Price, Arc<RwLock<PriceLevel>>>>>,
    asks: Arc<RwLock<BTreeMap<Price, Arc<RwLock<PriceLevel>>>>>,

    // Fast order lookup - using DashMap for lock-free concurrent access
    orders: Arc<DashMap<OrderId, Arc<RwLock<Order>>>>,

    // Sparse vector for memory-mapped price levels
    price_levels: Arc<RwLock<SparseVector<Arc<RwLock<PriceLevel>>>>>,

    // Performance metrics
    metrics: Arc<AtomicMetrics>,
    latency_tracker: SharedLatencyTracker,

    // Sequence tracking
    sequence: AtomicU64,
    last_update: AtomicU64,

    // Memory mapping for persistence
    memory_region: Option<Arc<RwLock<MemoryMappedRegion>>>,
}

impl OrderBook {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            bids: Arc::new(RwLock::new(BTreeMap::new())),
            asks: Arc::new(RwLock::new(BTreeMap::new())),
            orders: Arc::new(DashMap::new()),
            price_levels: Arc::new(RwLock::new(SparseVector::new(10000))), // 10k price levels
            metrics: Arc::new(AtomicMetrics::new()),
            latency_tracker: SharedLatencyTracker::new(10000),
            sequence: AtomicU64::new(0),
            last_update: AtomicU64::new(0),
            memory_region: None,
        }
    }

    pub fn with_memory_mapping(symbol: Symbol, memory_region: MemoryMappedRegion) -> Self {
        let mut book = Self::new(symbol);
        book.memory_region = Some(Arc::new(RwLock::new(memory_region)));
        book
    }

    pub fn add_order(&self, order: Order) -> Result<Vec<ExecutionReport>> {
        let measurement = LatencyMeasurement::start("add_order");
        let mut execution_reports = Vec::new();

        // Fast path: Check if this is a market order or if there's immediate matching opportunity
        let can_match_immediately = match order.side {
            OrderSide::Buy => {
                if let Some(best_ask) = self.get_best_ask_price() {
                    order.can_match(best_ask.into_inner().into())
                } else {
                    false
                }
            }
            OrderSide::Sell => {
                if let Some(best_bid) = self.get_best_bid_price() {
                    order.can_match(best_bid.into_inner().into())
                } else {
                    false
                }
            }
        };

        let order_arc = Arc::new(RwLock::new(order.clone()));
        let order_id = order.id;

        // If immediate matching is possible, do it first
        if can_match_immediately {
            let matches = self.find_matches(&order)?;
            for match_result in matches {
                execution_reports.push(self.create_execution_report(&match_result, ExecutionType::Fill)?);
                self.metrics.increment_orders_matched();
            }
        }

        // If order is not fully filled, add remaining quantity to book
        let remaining_order = order_arc.read();
        if !remaining_order.is_fully_filled() {
            self.orders.insert(order_id, Arc::clone(&order_arc));
            self.add_order_to_price_level(&*remaining_order)?;

            // Create execution report for order acceptance
            execution_reports.push(ExecutionReport {
                execution_id: ExecutionId::new(),
                order_id: remaining_order.id,
                trade_id: None,
                client_id: remaining_order.client_id.clone(),
                symbol: self.symbol.clone(),
                side: remaining_order.side,
                execution_type: ExecutionType::New,
                order_status: OrderStatus::Pending,
                price: remaining_order.price,
                quantity: remaining_order.remaining_quantity(),
                cumulative_quantity: remaining_order.filled_quantity,
                leaves_quantity: remaining_order.remaining_quantity(),
                last_price: None,
                last_quantity: None,
                average_price: remaining_order.price,
                commission: Decimal::ZERO,
                commission_asset: "USDT".to_string(),
                timestamp: Utc::now(),
                latency_micros: measurement.elapsed_micros(),
            });
        }

        // Update metrics
        self.metrics.increment_orders_processed();
        self.latency_tracker.record_latency(measurement.elapsed_nanos());
        self.sequence.fetch_add(1, Ordering::Relaxed);
        self.last_update.store(Utc::now().timestamp_nanos() as u64, Ordering::Relaxed);

        Ok(execution_reports)
    }

    pub fn cancel_order(&self, order_id: OrderId) -> Result<Option<ExecutionReport>> {
        let measurement = LatencyMeasurement::start("cancel_order");

        if let Some((_, order_arc)) = self.orders.remove(&order_id) {
            let mut order = order_arc.write();
            if order.status == OrderStatus::Pending || order.status == OrderStatus::PartiallyFilled {
                // Remove from price level
                self.remove_order_from_price_level(&*order)?;

                // Update order status
                order.status = OrderStatus::Cancelled;
                order.last_update = Utc::now();

                // Create execution report
                let execution_report = ExecutionReport {
                    execution_id: ExecutionId::new(),
                    order_id: order.id,
                    trade_id: None,
                    client_id: order.client_id.clone(),
                    symbol: self.symbol.clone(),
                    side: order.side,
                    execution_type: ExecutionType::Canceled,
                    order_status: OrderStatus::Cancelled,
                    price: order.price,
                    quantity: order.remaining_quantity(),
                    cumulative_quantity: order.filled_quantity,
                    leaves_quantity: Decimal::ZERO,
                    last_price: None,
                    last_quantity: None,
                    average_price: order.price,
                    commission: Decimal::ZERO,
                    commission_asset: "USDT".to_string(),
                    timestamp: Utc::now(),
                    latency_micros: measurement.elapsed_micros(),
                };

                self.metrics.increment_orders_cancelled();
                self.latency_tracker.record_latency(measurement.elapsed_nanos());

                Ok(Some(execution_report))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_order(&self, order_id: &OrderId) -> Option<Order> {
        self.orders.get(order_id).map(|order_arc| order_arc.read().clone())
    }

    pub fn get_best_bid_price(&self) -> Option<Price> {
        self.bids.read()
            .keys()
            .next_back()  // Get highest bid
            .copied()
    }

    pub fn get_best_ask_price(&self) -> Option<Price> {
        self.asks.read()
            .keys()
            .next()  // Get lowest ask
            .copied()
    }

    pub fn get_spread(&self) -> Option<Decimal> {
        match (self.get_best_bid_price(), self.get_best_ask_price()) {
            (Some(bid), Some(ask)) => {
                let spread = ask.into_inner() - bid.into_inner();
                Some(Decimal::from_f64_retain(spread).unwrap_or(Decimal::ZERO))
            }
            _ => None,
        }
    }

    pub fn get_market_depth(&self, levels: usize) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let bids = self.bids.read();
        let asks = self.asks.read();

        let bid_levels: Vec<(Decimal, Decimal)> = bids
            .iter()
            .rev()  // Highest to lowest
            .take(levels)
            .map(|(price, level_arc)| {
                let level = level_arc.read();
                (
                    Decimal::from_f64_retain(price.into_inner()).unwrap_or(Decimal::ZERO),
                    Decimal::from(level.get_total_volume())
                )
            })
            .collect();

        let ask_levels: Vec<(Decimal, Decimal)> = asks
            .iter()
            .take(levels)
            .map(|(price, level_arc)| {
                let level = level_arc.read();
                (
                    Decimal::from_f64_retain(price.into_inner()).unwrap_or(Decimal::ZERO),
                    Decimal::from(level.get_total_volume())
                )
            })
            .collect();

        (bid_levels, ask_levels)
    }

    pub fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        self.metrics.get_snapshot()
    }

    pub fn get_latency_distribution(&self) -> LatencyDistribution {
        self.latency_tracker.get_distribution()
    }

    fn add_order_to_price_level(&self, order: &Order) -> Result<()> {
        if let Some(price) = order.price {
            let price_key = Price::from(price.to_f64().unwrap_or(0.0));

            match order.side {
                OrderSide::Buy => {
                    let mut bids = self.bids.write();
                    let level_arc = bids.entry(price_key)
                        .or_insert_with(|| Arc::new(RwLock::new(PriceLevel::new(price))));

                    let mut level = level_arc.write();
                    level.add_order(order.clone());
                }
                OrderSide::Sell => {
                    let mut asks = self.asks.write();
                    let level_arc = asks.entry(price_key)
                        .or_insert_with(|| Arc::new(RwLock::new(PriceLevel::new(price))));

                    let mut level = level_arc.write();
                    level.add_order(order.clone());
                }
            }
        }

        Ok(())
    }

    fn remove_order_from_price_level(&self, order: &Order) -> Result<()> {
        if let Some(price) = order.price {
            let price_key = Price::from(price.to_f64().unwrap_or(0.0));

            match order.side {
                OrderSide::Buy => {
                    let mut bids = self.bids.write();
                    if let Some(level_arc) = bids.get(&price_key) {
                        let mut level = level_arc.write();
                        level.remove_order(order.id);

                        // Remove empty price level
                        if level.is_empty() {
                            drop(level);
                            bids.remove(&price_key);
                        }
                    }
                }
                OrderSide::Sell => {
                    let mut asks = self.asks.write();
                    if let Some(level_arc) = asks.get(&price_key) {
                        let mut level = level_arc.write();
                        level.remove_order(order.id);

                        // Remove empty price level
                        if level.is_empty() {
                            drop(level);
                            asks.remove(&price_key);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn find_matches(&self, incoming_order: &Order) -> Result<Vec<MatchResult>> {
        let mut matches = Vec::new();
        let mut remaining_quantity = incoming_order.remaining_quantity();

        match incoming_order.side {
            OrderSide::Buy => {
                // Match against asks (lowest price first)
                let asks = self.asks.read();
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
                                let match_result = MatchResult::new(
                                    ask_order.id,
                                    incoming_order.id,
                                    self.symbol.clone(),
                                    ask_price_decimal,
                                    match_quantity,
                                    OrderSide::Sell,
                                    incoming_order.timestamp,
                                );

                                matches.push(match_result);
                                remaining_quantity -= match_quantity;
                            }
                        }
                    } else {
                        break; // No more matching possible at this price or higher
                    }
                }
            }
            OrderSide::Sell => {
                // Match against bids (highest price first)
                let bids = self.bids.read();
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
                                let match_result = MatchResult::new(
                                    bid_order.id,
                                    incoming_order.id,
                                    self.symbol.clone(),
                                    bid_price_decimal,
                                    match_quantity,
                                    OrderSide::Buy,
                                    incoming_order.timestamp,
                                );

                                matches.push(match_result);
                                remaining_quantity -= match_quantity;
                            }
                        }
                    } else {
                        break; // No more matching possible at this price or lower
                    }
                }
            }
        }

        Ok(matches)
    }

    fn create_execution_report(&self, match_result: &MatchResult, exec_type: ExecutionType) -> Result<ExecutionReport> {
        Ok(ExecutionReport {
            execution_id: ExecutionId::new(),
            order_id: match_result.taker_order_id,
            trade_id: Some(match_result.trade_id),
            client_id: "system".to_string(), // This should come from order
            symbol: self.symbol.clone(),
            side: match_result.taker_fill.side,
            execution_type: exec_type,
            order_status: OrderStatus::PartiallyFilled, // This should be calculated
            price: Some(match_result.price),
            quantity: match_result.quantity,
            cumulative_quantity: match_result.quantity, // This should be calculated
            leaves_quantity: Decimal::ZERO, // This should be calculated
            last_price: Some(match_result.price),
            last_quantity: Some(match_result.quantity),
            average_price: Some(match_result.price),
            commission: Decimal::ZERO,
            commission_asset: "USDT".to_string(),
            timestamp: match_result.timestamp,
            latency_micros: match_result.matching_latency_nanos / 1000,
        })
    }
}