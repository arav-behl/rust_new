# Quick Start Guide

## How to Run the Order Book Demo

### Simple Method (Recommended)

```bash
cargo run --release
```

That's it! The application will:
1. Connect to Binance WebSocket feeds
2. Stream live prices for BTC, ETH, SOL
3. Demonstrate order book operations
4. Show trade matching in action

### What You'll See

#### 1. Startup (first few seconds)
```
🚀 High-Performance Crypto Order Book Engine
==============================================

✓ Connected to Binance WebSocket feeds
✓ Streaming live market data...
```

#### 2. Live Price Streaming
```
📊 BTCUSDT = $67,234.56
📊 ETHUSDT = $3,456.78
📊 SOLUSDT = $145.23
```

#### 3. Order Book Demo
```
📖 Demonstrating Order Book Operations:
========================================

Adding SELL orders to the book:
  Order #1: SELL 0.5 BTC @ $50,100
  Order #2: SELL 1.0 BTC @ $50,200
  Order #3: SELL 0.75 BTC @ $50,150

Adding BUY orders to the book:
  Order #4: BUY 0.3 BTC @ $49,900
  Order #5: BUY 0.8 BTC @ $49,800
```

#### 4. Order Book Depth
```
📊 Order Book Depth (Top 5 levels):
====================================

Asks (Sell Orders):
  $ 50,200.00  |  1.0000 BTC
  $ 50,150.00  |  0.7500 BTC
  $ 50,100.00  |  0.5000 BTC

  --- Spread: $200.00 ---

Bids (Buy Orders):
  $ 49,900.00  |  0.3000 BTC
  $ 49,800.00  |  0.8000 BTC
```

#### 5. Trade Execution
```
⚡ Executing Market Order:
=========================

Placing BUY order: 1.0 BTC @ market (limit: $50,200)

✓ Order matched! Generated 2 trade(s):
  • Trade: 0.5000 BTC @ $50,100.00 (Maker: #1, Taker: #6)
  • Trade: 0.5000 BTC @ $50,150.00 (Maker: #3, Taker: #6)
```

#### 6. Live Market Data
```
📡 Live Market Data from Binance:
==================================
  BTCUSDT | Price: $67,234.56 | Bid: $67,234.00 | Ask: $67,235.00 | Spread: $1.00
  ETHUSDT | Price: $3,456.78 | Bid: $3,456.50 | Ask: $3,457.00 | Spread: $0.50
  SOLUSDT | Price: $145.23 | Bid: $145.22 | Ask: $145.24 | Spread: $0.02
```

#### 7. Continuous Updates
```
📊 BTC: $67,234.56 | ETH: $3,456.78 | SOL: $145.23
```
(Updates every 10 seconds)

### Alternative: Development Mode (faster compilation)

```bash
# Faster to compile, slower to run
cargo run
```

### Running Tests

```bash
# Run unit tests
cargo test

# Run with output
cargo test -- --nocapture
```

### Building for Production

```bash
# Create optimized binary
cargo build --release

# Run the optimized binary directly
./target/release/trading-engine
```

## Understanding the Output

### WebSocket Connection Messages
- `✓ Connected to Binance ticker feed` - Price streaming connected
- `✓ Connected to Binance depth feed` - Order book depth connected

### Price Updates
- `📊 SYMBOL = $PRICE` - Latest ticker price from Binance

### Order Book Messages
- `📖 SYMBOL Bid: $X Ask: $Y Spread: $Z` - Order book depth update

### Order Operations
- `Order #N: BUY/SELL quantity @ price` - Order added to book
- `Trade: quantity @ price (Maker: #X, Taker: #Y)` - Orders matched

### Statistics
- `Active Orders` - Number of orders currently in the book
- `Best Bid/Ask` - Top of the order book
- `Mid Price` - Average of best bid and ask
- `Spread` - Difference between best bid and ask

## Troubleshooting

### Connection Issues
If you see connection errors:
- Check your internet connection
- Binance API may be temporarily unavailable
- The app will automatically retry every 5 seconds

### Compilation Errors
If build fails:
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --release
```

### Port Already in Use
This demo doesn't use ports - it's a console application only.

## What's Happening Behind the Scenes

1. **WebSocket Connections**: Two concurrent connections to Binance
   - Ticker stream for price updates
   - Depth stream for order book snapshots

2. **Order Book Operations**: Local order book simulation
   - Orders added with price-time priority
   - Matching engine finds crosses
   - Trades generated when orders match

3. **Concurrent Processing**: Tokio async runtime
   - WebSocket feeds run in background
   - Main thread demonstrates order book
   - All operations are non-blocking

## Next Steps

After running the demo:

1. **Examine the code**:
   - `src/orderbook/book.rs` - Order matching logic
   - `src/exchange/binance.rs` - WebSocket integration
   - `src/main.rs` - Demo flow

2. **Run the tests**:
   ```bash
   cargo test
   ```

3. **Experiment**:
   - Modify order prices in `main.rs`
   - Add more symbols in the BinanceFeed
   - Change the order quantities

4. **Build your own features**:
   - Add a REST API (using Axum)
   - Create a WebSocket server
   - Add order cancellation demo
   - Implement stop-loss orders

## Performance Notes

### Compilation Time
- First build: ~2-3 minutes (downloads and compiles dependencies)
- Subsequent builds: ~10-30 seconds (incremental compilation)
- Release build: Longer but produces optimized binary

### Runtime Performance
- Memory usage: ~20-30 MB
- CPU usage: Minimal (mostly idle waiting for WebSocket data)
- Latency: Sub-millisecond order matching

## Docker (Optional)

```bash
# Build image
docker build -t crypto-orderbook .

# Run container
docker run crypto-orderbook
```

## Exit the Application

Press `Ctrl+C` to stop the application gracefully.
