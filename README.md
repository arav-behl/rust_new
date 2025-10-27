# High-Performance Cryptocurrency Order Book Engine

**A Rust implementation of a professional-grade limit order book with real-time market data integration**

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Overview
https://github.com/user-attachments/assets/eac269d2-abdf-45b5-96b2-efa18636f078

This project implements a high-performance order matching engine in Rust, demonstrating advanced systems programming concepts and financial market microstructure knowledge. The implementation is based on professional C++ orderbook designs, adapted to leverage Rust's memory safety guarantees and modern async capabilities.

## Technical Highlights

### Core Architecture

- **High-Performance Order Book**: BTreeMap-based price levels with O(log n) operations
- **Price-Time Priority Matching**: Industry-standard FIFO matching algorithm
- **Real-Time Data Integration**: WebSocket feeds from Binance exchange
- **Concurrent Architecture**: Tokio-based async runtime with proper synchronization
- **Memory Safety**: Zero-cost abstractions with compile-time safety guarantees

### Key Features

1. **Order Book Implementation**
   - Sorted price levels using BTreeMap for efficient access
   - VecDeque-based FIFO queues at each price level
   - HashMap for O(1) order lookup by ID
   - Automatic cleanup of empty price levels

2. **Order Matching Engine**
   - Price-time priority algorithm
   - Support for partial order fills
   - Automatic trade generation
   - Maintains market integrity invariants

3. **Market Data Integration**
   - Real-time ticker feed (price updates)
   - Order book depth snapshots (100ms intervals)
   - Automatic reconnection on connection loss
   - Thread-safe market data aggregation

4. **Type Safety**
   - Strong typing for financial primitives
   - Compile-time prevention of common errors
   - Zero-cost abstractions

## Quick Start

```bash
# Clone the repository
git clone <repository-url>
cd rust_crypto_project

# Run the demo
cargo run --release

# Run tests
cargo test
```

## Project Structure

```
src/
├── types/
│   └── order.rs          # Order, Trade, and type definitions
├── orderbook/
│   └── book.rs           # Core order book with matching engine
├── exchange/
│   └── binance.rs        # WebSocket integration
├── lib.rs                # Public API
└── main.rs               # Demo application

Documentation:
├── README.md             # Project overview
├── ARCHITECTURE.md       # Technical deep-dive
├── PROJECT_SUMMARY.md    # Interview preparation guide
└── QUICKSTART.md         # Running instructions
```

**Code Metrics:**
- ~1,000 lines of production-quality Rust
- Comprehensive inline documentation
- Unit tests for core matching logic
- Zero compiler warnings

## Order Book Design

### Data Structures

The implementation uses carefully chosen data structures for optimal performance:

```rust
pub struct OrderBook {
    symbol: String,
    bids: BTreeMap<OrderedFloat, PriceLevel>,  // Sorted highest to lowest
    asks: BTreeMap<OrderedFloat, PriceLevel>,  // Sorted lowest to highest
    orders: HashMap<OrderId, OrderSide>,       // Fast lookup by ID
}

pub struct PriceLevel {
    price: f64,
    orders: VecDeque<Order>,   // FIFO queue for time priority
    total_quantity: f64,
}
```

### Algorithm Complexity

| Operation | Time Complexity | Description |
|-----------|----------------|-------------|
| Add Order | O(log n + m) | Insert into tree + match m orders |
| Cancel Order | O(log n + k) | Find level + scan k orders at price |
| Best Bid/Ask | O(1) | Direct tree boundary access |
| Get Depth | O(k) | Iterate k levels |
| Match Order | O(m) | Process m matching orders |

Where:
- n = number of active price levels
- m = number of orders matched
- k = depth levels requested

## Technical Implementation

### Price-Time Priority Matching

The matching engine implements the industry-standard price-time priority algorithm:

1. **Price Priority**: Orders execute at the best available price
   - Buy orders match against lowest ask prices
   - Sell orders match against highest bid prices

2. **Time Priority**: At each price level, orders match in FIFO order
   - Earlier orders at the same price have priority
   - Implemented using VecDeque for efficient queue operations

### Concurrency Model

Thread-safe concurrent architecture using Rust's ownership system:

```rust
// Market data: Multiple readers, single writer
Arc<RwLock<Vec<MarketData>>>

// Order book: Exclusive access for order operations
Arc<Mutex<OrderBook>>
```

**Design Decisions:**
- `Arc` for shared ownership across async tasks
- `Mutex` for order book (write-heavy workload)
- `RwLock` for market data (read-heavy workload)

### WebSocket Integration

Asynchronous WebSocket connections to Binance:

- **Ticker Stream**: Real-time price updates for multiple symbols
- **Depth Stream**: Order book snapshots at 100ms intervals
- **Resilience**: Automatic reconnection with exponential backoff
- **Non-blocking**: Concurrent tasks don't block order processing

## Code Quality

### Rust Best Practices

- **Zero unsafe code**: All operations use safe Rust abstractions
- **No unwrap() in production paths**: Proper error handling throughout
- **Comprehensive documentation**: All public APIs documented
- **Type-driven design**: Leverages Rust's type system for correctness

### Testing

```bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_price_time_priority
```

**Test Coverage:**
- Order matching logic
- Price-time priority verification
- Cancellation behavior
- Edge cases (empty book, partial fills)

## Performance Characteristics

### Computational Efficiency

- **Memory**: Sparse data structure (only active price levels stored)
- **CPU**: Efficient algorithms with minimal allocations
- **Latency**: Sub-millisecond order matching
- **Cache**: Good locality with VecDeque and BTreeMap

