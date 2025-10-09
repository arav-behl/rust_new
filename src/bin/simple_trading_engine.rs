// Simple Trading Engine - Standalone Binary
use orderbook_engine::*;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🚀 Simple Live Trading Engine");
    println!("================================");
    println!("Demonstrates: Rust + async + WebSocket + REST APIs");
    println!();

    let engine = TradingEngine::new();

    println!("🔗 Connecting to Binance WebSocket...");
    engine.start_price_feeds().await?;

    // Wait for initial data
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let app = Router::new()
        .route("/", get(serve_frontend))
        .route("/health", get(health_check))
        .route("/api/v1/orders", post(submit_order_handler))
        .route("/api/v1/orders", get(get_orders_handler))
        .route("/api/v1/portfolio", get(get_portfolio_handler))
        .route("/api/v1/prices", get(get_prices_handler))
        .route("/api/v1/depth", get(get_depth_handler))
        .with_state(engine);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?;
    println!("🌐 Server running on http://{}", addr);
    println!("📋 API Endpoints:");
    println!("   GET  /health");
    println!("   GET  /api/v1/prices");
    println!("   GET  /api/v1/depth");
    println!("   POST /api/v1/orders");
    println!("   GET  /api/v1/portfolio");
    println!();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
