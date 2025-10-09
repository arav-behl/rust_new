use crate::types::*;
use crate::utils::LatencyTracker;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_count: u32,
    pub orders: std::collections::VecDeque<Order>,
}

impl PriceLevel {
    pub fn new(price: Decimal) -> Self {
        Self {
            price,
            quantity: Decimal::ZERO,
            order_count: 0,
            orders: std::collections::VecDeque::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) {
        self.quantity += order.remaining_quantity();
        self.order_count += 1;
        self.orders.push_back(order);
    }

    pub fn remove_order(&mut self, order_id: OrderId) -> Option<Order> {
        if let Some(pos) = self.orders.iter().position(|o| o.id == order_id) {
            let order = self.orders.remove(pos).unwrap();
            self.quantity -= order.remaining_quantity();
            self.order_count -= 1;
            Some(order)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    pub fn get_total_volume(&self) -> f64 {
        self.quantity.to_string().parse().unwrap_or(0.0)
    }
}

#[derive(Debug)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: BTreeMap<Decimal, PriceLevel>, // Descending order (highest first)
    pub asks: BTreeMap<Decimal, PriceLevel>, // Ascending order (lowest first)
    pub orders: HashMap<OrderId, Order>,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) -> Vec<Order> {
        let mut matches = Vec::new();
        let mut remaining_order = order.clone();

        // Try to match the order first
        matches.extend(self.match_order(&mut remaining_order));

        // If there's remaining quantity, add to book
        if remaining_order.remaining_quantity() > Decimal::ZERO {
            self.add_to_book(remaining_order);
        }

        matches
    }

    fn match_order(&mut self, order: &mut Order) -> Vec<Order> {
        let mut matches = Vec::new();

        match order.side {
            OrderSide::Buy => {
                // Match against asks (sellers)
                while order.remaining_quantity() > Decimal::ZERO {
                    if let Some((&best_ask_price, _)) = self.asks.iter().next() {
                        if let Some(order_price) = order.price {
                            if order_price >= best_ask_price {
                                if let Some(matched_order) = self.execute_match(order, best_ask_price) {
                                    matches.push(matched_order);
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            // Market order
                            if let Some(matched_order) = self.execute_match(order, best_ask_price) {
                                matches.push(matched_order);
                            } else {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
            OrderSide::Sell => {
                // Match against bids (buyers)
                while order.remaining_quantity() > Decimal::ZERO {
                    if let Some((&best_bid_price, _)) = self.bids.iter().rev().next() {
                        if let Some(order_price) = order.price {
                            if order_price <= best_bid_price {
                                if let Some(matched_order) = self.execute_match(order, best_bid_price) {
                                    matches.push(matched_order);
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        } else {
                            // Market order
                            if let Some(matched_order) = self.execute_match(order, best_bid_price) {
                                matches.push(matched_order);
                            } else {
                                break;
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

    fn execute_match(&mut self, incoming_order: &mut Order, price: Decimal) -> Option<Order> {
        let side_book = match incoming_order.side {
            OrderSide::Buy => &mut self.asks,
            OrderSide::Sell => &mut self.bids,
        };

        if let Some(level) = side_book.get_mut(&price) {
            let match_quantity = incoming_order.remaining_quantity().min(level.quantity);

            // Update incoming order
            incoming_order.filled_quantity += match_quantity;
            if incoming_order.is_fully_filled() {
                incoming_order.status = OrderStatus::Filled;
            } else {
                incoming_order.status = OrderStatus::PartiallyFilled;
            }

            // Update price level
            level.quantity -= match_quantity;
            level.order_count -= 1;

            // Remove empty price levels
            if level.quantity == Decimal::ZERO {
                side_book.remove(&price);
            }

            // Create match result
            let mut matched_order = incoming_order.clone();
            matched_order.filled_quantity = match_quantity;
            matched_order.price = Some(price);

            Some(matched_order)
        } else {
            None
        }
    }

    fn add_to_book(&mut self, order: Order) {
        if let Some(price) = order.price {
            let remaining_qty = order.remaining_quantity();
            if remaining_qty > Decimal::ZERO {
                let side_book = match order.side {
                    OrderSide::Buy => &mut self.bids,
                    OrderSide::Sell => &mut self.asks,
                };

                let level = side_book.entry(price).or_insert(PriceLevel {
                    price,
                    quantity: Decimal::ZERO,
                    order_count: 0,
                    orders: std::collections::VecDeque::new(),
                });

                level.quantity += remaining_qty;
                level.order_count += 1;

                self.orders.insert(order.id, order);
            }
        }
    }

    pub fn get_best_bid(&self) -> Option<Decimal> {
        self.bids.iter().rev().next().map(|(price, _)| *price)
    }

    pub fn get_best_ask(&self) -> Option<Decimal> {
        self.asks.iter().next().map(|(price, _)| *price)
    }

    pub fn get_spread(&self) -> Option<Decimal> {
        match (self.get_best_bid(), self.get_best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    pub fn get_mid_price(&self) -> Option<Decimal> {
        match (self.get_best_bid(), self.get_best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct TradingEngine {
    pub order_books: Arc<RwLock<HashMap<String, OrderBook>>>,
    pub latency_tracker: Arc<RwLock<LatencyTracker>>,
    pub total_orders: Arc<RwLock<u64>>,
    pub total_matches: Arc<RwLock<u64>>,
}

impl TradingEngine {
    pub fn new() -> Self {
        let mut order_books = HashMap::new();

        // Initialize with popular crypto pairs
        order_books.insert("BTCUSDT".to_string(), OrderBook::new("BTCUSDT".to_string()));
        order_books.insert("ETHUSDT".to_string(), OrderBook::new("ETHUSDT".to_string()));
        order_books.insert("ADAUSDT".to_string(), OrderBook::new("ADAUSDT".to_string()));

        Self {
            order_books: Arc::new(RwLock::new(order_books)),
            latency_tracker: Arc::new(RwLock::new(LatencyTracker::new(10000))),
            total_orders: Arc::new(RwLock::new(0)),
            total_matches: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn process_order(&self, order: Order) -> Result<Vec<Order>, String> {
        let start_time = std::time::Instant::now();

        let mut order_books = self.order_books.write().await;
        let order_book = order_books
            .get_mut(&order.symbol)
            .ok_or_else(|| format!("Unknown symbol: {}", order.symbol))?;

        let matches = order_book.add_order(order);

        // Update statistics
        let latency = start_time.elapsed().as_nanos() as u64;
        self.latency_tracker.write().await.record_latency(latency);
        *self.total_orders.write().await += 1;
        *self.total_matches.write().await += matches.len() as u64;

        Ok(matches)
    }

    pub async fn get_order_book_snapshot(&self, symbol: &str) -> Option<(Vec<PriceLevel>, Vec<PriceLevel>)> {
        let order_books = self.order_books.read().await;
        if let Some(book) = order_books.get(symbol) {
            let bids: Vec<PriceLevel> = book.bids
                .iter()
                .rev()
                .take(10)
                .map(|(_, level)| level.clone())
                .collect();

            let asks: Vec<PriceLevel> = book.asks
                .iter()
                .take(10)
                .map(|(_, level)| level.clone())
                .collect();

            Some((bids, asks))
        } else {
            None
        }
    }
}