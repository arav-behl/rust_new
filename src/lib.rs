// High-Performance Cryptocurrency Order Book Engine
// Demonstrates: Async Rust, WebSocket Integration, Order Matching, Market Microstructure

pub mod exchange;
pub mod orderbook;
pub mod types;

pub use exchange::{BinanceFeed, MarketData};
pub use orderbook::{OrderBook, SharedOrderBook};
pub use types::{Order, OrderId, OrderSide, OrderStatus, OrderType, Trade};
