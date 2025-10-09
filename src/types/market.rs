use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketTick {
    pub symbol: Symbol,
    pub exchange: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: TickSide,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TickSide {
    Bid,
    Ask,
    Trade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: Decimal,
    pub quantity: Decimal,
    pub order_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub symbol: Symbol,
    pub exchange: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub timestamp: DateTime<Utc>,
    pub sequence: u64,
    pub last_update_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub symbol: Symbol,
    pub exchange: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub timestamp: DateTime<Utc>,
    pub is_buyer_maker: bool,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub symbol: Symbol,
    pub open_price: Decimal,
    pub high_price: Decimal,
    pub low_price: Decimal,
    pub close_price: Decimal,
    pub volume: Decimal,
    pub quote_volume: Decimal,
    pub timestamp: DateTime<Utc>,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketEvent {
    Tick(MarketTick),
    BookSnapshot(OrderBookSnapshot),
    Trade(Trade),
    Stats(MarketStats),
}

impl MarketEvent {
    pub fn symbol(&self) -> &Symbol {
        match self {
            MarketEvent::Tick(tick) => &tick.symbol,
            MarketEvent::BookSnapshot(snapshot) => &snapshot.symbol,
            MarketEvent::Trade(trade) => &trade.symbol,
            MarketEvent::Stats(stats) => &stats.symbol,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            MarketEvent::Tick(tick) => tick.timestamp,
            MarketEvent::BookSnapshot(snapshot) => snapshot.timestamp,
            MarketEvent::Trade(trade) => trade.timestamp,
            MarketEvent::Stats(stats) => stats.timestamp,
        }
    }

    pub fn sequence(&self) -> Option<u64> {
        match self {
            MarketEvent::Tick(tick) => Some(tick.sequence),
            MarketEvent::BookSnapshot(snapshot) => Some(snapshot.sequence),
            MarketEvent::Trade(trade) => Some(trade.sequence),
            MarketEvent::Stats(_) => None,
        }
    }
}