use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::types::order::{Order, OrderId, OrderSide, OrderStatus, Trade};

/// Price level in the order book
/// Contains all orders at a specific price
#[derive(Debug, Clone)]
pub struct PriceLevel {
    pub price: f64,
    pub orders: VecDeque<Order>,
    pub total_quantity: f64,
}

impl PriceLevel {
    pub fn new(price: f64) -> Self {
        Self {
            price,
            orders: VecDeque::new(),
            total_quantity: 0.0,
        }
    }

    pub fn add_order(&mut self, order: Order) {
        self.total_quantity += order.remaining_quantity;
        self.orders.push_back(order);
    }

    pub fn remove_order(&mut self, order_id: OrderId) -> Option<Order> {
        if let Some(pos) = self.orders.iter().position(|o| o.id == order_id) {
            let order = self.orders.remove(pos)?;
            self.total_quantity -= order.remaining_quantity;
            Some(order)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

/// High-performance order book
/// Uses BTreeMap for price-sorted levels, inspired by Tzadiko's C++ implementation
pub struct OrderBook {
    pub symbol: String,

    // Bids: highest price first (reverse order)
    bids: BTreeMap<OrderedFloat, PriceLevel>,

    // Asks: lowest price first (natural order)
    asks: BTreeMap<OrderedFloat, PriceLevel>,

    // Fast order lookup by ID
    orders: HashMap<OrderId, OrderSide>,
}

/// Wrapper for f64 to make it Ord for BTreeMap
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedFloat(f64);

impl OrderedFloat {
    fn new(value: f64) -> Self {
        Self(value)
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
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

    /// Add an order to the book and attempt to match it
    /// Returns list of trades generated
    pub fn add_order(&mut self, order: Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut order = order;

        // Try to match the order first
        trades.extend(self.match_order(&mut order));

        // If order has remaining quantity, add to book
        if !order.is_filled() {
            self.add_order_to_book(order);
        }

        trades
    }

    /// Cancel an order from the book
    pub fn cancel_order(&mut self, order_id: OrderId) -> Option<Order> {
        // Find which side the order is on
        let side = self.orders.remove(&order_id)?;

        // Remove from appropriate side
        let levels = match side {
            OrderSide::Buy => &mut self.bids,
            OrderSide::Sell => &mut self.asks,
        };

        // Search all price levels for the order
        for (_, level) in levels.iter_mut() {
            if let Some(mut order) = level.remove_order(order_id) {
                order.status = OrderStatus::Cancelled;

                // Clean up empty levels
                if level.is_empty() {
                    let price_key = OrderedFloat::new(level.price);
                    match side {
                        OrderSide::Buy => self.bids.remove(&price_key),
                        OrderSide::Sell => self.asks.remove(&price_key),
                    };
                }

                return Some(order);
            }
        }

        None
    }

    /// Get best bid price
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next_back().map(|k| k.0)
    }

    /// Get best ask price
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|k| k.0)
    }

    /// Get bid-ask spread
    pub fn spread(&self) -> Option<f64> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get mid-market price
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some((ask + bid) / 2.0),
            _ => None,
        }
    }

    /// Get market depth (top N levels)
    pub fn get_depth(&self, levels: usize) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let bid_levels: Vec<(f64, f64)> = self
            .bids
            .iter()
            .rev()
            .take(levels)
            .map(|(_, level)| (level.price, level.total_quantity))
            .collect();

        let ask_levels: Vec<(f64, f64)> = self
            .asks
            .iter()
            .take(levels)
            .map(|(_, level)| (level.price, level.total_quantity))
            .collect();

        (bid_levels, ask_levels)
    }

    /// Get total order count
    pub fn order_count(&self) -> usize {
        self.orders.len()
    }

    // Private helper methods

    fn add_order_to_book(&mut self, order: Order) {
        let price_key = OrderedFloat::new(order.price);
        let side = order.side;

        // Track order
        self.orders.insert(order.id, side);

        // Add to appropriate side
        match side {
            OrderSide::Buy => {
                self.bids
                    .entry(price_key)
                    .or_insert_with(|| PriceLevel::new(order.price))
                    .add_order(order);
            }
            OrderSide::Sell => {
                self.asks
                    .entry(price_key)
                    .or_insert_with(|| PriceLevel::new(order.price))
                    .add_order(order);
            }
        }
    }

    fn match_order(&mut self, taker_order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();

        match taker_order.side {
            OrderSide::Buy => {
                // Match against asks (sell orders)
                self.match_against_asks(taker_order, &mut trades);
            }
            OrderSide::Sell => {
                // Match against bids (buy orders)
                self.match_against_bids(taker_order, &mut trades);
            }
        }

        trades
    }

    fn match_against_asks(&mut self, buy_order: &mut Order, trades: &mut Vec<Trade>) {
        let mut empty_levels = Vec::new();

        // Iterate through asks from lowest to highest price
        for (price_key, level) in self.asks.iter_mut() {
            if buy_order.is_filled() {
                break;
            }

            // Check if buy order price is >= ask price
            if !buy_order.can_match(price_key.0) {
                break;
            }

            // Match against orders at this price level
            while !buy_order.is_filled() && !level.orders.is_empty() {
                let maker_order = level.orders.front_mut().unwrap();

                let match_quantity = buy_order.remaining_quantity.min(maker_order.remaining_quantity);
                let match_price = maker_order.price; // Price-time priority

                // Create trade
                let trade = Trade::new(
                    maker_order.id,
                    buy_order.id,
                    self.symbol.clone(),
                    match_price,
                    match_quantity,
                );
                trades.push(trade);

                // Update quantities
                buy_order.fill(match_quantity);
                maker_order.fill(match_quantity);
                level.total_quantity -= match_quantity;

                // Remove filled orders
                if maker_order.is_filled() {
                    let filled_order = level.orders.pop_front().unwrap();
                    self.orders.remove(&filled_order.id);
                }
            }

            if level.is_empty() {
                empty_levels.push(*price_key);
            }
        }

        // Clean up empty levels
        for price_key in empty_levels {
            self.asks.remove(&price_key);
        }
    }

    fn match_against_bids(&mut self, sell_order: &mut Order, trades: &mut Vec<Trade>) {
        let mut empty_levels = Vec::new();

        // Iterate through bids from highest to lowest price
        for (price_key, level) in self.bids.iter_mut().rev() {
            if sell_order.is_filled() {
                break;
            }

            // Check if sell order price is <= bid price
            if !sell_order.can_match(price_key.0) {
                break;
            }

            // Match against orders at this price level
            while !sell_order.is_filled() && !level.orders.is_empty() {
                let maker_order = level.orders.front_mut().unwrap();

                let match_quantity = sell_order.remaining_quantity.min(maker_order.remaining_quantity);
                let match_price = maker_order.price; // Price-time priority

                // Create trade
                let trade = Trade::new(
                    maker_order.id,
                    sell_order.id,
                    self.symbol.clone(),
                    match_price,
                    match_quantity,
                );
                trades.push(trade);

                // Update quantities
                sell_order.fill(match_quantity);
                maker_order.fill(match_quantity);
                level.total_quantity -= match_quantity;

                // Remove filled orders
                if maker_order.is_filled() {
                    let filled_order = level.orders.pop_front().unwrap();
                    self.orders.remove(&filled_order.id);
                }
            }

            if level.is_empty() {
                empty_levels.push(*price_key);
            }
        }

        // Clean up empty levels
        for price_key in empty_levels {
            self.bids.remove(&price_key);
        }
    }
}

