# Architecture Overview

## System Design

This project implements a high-performance order book matching engine with live market data integration. The architecture is designed to demonstrate professional software engineering practices and understanding of market microstructure.

## Core Components

### 1. Order Book Engine (`src/orderbook/book.rs`)

The heart of the system - a limit order book with price-time priority matching.

**Data Structures:**
- `BTreeMap<OrderedFloat, PriceLevel>` for bids (descending order)
- `BTreeMap<OrderedFloat, PriceLevel>` for asks (ascending order)
- `HashMap<OrderId, OrderSide>` for O(1) order lookup
- `VecDeque<Order>` for FIFO queues at each price level

**Key Operations:**
```
add_order()     → Match against book, add remaining to appropriate side
cancel_order()  → Remove from price level, cleanup empty levels
match_order()   → Price-time priority matching algorithm
get_depth()     → Return top N price levels for market data
```

**Design Inspiration:**
Based on [Tzadiko's C++ Orderbook](https://github.com/Tzadiko/Orderbook), adapted to Rust with:
- Memory safety guarantees
- Safe concurrent access patterns
- Rust idioms and best practices

### 2. Market Data Integration (`src/exchange/binance.rs`)

Real-time WebSocket feeds from Binance exchange.

**Features:**
- Concurrent ticker and depth feeds
- Automatic reconnection on connection loss
- Market data aggregation (price, bid/ask, spread)
- Non-blocking async architecture

**WebSocket Streams:**
```
Ticker Feed:  wss://stream.binance.com:9443/stream?streams=btcusdt@ticker/ethusdt@ticker/...
Depth Feed:   wss://stream.binance.com:9443/stream?streams=btcusdt@depth10@100ms/...
```

### 3. Type System (`src/types/order.rs`)

Strong typing for financial primitives:

```rust
Order          → Represents a limit or market order
OrderId        → Unique identifier (atomic counter)
OrderSide      → Buy or Sell
OrderType      → Market, Limit, GoodTillCancel
OrderStatus    → Pending, PartiallyFilled, Filled, Cancelled
Trade          → Immutable trade record
```

## Data Flow

```
┌──────────────────┐
│  Binance API     │
│  (WebSocket)     │
└────────┬─────────┘
         │
         v
┌────────────────────────────────┐
│  BinanceFeed                   │
│  - Parse ticker messages       │
│  - Parse depth updates         │
│  - Aggregate market data       │
└────────┬───────────────────────┘
         │
         v
┌────────────────────────────────┐
│  Market Data Store             │
│  (Arc<RwLock<Vec<MarketData>>)│
└────────────────────────────────┘

         User Orders
         │
         v
┌────────────────────────────────┐
│  SharedOrderBook               │
│  (Arc<Mutex<OrderBook>>)       │
├────────────────────────────────┤
│  1. Match against resting      │
│     orders (price-time)        │
│  2. Generate trades            │
│  3. Add remaining to book      │
└────────┬───────────────────────┘
         │
         v
    Trade Reports
```

## Concurrency Model

### Async Tasks

1. **Ticker Feed Task**: Continuously reads price updates
2. **Depth Feed Task**: Continuously reads order book depth
3. **Main Application**: Demonstrates order book operations

### Synchronization

- **BinanceFeed market data**: `Arc<RwLock<Vec<MarketData>>>`
  - Multiple readers for market data queries
  - Single writer from WebSocket feed

- **SharedOrderBook**: `Arc<Mutex<OrderBook>>`
  - Exclusive access for order operations
  - Simpler than RwLock for this use case (frequent writes)

### Task Spawning

```rust
tokio::spawn(async move {
    // WebSocket connection loop with automatic reconnection
    loop {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => { /* process messages */ }
            Err(_) => { /* wait and retry */ }
        }
    }
});
```

## Order Matching Algorithm

### Price-Time Priority

The matching engine implements **price-time priority** (also called FIFO):

1. **Price Priority**:
   - Buy orders match with lowest available sell price
   - Sell orders match with highest available buy price

2. **Time Priority**:
   - Among orders at the same price, earlier orders match first
   - Implemented via `VecDeque` (FIFO queue) at each price level

### Matching Example

```
Order Book State:
  Asks:  50200 → [Order3(0.5), Order5(0.3)]
         50150 → [Order1(0.75)]
         50100 → [Order2(0.5)]

  Bids:  49900 → [Order4(0.3)]

New Order: BUY 1.0 @ 50200 (limit)

Matching Process:
1. Check best ask: 50100
2. Match 0.5 BTC @ 50100 (Order2 filled, removed)
3. Check next ask: 50150
4. Match 0.5 BTC @ 50150 (Order1 partial, 0.25 remains)
5. Buy order filled (1.0 total), stop matching

Result:
  Trades: [(0.5, 50100), (0.5, 50150)]
  Order Book:
    Asks: 50200 → [Order3(0.5), Order5(0.3)]
          50150 → [Order1(0.25)]
```

## Performance Considerations

### Time Complexity

| Operation | Complexity | Reason |
|-----------|-----------|---------|
| Add Order | O(log n + m) | BTreeMap insert + matching m orders |
| Cancel Order | O(log n + k) | Find price level + scan queue |
| Best Bid/Ask | O(1) | BTreeMap first/last |
| Match Against Level | O(k) | Linear scan of orders at price |

Where:
- n = number of price levels
- m = number of orders matched
- k = orders at a specific price level

### Memory Layout

- **Sparse representation**: Only active price levels stored
- **Automatic cleanup**: Empty levels removed
- **Efficient queues**: VecDeque provides good cache locality

### Trade-offs

**BTreeMap vs HashMap:**
- ✅ Sorted iteration for depth queries
- ✅ O(1) best bid/ask access
- ❌ O(log n) insertion (acceptable for order books)

**Mutex vs RwLock:**
- ✅ Simpler API, less error-prone
- ✅ Adequate for demo (order operations are writes)
- ❌ No concurrent reads (production could use RwLock)

**f64 vs Decimal:**
- ✅ Simple, fast
- ✅ Sufficient precision for demo
- ❌ Floating-point issues (production uses fixed-point)

## Extensibility

### Potential Enhancements

1. **Order Types**:
   - IOC (Immediate-or-Cancel)
   - FOK (Fill-or-Kill)
   - Stop/Stop-Limit orders

2. **Performance**:
   - Lock-free data structures
   - SPSC channels for order submission
   - Thread-per-core architecture

3. **Features**:
   - Order book snapshots
   - Trade history
   - REST API (using Axum)
   - WebSocket feed for clients

4. **Production Readiness**:
   - Fixed-point arithmetic (avoiding f64)
   - Persistence layer
   - Metrics and monitoring
   - Comprehensive error handling

## Testing Strategy

### Unit Tests

Included in `src/orderbook/book.rs`:
- Order addition and matching
- Cancellation logic
- Price-time priority verification
- Edge cases (empty book, partial fills)

### Integration Testing

The `main.rs` serves as an integration test:
1. WebSocket connection
2. Market data ingestion
3. Order book operations
4. Trade generation

## Deployment Considerations

### Docker

Simple Dockerfile for containerization:
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/trading-engine /usr/local/bin/
CMD ["trading-engine"]
```

### Resource Usage

- **Memory**: Minimal (sparse data structures)
- **CPU**: Low (efficient algorithms)
- **Network**: WebSocket connections (2 concurrent)

## Summary

This architecture demonstrates:

✅ **Professional code organization**
✅ **Understanding of market microstructure**
✅ **Async Rust best practices**
✅ **Thoughtful data structure choices**
✅ **Production-ready patterns** (with noted simplifications)

The design balances **clarity** and **performance**, making it an excellent portfolio piece for quantitative trading roles.