### Scalability Considerations

Current implementation optimized for clarity and correctness. Production enhancements could include:

1. **Lock-free data structures** for higher throughput
2. **Thread-per-core architecture** for better CPU utilization
3. **SPSC channels** for zero-allocation message passing
4. **Custom allocators** for predictable latency

## Design Inspiration

This implementation is based on the [Tzadiko C++ Orderbook](https://github.com/Tzadiko/Orderbook), documented on [@TheCodingJesus](https://www.youtube.com/@TheCodingJesus) YouTube channel.

### Rust Advantages Over C++

| Aspect | C++ | This Project (Rust) |
|--------|-----|---------------------|
| Memory Safety | Manual management | Automatic, compile-time verified |
| Concurrency | Mutex + careful coding | Ownership prevents data races |
| Error Handling | Exceptions or error codes | Result types with ? operator |
| Price Levels | std::map | BTreeMap (similar performance) |
| Thread Safety | Requires discipline | Enforced by compiler |

## Demonstrated Skills

### Systems Programming

- Advanced data structure implementation
- Algorithm design and analysis
- Memory-efficient design patterns
- Performance optimization techniques

### Concurrent Programming

- Async/await and futures
- Thread-safe shared state
- Lock-based synchronization
- Task spawning and management

### Domain Knowledge

- Order book mechanics
- Market microstructure
- Trade execution logic
- Financial market concepts

### Software Engineering

- Clean architecture
- Comprehensive documentation
- Test-driven development
- Professional code organization

## Building and Deployment

### Local Development

```bash
# Development build (fast compilation)
cargo build

# Optimized release build
cargo build --release

# Run with logging
RUST_LOG=debug cargo run --release
```

### Docker

```bash
# Build image
docker build -t crypto-orderbook .

# Run container
docker run crypto-orderbook
```

### Production Considerations

For production deployment, additional considerations include:

- **Fixed-point arithmetic** instead of floating-point for price/quantity
- **Persistence layer** for order book snapshots and recovery
- **Metrics and monitoring** (Prometheus, Grafana)
- **Comprehensive logging** with structured log formats
- **Rate limiting** and request validation
- **Authentication** and authorization

## Design Trade-offs

### Implementation Decisions

**BTreeMap vs HashMap:**
- Chosen BTreeMap for sorted iteration and O(1) best bid/ask
- Acceptable O(log n) insertion cost for order book operations
- Better cache locality than hash-based structures

**Mutex vs RwLock for OrderBook:**
- Chosen Mutex for simpler API and better write performance
- Order operations are predominantly writes
- Production systems could use RwLock for concurrent reads

**f64 vs Decimal:**
- Used f64 for simplicity and performance
- Acceptable for demonstration purposes
- Production systems require fixed-point arithmetic

**Synchronous Matching vs Event-Driven:**
- Synchronous matching for simplicity and correctness
- Easier to reason about order book state
- Event-driven architecture possible for higher throughput

## Future Enhancements

Potential extensions for discussion or implementation:

### Order Types
- IOC (Immediate-or-Cancel)
- FOK (Fill-or-Kill)
- Stop orders and stop-limit orders
- Iceberg orders
- Time-in-force options

### Features
- REST API using Axum framework
- WebSocket server for client updates
- Order book snapshots and history
- Trade tape with historical data
- Position tracking and P&L

### Performance
- Lock-free data structures
- Thread-per-core architecture
- Custom memory allocators
- Zero-copy message passing

### Production Readiness
- Database persistence (PostgreSQL)
- Distributed deployment
- Load balancing
- Circuit breakers and fallbacks

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)**: Detailed technical architecture
- **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)**: Executive summary and talking points
- **[QUICKSTART.md](QUICKSTART.md)**: Getting started guide
- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)**: Common issues and solutions

## Use Cases

This project demonstrates capabilities relevant to:

- **High-Frequency Trading Systems**: Low-latency order matching
- **Market Making Platforms**: Order book management
- **Trading Infrastructure**: Real-time data processing
- **Financial Systems**: Concurrent transaction processing
- **Systems Programming**: Advanced Rust patterns

## Contributing

This is a portfolio project demonstrating professional Rust development practices. The codebase is designed to be:

- Easy to understand and modify
- Well-documented for learning
- Extensible for additional features
- Suitable for interview discussions

## License

MIT License - See LICENSE file for details

## Technical Questions

### Architecture Decisions

**Q: Why BTreeMap instead of a custom tree structure?**
A: BTreeMap provides excellent performance characteristics (O(log n) operations) with a well-tested implementation. Custom structures could offer marginal improvements but at the cost of complexity and potential bugs.

**Q: How does the matching engine ensure correctness?**
A: The engine maintains strict invariants: price-time priority is enforced through sorted data structures (BTreeMap) and FIFO queues (VecDeque). Trade generation happens atomically within the matching logic.

**Q: What are the scaling limits?**
A: Current architecture handles thousands of orders efficiently. Scaling to millions would require lock-free structures and distributed architecture, both feasible extensions.

**Q: How is thread safety guaranteed?**
A: Rust's ownership system prevents data races at compile time. Arc<Mutex<_>> ensures exclusive access to order book, while Arc<RwLock<_>> allows concurrent reads of market data.

## Acknowledgments

- Inspired by [Tzadiko's C++ Orderbook](https://github.com/Tzadiko/Orderbook)
- Market data provided by Binance API
- Built with the Rust programming language and Tokio async runtime

---

**This project demonstrates production-ready Rust systems programming with a focus on correctness, performance, and clean architecture.**