/// Thread-safe wrapper for OrderBook
pub struct SharedOrderBook {
    inner: Arc<Mutex<OrderBook>>,
}

impl SharedOrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            inner: Arc::new(Mutex::new(OrderBook::new(symbol))),
        }
    }

    pub fn add_order(&self, order: Order) -> Vec<Trade> {
        self.inner.lock().unwrap().add_order(order)
    }

    pub fn cancel_order(&self, order_id: OrderId) -> Option<Order> {
        self.inner.lock().unwrap().cancel_order(order_id)
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.inner.lock().unwrap().best_bid()
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.inner.lock().unwrap().best_ask()
    }

    pub fn spread(&self) -> Option<f64> {
        self.inner.lock().unwrap().spread()
    }

    pub fn mid_price(&self) -> Option<f64> {
        self.inner.lock().unwrap().mid_price()
    }

    pub fn get_depth(&self, levels: usize) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        self.inner.lock().unwrap().get_depth(levels)
    }

    pub fn order_count(&self) -> usize {
        self.inner.lock().unwrap().order_count()
    }
}

impl Clone for SharedOrderBook {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::order::OrderSide;

    #[test]
    fn test_add_and_match_orders() {
        let mut book = OrderBook::new("BTCUSDT".to_string());

        // Add a sell order
        let sell_order = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50000.0, 1.0);
        book.add_order(sell_order);

        assert_eq!(book.best_ask(), Some(50000.0));
        assert_eq!(book.order_count(), 1);

        // Add a matching buy order
        let buy_order = Order::new_limit("BTCUSDT".to_string(), OrderSide::Buy, 50000.0, 0.5);
        let trades = book.add_order(buy_order);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 0.5);
        assert_eq!(book.order_count(), 1); // Sell order still has 0.5 remaining
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new("BTCUSDT".to_string());

        let order = Order::new_limit("BTCUSDT".to_string(), OrderSide::Buy, 50000.0, 1.0);
        let order_id = order.id;
        book.add_order(order);

        assert_eq!(book.order_count(), 1);

        let cancelled = book.cancel_order(order_id);
        assert!(cancelled.is_some());
        assert_eq!(book.order_count(), 0);
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new("BTCUSDT".to_string());

        // Add two sell orders at same price
        let sell1 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50000.0, 0.5);
        let sell1_id = sell1.id;
        book.add_order(sell1);

        let sell2 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50000.0, 0.5);
        let sell2_id = sell2.id;
        book.add_order(sell2);

        // Add buy order that matches
        let buy = Order::new_limit("BTCUSDT".to_string(), OrderSide::Buy, 50000.0, 0.5);
        let trades = book.add_order(buy);

        // Should match with first sell order (time priority)
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].maker_order_id, sell1_id);
    }
}
