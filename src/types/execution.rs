use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{OrderId, OrderSide, Symbol};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradeId(pub Uuid);

impl TradeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TradeId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub execution_id: ExecutionId,
    pub order_id: OrderId,
    pub trade_id: Option<TradeId>,
    pub client_id: String,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub execution_type: ExecutionType,
    pub order_status: super::OrderStatus,
    pub price: Option<Decimal>,
    pub quantity: Decimal,
    pub cumulative_quantity: Decimal,
    pub leaves_quantity: Decimal,
    pub last_price: Option<Decimal>,
    pub last_quantity: Option<Decimal>,
    pub average_price: Option<Decimal>,
    pub commission: Decimal,
    pub commission_asset: String,
    pub timestamp: DateTime<Utc>,
    pub latency_micros: u64,
}

// Simplified execution report for trading service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleExecutionReport {
    pub order_id: OrderId,
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
    pub status: super::OrderStatus,
    pub latency_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionType {
    New,
    PartialFill,
    Fill,
    DoneForDay,
    Canceled,
    Replaced,
    PendingCancel,
    Stopped,
    Rejected,
    Suspended,
    PendingNew,
    Calculated,
    Expired,
    Restated,
    PendingReplace,
    Trade,
    TradeCorrect,
    TradeCancel,
    OrderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub fill_id: Uuid,
    pub trade_id: TradeId,
    pub execution_id: ExecutionId,
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub price: Decimal,
    pub quantity: Decimal,
    pub commission: Decimal,
    pub commission_asset: String,
    pub timestamp: DateTime<Utc>,
    pub is_maker: bool,
    pub match_latency_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub trade_id: TradeId,
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub symbol: Symbol,
    pub price: Decimal,
    pub quantity: Decimal,
    pub maker_fill: Fill,
    pub taker_fill: Fill,
    pub timestamp: DateTime<Utc>,
    pub matching_latency_nanos: u64,
}

impl MatchResult {
    pub fn new(
        maker_order_id: OrderId,
        taker_order_id: OrderId,
        symbol: Symbol,
        price: Decimal,
        quantity: Decimal,
        maker_side: OrderSide,
        matching_start: DateTime<Utc>,
    ) -> Self {
        let trade_id = TradeId::new();
        let timestamp = Utc::now();
        let matching_latency_nanos =
            (timestamp.timestamp_nanos() - matching_start.timestamp_nanos()) as u64;

        let maker_fill = Fill {
            fill_id: Uuid::new_v4(),
            trade_id,
            execution_id: ExecutionId::new(),
            order_id: maker_order_id,
            symbol: symbol.clone(),
            side: maker_side,
            price,
            quantity,
            commission: Decimal::ZERO, // Will be calculated elsewhere
            commission_asset: "USDT".to_string(),
            timestamp,
            is_maker: true,
            match_latency_nanos: matching_latency_nanos,
        };

        let taker_fill = Fill {
            fill_id: Uuid::new_v4(),
            trade_id,
            execution_id: ExecutionId::new(),
            order_id: taker_order_id,
            symbol: symbol.clone(),
            side: match maker_side {
                OrderSide::Buy => OrderSide::Sell,
                OrderSide::Sell => OrderSide::Buy,
            },
            price,
            quantity,
            commission: Decimal::ZERO, // Will be calculated elsewhere
            commission_asset: "USDT".to_string(),
            timestamp,
            is_maker: false,
            match_latency_nanos: matching_latency_nanos,
        };

        Self {
            trade_id,
            maker_order_id,
            taker_order_id,
            symbol,
            price,
            quantity,
            maker_fill,
            taker_fill,
            timestamp,
            matching_latency_nanos,
        }
    }
}