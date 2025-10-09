use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub client_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub filled_quantity: Decimal,
    pub status: OrderStatus,
    pub timestamp: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
}

impl Order {
    pub fn new(
        client_id: String,
        symbol: String,
        side: OrderSide,
        order_type: OrderType,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> Self {
        Self {
            id: OrderId::new(),
            client_id,
            symbol,
            side,
            order_type,
            quantity,
            price,
            filled_quantity: Decimal::ZERO,
            status: OrderStatus::Pending,
            timestamp: Utc::now(),
            last_update: Utc::now(),
        }
    }

    pub fn remaining_quantity(&self) -> Decimal {
        self.quantity - self.filled_quantity
    }

    pub fn is_fully_filled(&self) -> bool {
        self.filled_quantity >= self.quantity
    }

    pub fn can_match(&self, market_price: Decimal) -> bool {
        match (&self.order_type, &self.side, self.price) {
            (OrderType::Market, _, _) => true,
            (OrderType::Limit, OrderSide::Buy, Some(limit_price)) => limit_price >= market_price,
            (OrderType::Limit, OrderSide::Sell, Some(limit_price)) => limit_price <= market_price,
            _ => false,
        }
    }
}