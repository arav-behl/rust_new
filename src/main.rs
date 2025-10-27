// High-Performance Cryptocurrency Trading Engine
// Demonstrates: WebSocket feeds, Order book matching, Async Rust, Market microstructure

use crypto_orderbook::{BinanceFeed, Order, OrderSide, SharedOrderBook};
use std::io::{self, Write};

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("\n🚀 High-Performance Crypto Order Book Engine");
    println!("==============================================\n");

    // Create order book for BTC/USDT
    let orderbook = SharedOrderBook::new("BTCUSDT".to_string());

    // Initialize Binance WebSocket feeds
    let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string(), "SOLUSDT".to_string()];
    let feed = BinanceFeed::new(symbols);

    // Start market data feeds
    feed.start_price_feed().await;
    feed.start_depth_feed(orderbook.clone()).await;

    println!("✓ Connected to Binance WebSocket feeds");
    println!("✓ Streaming live market data...\n");

    // Wait for initial data
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Demo: Add some orders to the book
    println!("\n📖 Demonstrating Order Book Operations:");
    println!("========================================\n");

    // Add sell orders
    println!("Adding SELL orders to the book:");
    let sell1 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50100.0, 0.5);
    println!("  Order #{}: SELL 0.5 BTC @ $50,100", sell1.id.0);
    orderbook.add_order(sell1);

    let sell2 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50200.0, 1.0);
    println!("  Order #{}: SELL 1.0 BTC @ $50,200", sell2.id.0);
    orderbook.add_order(sell2);

    let sell3 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Sell, 50150.0, 0.75);
    println!("  Order #{}: SELL 0.75 BTC @ $50,150", sell3.id.0);
    orderbook.add_order(sell3);

    // Add buy orders
    println!("\nAdding BUY orders to the book:");
    let buy1 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Buy, 49900.0, 0.3);
    println!("  Order #{}: BUY 0.3 BTC @ $49,900", buy1.id.0);
    orderbook.add_order(buy1);

    let buy2 = Order::new_limit("BTCUSDT".to_string(), OrderSide::Buy, 49800.0, 0.8);
    println!("  Order #{}: BUY 0.8 BTC @ $49,800", buy2.id.0);
    orderbook.add_order(buy2);

    // Show order book state
    println!("\n📊 Order Book Depth (Top 5 levels):");
    println!("====================================");
    let (bids, asks) = orderbook.get_depth(5);

    println!("\nAsks (Sell Orders):");
    for (price, qty) in asks.iter().rev() {
        println!("  ${:>10.2}  |  {:.4} BTC", price, qty);
    }

    if let Some(spread) = orderbook.spread() {
        println!("\n  --- Spread: ${:.2} ---", spread);
    }

    println!("\nBids (Buy Orders):");
    for (price, qty) in bids.iter() {
        println!("  ${:>10.2}  |  {:.4} BTC", price, qty);
    }

    // Demo: Match an order
    println!("\n\n⚡ Executing Market Order:");
    println!("=========================");
    let market_buy = Order::new_limit("BTCUSDT".to_string(), OrderSide::Buy, 50200.0, 1.0);
    println!("\nPlacing BUY order: 1.0 BTC @ market (limit: $50,200)");

    let trades = orderbook.add_order(market_buy);

    if !trades.is_empty() {
        println!("\n✓ Order matched! Generated {} trade(s):", trades.len());
        for trade in &trades {
            println!(
                "  • Trade: {:.4} BTC @ ${:.2} (Maker: #{}, Taker: #{})",
                trade.quantity,
                trade.price,
                trade.maker_order_id.0,
                trade.taker_order_id.0
            );
        }
    }

    // Show updated book
    println!("\n📊 Updated Order Book:");
    println!("======================");
    let (bids, asks) = orderbook.get_depth(5);

    println!("\nAsks:");
    for (price, qty) in asks.iter().rev() {
        println!("  ${:>10.2}  |  {:.4} BTC", price, qty);
    }

    if let Some(spread) = orderbook.spread() {
        println!("\n  --- Spread: ${:.2} ---", spread);
    }

    println!("\nBids:");
    for (price, qty) in bids.iter() {
        println!("  ${:>10.2}  |  {:.4} BTC", price, qty);
    }

    println!("\n📈 Order Book Statistics:");
    println!("========================");
    println!("  Active Orders: {}", orderbook.order_count());
    if let Some(bid) = orderbook.best_bid() {
        println!("  Best Bid: ${:.2}", bid);
    }
    if let Some(ask) = orderbook.best_ask() {
        println!("  Best Ask: ${:.2}", ask);
    }
    if let Some(mid) = orderbook.mid_price() {
        println!("  Mid Price: ${:.2}", mid);
    }

    // Show live market data
    println!("\n\n📡 Live Market Data from Binance:");
    println!("==================================");

    let market_data = feed.get_market_data().await;
    for data in market_data {
        println!(
            "  {} | Price: ${:.2} | Bid: ${:.2} | Ask: ${:.2} | Spread: ${:.2}",
            data.symbol, data.price, data.bid_price, data.ask_price, data.spread
        );
    }

    println!("\n\n🎯 Key Features Demonstrated:");
    println!("============================");
    println!("  ✓ Real-time WebSocket data ingestion from Binance");
    println!("  ✓ High-performance order book with price-time priority");
    println!("  ✓ Order matching engine with trade generation");
    println!("  ✓ BTreeMap-based price levels for O(log n) operations");
    println!("  ✓ Async Rust with Tokio for concurrent operations");
    println!("  ✓ Thread-safe shared state with Arc<Mutex>");
    println!("  ✓ Market microstructure concepts (bid/ask, spread, depth)");

    println!("\n📚 Technical Skills Showcased:");
    println!("==============================");
    println!("  • Async/await and concurrent task spawning");
    println!("  • WebSocket protocol handling");
    println!("  • Advanced data structures (BTreeMap, VecDeque)");
    println!("  • Order matching algorithms");
    println!("  • Client-server communication patterns");
    println!("  • Memory-efficient design with smart pointers");

    println!("\n💡 Press Ctrl+C to exit (feeds will continue streaming)...\n");

    // Keep running to show live data
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

        // Periodically show market updates
        let market_data = feed.get_market_data().await;
        print!("\r");
        if let Some(btc) = market_data.iter().find(|d| d.symbol == "BTCUSDT") {
            print!("📊 BTC: ${:.2} | ", btc.price);
        }
        if let Some(eth) = market_data.iter().find(|d| d.symbol == "ETHUSDT") {
            print!("ETH: ${:.2} | ", eth.price);
        }
        if let Some(sol) = market_data.iter().find(|d| d.symbol == "SOLUSDT") {
            print!("SOL: ${:.2}", sol.price);
        }
        io::stdout().flush().unwrap();
    }
}
