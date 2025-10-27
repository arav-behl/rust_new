# High-Performance Cryptocurrency Order Book Engine

**A Rust implementation of a professional-grade order book with real-time Binance WebSocket integration**

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## 🎯 What This Project Demonstrates

This project showcases advanced Rust programming skills relevant to quantitative trading and high-frequency trading systems:

- **High-Performance Order Book**: Price-time priority matching with O(log n) operations
- **Real-Time Market Data**: Live WebSocket feeds from Binance
- **Order Matching Engine**: Professional-grade limit order matching algorithm
- **Async Rust**: Tokio-based concurrent architecture
- **Market Microstructure**: Understanding of bid/ask spreads, order book depth, and trade execution

## 🚀 Quick Start

```bash
# Clone the repository
git clone <repository>
cd rust_crypto_project

# Run the demo
cargo run --release

# Run tests
cargo test
```

## ✨ Core Features

### 1. **Order Book Implementation**
Inspired by the [Tzadiko C++ Orderbook](https://github.com/Tzadiko/Orderbook), implemented in Rust with:

- **BTreeMap-based price levels** for efficient sorted access
- **Price-time priority** matching algorithm (FIFO at each price level)
- **VecDeque for order queues** at each price level
- **Fast order lookup** with HashMap by OrderId
- **Thread-safe** with Arc<Mutex> wrapper

```rust
// Create an order book
let orderbook = SharedOrderBook::new("BTCUSDT".to_string());

// Add orders
let order = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50000.0, 1.0);
let trades = orderbook.add_order(order);

// Query the book
let best_bid = orderbook.best_bid();
let spread = orderbook.spread();
let (bids, asks) = orderbook.get_depth(10);
```

### 2. **Real-Time Market Data**
Live WebSocket integration with Binance:

- **Ticker feed**: Real-time price updates
- **Depth feed**: Order book depth updates every 100ms
- **Automatic reconnection** on connection loss
- **Multiple symbols** supported (BTC, ETH, SOL, etc.)

### 3. **Order Matching Engine**
Professional matching logic:

- **Price-time priority**: Orders at same price matched in time order (FIFO)
- **Partial fills**: Support for orders that partially match
- **Trade generation**: Automatic trade records for matched orders
- **Best execution**: Matches against best available prices

## 🏗️ Architecture

```
┌─────────────────────────────────────────┐
│         Binance WebSocket               │
│    (Ticker + Depth Feeds)               │
└──────────────┬──────────────────────────┘
               │
               v
┌─────────────────────────────────────────┐
│         BinanceFeed                     │
│  - Market data aggregation              │
│  - Price/spread tracking                │
└──────────────┬──────────────────────────┘
               │
               v
┌─────────────────────────────────────────┐
│         Order Book Engine               │
│  - BTreeMap price levels                │
│  - Order matching                       │
│  - Trade generation                     │
└─────────────────────────────────────────┘
```

## 📊 Order Book Design

Based on the professional C++ implementation by Tzadiko, adapted to Rust:

### Data Structures

```rust
pub struct OrderBook {
    symbol: String,
    bids: BTreeMap<OrderedFloat, PriceLevel>,  // Highest price first
    asks: BTreeMap<OrderedFloat, PriceLevel>,  // Lowest price first
    orders: HashMap<OrderId, OrderSide>,       // Fast lookup
}

pub struct PriceLevel {
    price: f64,
    orders: VecDeque<Order>,  // FIFO queue
    total_quantity: f64,
}
```

### Key Operations

| Operation | Complexity | Description |
|-----------|-----------|-------------|
| Add Order | O(log n) | Insert into BTreeMap + match |
| Cancel Order | O(log n) | Remove from price level |
| Best Bid/Ask | O(1) | BTreeMap boundary access |
| Get Depth | O(k) | Iterate k levels |
| Match Order | O(m) | m = orders matched |

## 🎓 Rust Concepts Demonstrated

### 1. **Advanced Data Structures**
```rust
// BTreeMap for sorted price levels
bids: BTreeMap<OrderedFloat, PriceLevel>

// VecDeque for FIFO order queues
orders: VecDeque<Order>

// HashMap for fast lookup
orders: HashMap<OrderId, OrderSide>
```

### 2. **Async/Await & Concurrency**
```rust
// Spawn concurrent WebSocket tasks
tokio::spawn(async move {
    loop {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                // Process messages
            }
        }
    }
});
```

### 3. **Smart Pointers & Thread Safety**
```rust
pub struct SharedOrderBook {
    inner: Arc<Mutex<OrderBook>>,  // Thread-safe shared ownership
}
```

### 4. **Type Safety & Custom Ordering**
```rust
// Wrapper to make f64 orderable for BTreeMap
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct OrderedFloat(f64);
```

## 💼 Resume Talking Points

**For Interviews at Trading Firms:**

1. **"Implemented a high-performance order book in Rust with price-time priority matching"**
   - BTreeMap for O(log n) price level access
   - VecDeque for FIFO matching at each price
   - Demonstrates understanding of market microstructure

2. **"Integrated real-time Binance WebSocket feeds for live market data"**
   - Async Rust with Tokio
   - Concurrent task management
   - Production-quality error handling and reconnection logic

3. **"Built order matching engine with trade generation and partial fills"**
   - Professional matching algorithm
   - Proper fill/partial fill handling
   - Trade record generation

4. **"Designed thread-safe concurrent architecture using Arc/Mutex patterns"**
   - Proper Rust ownership and borrowing
   - Safe shared mutable state
   - Lock-based synchronization

## 🛠️ Project Structure

```
src/
├── types/
│   └── order.rs          # Order, Trade, OrderType definitions
├── orderbook/
│   └── book.rs           # OrderBook implementation
├── exchange/
│   └── binance.rs        # WebSocket integration
├── lib.rs                # Library interface
└── main.rs               # Demo application

Key files:
- orderbook/book.rs (~460 lines) - Core order book with matching
- exchange/binance.rs (~200 lines) - WebSocket data ingestion
- types/order.rs (~150 lines) - Type definitions
```

## 📈 Comparison with C++ Implementation

| Feature | C++ (Tzadiko) | This Project (Rust) |
|---------|---------------|---------------------|
| Price Levels | std::map | BTreeMap |
| Order Queue | Custom queue | VecDeque |
| Thread Safety | Mutex | Arc<Mutex> |
| Order Lookup | OrderId map | HashMap<OrderId> |
| Memory Safety | Manual | Automatic (Rust) |
| Type Safety | Templates | Generics + Traits |

## 🔬 Technical Highlights

### Price-Time Priority
Orders are matched using **price-time priority**:
1. **Price priority**: Best prices match first (highest bid, lowest ask)
2. **Time priority**: Among orders at same price, earlier orders match first (FIFO)

### Memory Efficiency
- **Sparse data structures**: Only stores active price levels
- **Automatic cleanup**: Empty price levels removed
- **Smart pointers**: Arc for shared ownership, no manual memory management

### Async Architecture
- **Non-blocking**: WebSocket feeds run concurrently
- **Tokio runtime**: Professional-grade async runtime
- **Proper error handling**: Reconnection logic for network failures

## 🎯 What Makes This Project Strong

### ✅ Honest and Professional
- Clean, well-documented code
- Based on established C++ reference implementation
- Realistic scope - not over-promised

### ✅ Demonstrates Real Skills
- Professional data structure choices
- Production-quality async patterns
- Understanding of order book mechanics

### ✅ Relevant to Trading Firms
- Direct experience with order books
- Market data integration
- Performance-conscious design

### ✅ Interview-Ready
- Can explain every design decision
- Prepared to discuss trade-offs
- Clear technical depth

## 📚 Learning Resources

- **C++ Reference**: [Tzadiko/Orderbook](https://github.com/Tzadiko/Orderbook)
- **Video Series**: [@TheCodingJesus](https://www.youtube.com/@TheCodingJesus) on YouTube
- **Market Microstructure**: Understanding order books and matching

## 🤔 Design Decisions

**Why BTreeMap instead of HashMap?**
- Need sorted access for best bid/ask
- Efficient iteration through price levels
- O(log n) is acceptable for order book operations

**Why VecDeque for order queues?**
- FIFO semantics for time priority
- Efficient push_back/pop_front
- Better cache locality than linked list

**Why Arc<Mutex> instead of lock-free?**
- Clear, maintainable code
- Sufficient performance for demonstration
- Production systems could use lock-free structures

**Why f64 instead of Decimal?**
- Simplicity and performance
- Acceptable for demonstration
- Production systems should use fixed-point arithmetic

## 📞 Interview Preparation

### Be Ready to Explain:

1. **Order matching algorithm**: Price-time priority, FIFO at each level
2. **BTreeMap choice**: O(log n) sorted access, efficient best bid/ask
3. **Async design**: Tokio for concurrent WebSocket handling
4. **Thread safety**: Arc<Mutex> for shared state across tasks
5. **Trade-offs**: Simplicity vs. maximum performance

### Code Walkthrough:
- Explain the OrderBook::match_order() logic
- Walk through add_order() and trade generation
- Discuss WebSocket message handling
- Explain SharedOrderBook thread-safety

---

**Built with Rust 🦀 | Demonstrates production-ready order book implementation and market data integration**

## 📝 License

MIT License - see LICENSE file for details

## 🙋 Questions?

This project demonstrates core skills for quantitative trading and market making roles. The clean architecture and professional implementation show understanding of both Rust systems programming and financial market microstructure.
