# Live Cryptocurrency Trading Engine

**A Rust-based real-time trading system demonstrating production-grade async programming, WebSocket integration, and REST API development**

![Rust](https://img.shields.io/badge/rust-1.75%2B-orange)
![Build Status](https://img.shields.io/badge/build-passing-brightgreen)

## 🎯 What This Project Demonstrates

This project showcases practical Rust skills relevant to quantitative trading and high-performance systems:

- **Async Rust**: Tokio runtime with concurrent WebSocket and HTTP tasks
- **Real-Time Data Processing**: Live market data from Binance WebSocket streams
- **REST API Development**: Axum web framework with JSON endpoints
- **Concurrent State Management**: Arc/RwLock patterns for shared mutable state
- **Error Handling**: Result types and graceful error recovery
- **Production Patterns**: Structured logging, health checks, and graceful shutdowns

## 🚀 Quick Start

```bash
# Clone and run
git clone <repository>
cd rust_crypto_project
cargo run --bin simple_trading_engine --release

# Open browser to http://localhost:8080
```

## ✨ Features

### Real-Time Market Data Integration
- **Live price feeds** from Binance WebSocket for BTC/USDT, ETH/USDT, SOL/USDT
- **Order book depth** with bid/ask spreads updated every 100ms
- **Automatic reconnection** on connection loss

### Trading Functionality
- **Order submission** with market and limit orders
- **Portfolio tracking** with real-time P&L calculation
- **Position management** with average price tracking
- **Realistic execution** using actual bid/ask prices from order book

### Technical Implementation
- **Concurrent processing**: Multiple WebSocket streams handled simultaneously
- **Shared state**: Thread-safe state management with Arc<RwLock>
- **REST API**: Clean JSON API for order submission and portfolio queries
- **Web interface**: Simple HTML/JS frontend for demonstration

## 📡 API Endpoints

### Market Data
```bash
GET /health                 # System health check
GET /api/v1/prices          # Current ticker prices
GET /api/v1/depth           # Order book depth with spreads
```

### Trading Operations
```bash
POST /api/v1/orders         # Submit new order
GET  /api/v1/orders         # Order history
GET  /api/v1/portfolio      # Portfolio with live P&L
```

### Example: Submit Order
```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Content-Type: application/json" \
  -d '{
    "symbol": "BTCUSDT",
    "side": "Buy",
    "quantity": 0.001,
    "price": null
  }'
```

## 🏗️ Architecture

```
┌─────────────────────┐
│  Binance WebSocket  │
│  (Price + Depth)    │
└──────────┬──────────┘
           │
           v
┌─────────────────────┐
│  Trading Engine     │
│  - Order Management │
│  - Portfolio State  │
│  - Market Data      │
└──────────┬──────────┘
           │
           v
┌─────────────────────┐
│  REST API (Axum)    │
│  - Order Submission │
│  - Portfolio Queries│
│  - Market Data      │
└─────────────────────┘
```

## 🛠️ Technical Stack

| Component | Technology | Purpose |
|-----------|-----------|---------|
| Runtime | Tokio | Async/await execution |
| HTTP Server | Axum | REST API framework |
| WebSocket | tokio-tungstenite | Real-time data streams |
| Serialization | Serde | JSON encoding/decoding |
| Concurrency | Arc/RwLock | Shared state management |
| Time | Chrono | Timestamp handling |

## 🎓 Rust Skills Demonstrated

### 1. **Async Programming**
```rust
// Concurrent WebSocket tasks
tokio::spawn(async move {
    loop {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                // Process messages
            }
            Err(e) => eprintln!("Error: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
});
```

### 2. **Ownership & Borrowing**
```rust
// Proper Arc/RwLock usage for shared state
let prices = Arc::clone(&self.current_prices);
let mut prices_lock = prices.write().await;
prices_lock.insert(symbol, price);
```

### 3. **Error Handling**
```rust
pub async fn submit_order(&self, request: OrderRequest)
    -> Result<Order, String> {
    // Proper Result-based error handling
}
```

### 4. **Type Safety**
```rust
// Strong typing with Serde for JSON APIs
#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: Option<f64>,
}
```

## 💼 Resume Talking Points

**For Interviews at Trading Firms:**

1. **"Built a real-time trading engine in Rust that processes live market data from Binance WebSocket feeds"**
   - Shows understanding of market data infrastructure
   - Demonstrates async Rust and concurrent programming

2. **"Implemented concurrent state management using Arc/RwLock patterns for thread-safe portfolio tracking"**
   - Understanding of Rust's ownership model
   - Production-ready concurrent programming

3. **"Created REST API with Axum handling order submission, portfolio queries, and real-time P&L calculations"**
   - Full-stack capability (backend API + data processing)
   - Understanding of trading system requirements

4. **"Integrated real-time order book depth to execute orders at realistic bid/ask prices"**
   - Understanding of market microstructure
   - Attention to realistic trading simulation

## 📚 Project Structure

```
src/
├── bin/
│   └── simple_trading_engine.rs  # Main binary (self-contained)
└── web/
    └── index.html                # Frontend interface

Key files:
- simple_trading_engine.rs (~440 lines)
  - TradingEngine struct with order/portfolio management
  - WebSocket integration for live data
  - REST API handlers
  - Async task spawning and management
```

## 🎯 What Makes This Project Good for Resumes

### ✅ Honest and Verifiable
- No exaggerated performance claims
- Everything actually works and can be demonstrated
- Realistic implementation scope

### ✅ Demonstrates Real Skills
- Production Rust patterns (not toy code)
- Real integration with external API (Binance)
- Proper async programming and state management

### ✅ Relevant to Trading Firms
- Shows understanding of market data
- Demonstrates systems programming capability
- Production-quality code structure

### ✅ Easy to Discuss in Interviews
- Clean, understandable codebase
- Can explain design decisions
- No black-box "magic" - everything is straightforward

## 🚀 Deployment

### Docker
```bash
docker build -t trading-engine .
docker run -p 8080:8080 trading-engine
```

### Railway / Render
- Push to GitHub
- Connect repository to Railway/Render
- Auto-detects Rust and builds
- Live demo URL for resume

## 🤔 Limitations (Be Honest!)

This is a **demonstration project**, not production trading software:

- **No order matching engine**: Orders are filled immediately at market price
- **Simplified portfolio**: Basic position tracking, no advanced risk management
- **Single-user**: No authentication or multi-user support
- **In-memory state**: Data lost on restart (no database persistence)
- **Basic error handling**: Production systems need more comprehensive error recovery

**Why this is okay:** This project demonstrates Rust fundamentals and system design. Real trading systems at firms like Wintermute would have dedicated teams, extensive testing, and compliance requirements. This shows you understand the basics and can learn the advanced parts.

## 📈 Future Enhancements (If Asked)

1. Add PostgreSQL for persistent storage
2. Implement proper order matching with limit order book
3. Add authentication and user management
4. Implement risk management and position limits
5. Add comprehensive unit and integration tests
6. Implement metrics and monitoring (Prometheus)
7. Add more sophisticated market making strategies

## 📞 Interview Preparation

### Be Ready to Explain:
1. **Why Rust?** Memory safety without garbage collection, great for low-latency systems
2. **Arc vs Rc?** Arc is atomic (thread-safe), Rc is not
3. **Why RwLock?** Multiple readers, single writer - good for frequently-read state
4. **Async vs threads?** Async is lighter weight for I/O-bound tasks like network
5. **How does error handling work?** Result types, ? operator, proper error propagation

### Code Walkthrough:
- Be able to explain the WebSocket connection code
- Explain how Arc/RwLock provides thread safety
- Walk through order execution logic
- Discuss trade-offs in design decisions

---

**Built with Rust 🦀 | Demonstrates production-ready async programming and real-time data processing**

