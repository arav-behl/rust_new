// Shuttle.rs deployment entry point
use shuttle_axum::ShuttleAxum;
use axum::{
    extract::State,
    response::{Html, Json},
    routing::{get, post},
    Router,
};

// Import everything from the binary
#[path = "bin/simple_trading_engine.rs"]
mod simple_engine;

use simple_engine::*;

#[shuttle_runtime::main]
async fn main() -> ShuttleAxum {
    tracing_subscriber::fmt::init();

    println!("🚀 Simple Live Trading Engine - Shuttle Deployment");
    println!("================================");

    let engine = TradingEngine::new();

    println!("🔗 Connecting to Binance WebSocket...");
    let _ = engine.start_price_feeds().await;

    // Wait for initial data
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let router = Router::new()
        .route("/", get(serve_frontend))
        .route("/health", get(health_check))
        .route("/api/v1/orders", post(submit_order_handler))
        .route("/api/v1/orders", get(get_orders_handler))
        .route("/api/v1/portfolio", get(get_portfolio_handler))
        .route("/api/v1/prices", get(get_prices_handler))
        .route("/api/v1/depth", get(get_depth_handler))
        .with_state(engine);

    println!("📋 API Ready!");

    Ok(router.into())
}
