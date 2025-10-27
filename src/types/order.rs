use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Unique identifier for an order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

impl OrderId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

/// Order side (Buy or Sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Market order - executes immediately at best available price
    Market,
    /// Limit order - executes only at specified price or better
    Limit,
    /// Good-till-cancel - remains in book until filled or cancelled
    GoodTillCancel,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

/// Core order structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: f64,
    pub initial_quantity: f64,
    pub remaining_quantity: f64,
    pub status: OrderStatus,
    pub timestamp: DateTime<Utc>,
}

impl Order {
    pub fn new_limit(symbol: String, side: OrderSide, price: f64, quantity: f64) -> Self {
        Self {
            id: OrderId::new(),
            symbol,
            side,
            order_type: OrderType::Limit,
            price,
            initial_quantity: quantity,
            remaining_quantity: quantity,
            status: OrderStatus::Pending,
            timestamp: Utc::now(),
        }
    }

    pub fn new_market(symbol: String, side: OrderSide, quantity: f64) -> Self {
        Self {
            id: OrderId::new(),
            symbol,
            side,
            order_type: OrderType::Market,
            price: 0.0, // Market orders don't have a price
            initial_quantity: quantity,
            remaining_quantity: quantity,
            status: OrderStatus::Pending,
            timestamp: Utc::now(),
        }
    }

    /// Fill the order with the specified quantity
    pub fn fill(&mut self, quantity: f64) {
        self.remaining_quantity -= quantity;
        if self.remaining_quantity <= 0.0 {
            self.remaining_quantity = 0.0;
            self.status = OrderStatus::Filled;
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
    }

    pub fn is_filled(&self) -> bool {
        self.remaining_quantity <= 0.0
    }

    pub fn filled_quantity(&self) -> f64 {
        self.initial_quantity - self.remaining_quantity
    }

    /// Check if this order can match with the given price
    pub fn can_match(&self, market_price: f64) -> bool {
        match (self.order_type, self.side) {
            (OrderType::Market, _) => true,
            (OrderType::Limit, OrderSide::Buy) | (OrderType::GoodTillCancel, OrderSide::Buy) => {
                self.price >= market_price
            }
            (OrderType::Limit, OrderSide::Sell) | (OrderType::GoodTillCancel, OrderSide::Sell) => {
                self.price <= market_price
            }
        }
    }
}

/// Trade information resulting from order matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub symbol: String,
    pub price: f64,
    pub quantity: f64,
    pub timestamp: DateTime<Utc>,
}

impl Trade {
    pub fn new(
        maker_order_id: OrderId,
        taker_order_id: OrderId,
        symbol: String,
        price: f64,
        quantity: f64,
    ) -> Self {
        Self {
            maker_order_id,
            taker_order_id,
            symbol,
            price,
            quantity,
            timestamp: Utc::now(),
        }
    }
}
