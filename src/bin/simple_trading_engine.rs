// Simple Live Trading Engine - Rust Project Showcase
// Demonstrates: async Rust, WebSocket handling, REST APIs, real-time data processing

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;
use axum::{
    extract::State,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Binance WebSocket ticker structure
#[derive(Debug, Deserialize)]
pub struct BinanceTicker {
    pub s: String,   // Symbol
    pub c: String,   // Current price
}

/// Binance WebSocket order book depth structure
#[derive(Debug, Deserialize)]
pub struct BinanceDepth {
    pub s: String,                    // Symbol
    pub b: Vec<(String, String)>,     // Bids [price, quantity]
    pub a: Vec<(String, String)>,     // Asks [price, quantity]
}

/// Trading order representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Position tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub average_price: f64,
    pub market_value: f64,
    pub pnl: f64,
}

/// Market depth snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDepth {
    pub symbol: String,
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_quantity: f64,
    pub ask_quantity: f64,
    pub spread: f64,
    pub mid_price: f64,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// Portfolio state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub cash_balance: f64,
    pub total_value: f64,
    pub positions: HashMap<String, Position>,
    pub total_pnl: f64,
}

/// Order submission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: Option<f64>,
}

/// Standard API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

/// Main trading engine state
#[derive(Clone)]
pub struct TradingEngine {
    orders: Arc<RwLock<Vec<Order>>>,
    portfolio: Arc<RwLock<Portfolio>>,
    current_prices: Arc<RwLock<HashMap<String, f64>>>,
    market_depth: Arc<RwLock<HashMap<String, MarketDepth>>>,
    order_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl TradingEngine {
    pub fn new() -> Self {
        let portfolio = Portfolio {
            cash_balance: 100000.0,
            total_value: 100000.0,
            positions: HashMap::new(),
            total_pnl: 0.0,
        };

        Self {
            orders: Arc::new(RwLock::new(Vec::new())),
            portfolio: Arc::new(RwLock::new(portfolio)),
            current_prices: Arc::new(RwLock::new(HashMap::new())),
            market_depth: Arc::new(RwLock::new(HashMap::new())),
            order_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Start live price feeds from Binance WebSocket
    pub async fn start_price_feeds(&self) -> Result<(), Box<dyn std::error::Error>> {
        let symbols = ["btcusdt", "ethusdt", "solusdt"];

        // Ticker feed URL
        let ticker_stream_names: Vec<String> = symbols.iter()
            .map(|s| format!("{}@ticker", s))
            .collect();
        let ticker_url = format!(
            "wss://stream.binance.com:9443/ws/{}",
            ticker_stream_names.join("/")
        );

        // Depth feed URL
        let depth_stream_names: Vec<String> = symbols.iter()
            .map(|s| format!("{}@depth5@100ms", s))
            .collect();
        let depth_url = format!(
            "wss://stream.binance.com:9443/ws/{}",
            depth_stream_names.join("/")
        );

        let prices = Arc::clone(&self.current_prices);
        let depth = Arc::clone(&self.market_depth);

        // Spawn ticker feed task
        let ticker_prices = Arc::clone(&prices);
        tokio::spawn(async move {
            loop {
                match connect_async(&ticker_url).await {
                    Ok((ws_stream, _)) => {
                        println!("🔗 Connected to Binance ticker feed");
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(ticker) = serde_json::from_str::<BinanceTicker>(&text) {
                                        let symbol = ticker.s.to_uppercase();
                                        let price: f64 = ticker.c.parse().unwrap_or(0.0);
                                        ticker_prices.write().await.insert(symbol.clone(), price);
                                        println!("📈 Price: {} = ${:.2}", symbol, price);
                                    }
                                }
                                Ok(Message::Close(_)) => break,
                                Err(e) => {
                                    eprintln!("❌ WebSocket error: {}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => eprintln!("❌ Connection failed: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        // Spawn depth feed task
        tokio::spawn(async move {
            loop {
                match connect_async(&depth_url).await {
                    Ok((ws_stream, _)) => {
                        println!("📊 Connected to Binance depth feed");
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(depth_data) = serde_json::from_str::<BinanceDepth>(&text) {
                                        let symbol = depth_data.s.to_uppercase();

                                        if let (Some(best_bid), Some(best_ask)) = (
                                            depth_data.b.first(),
                                            depth_data.a.first()
                                        ) {
                                            let bid_price = best_bid.0.parse::<f64>().unwrap_or(0.0);
                                            let ask_price = best_ask.0.parse::<f64>().unwrap_or(0.0);
                                            let bid_quantity = best_bid.1.parse::<f64>().unwrap_or(0.0);
                                            let ask_quantity = best_ask.1.parse::<f64>().unwrap_or(0.0);
                                            let spread = ask_price - bid_price;
                                            let mid_price = (bid_price + ask_price) / 2.0;

                                            let market_depth = MarketDepth {
                                                symbol: symbol.clone(),
                                                bid_price,
                                                ask_price,
                                                bid_quantity,
                                                ask_quantity,
                                                spread,
                                                mid_price,
                                                last_update: chrono::Utc::now(),
                                            };

                                            depth.write().await.insert(symbol, market_depth);
                                        }
                                    }
                                }
                                Ok(Message::Close(_)) => break,
                                Err(e) => {
                                    eprintln!("❌ Depth WebSocket error: {}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => eprintln!("❌ Depth connection failed: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        Ok(())
    }

    /// Submit and execute an order
    pub async fn submit_order(&self, request: OrderRequest) -> Result<Order, String> {
        // Get realistic execution price from market depth
        let (fill_price, execution_notes) = {
            let depth = self.market_depth.read().await;
            match depth.get(&request.symbol) {
                Some(market_data) => {
                    let is_buy = request.side.to_lowercase() == "buy";
                    let realistic_price = if is_buy {
                        request.price.unwrap_or(market_data.ask_price).max(market_data.ask_price)
                    } else {
                        request.price.unwrap_or(market_data.bid_price).min(market_data.bid_price)
                    };
                    (realistic_price, format!("Executed at {} price", if is_buy { "ask" } else { "bid" }))
                }
                None => {
                    let prices = self.current_prices.read().await;
                    let price = *prices.get(&request.symbol).ok_or("Symbol not found")?;
                    (request.price.unwrap_or(price), "Executed at ticker price".to_string())
                }
            }
        };

        // Create order
        let order_id = self.order_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let order = Order {
            id: order_id,
            symbol: request.symbol.clone(),
            side: request.side.clone(),
            quantity: request.quantity,
            price: Some(fill_price),
            status: "filled".to_string(),
            timestamp: Utc::now(),
        };

        println!("⚡ Order #{}: {} {} @ ${:.2} - {}",
                order_id, request.side.to_uppercase(), request.symbol,
                fill_price, execution_notes);

        // Update portfolio
        {
            let mut portfolio = self.portfolio.write().await;
            self.update_portfolio(&mut portfolio, &order).await;
        }

        // Store order
        self.orders.write().await.push(order.clone());

        Ok(order)
    }

    async fn update_portfolio(&self, portfolio: &mut Portfolio, order: &Order) {
        let trade_value = order.quantity * order.price.unwrap_or(0.0);
        let is_buy = order.side.to_lowercase() == "buy";

        // Update cash
        if is_buy {
            portfolio.cash_balance -= trade_value;
        } else {
            portfolio.cash_balance += trade_value;
        }

        // Update position
        let position = portfolio.positions.entry(order.symbol.clone())
            .or_insert(Position {
                symbol: order.symbol.clone(),
                quantity: 0.0,
                average_price: 0.0,
                market_value: 0.0,
                pnl: 0.0,
            });

        if is_buy {
            let total_cost = position.quantity * position.average_price + trade_value;
            position.quantity += order.quantity;
            if position.quantity > 0.0 {
                position.average_price = total_cost / position.quantity;
            }
        } else {
            position.quantity -= order.quantity;
            if position.quantity.abs() < 0.0001 {
                position.quantity = 0.0;
                position.average_price = 0.0;
            }
        }

        // Update market value with current price
        if let Some(current_price) = self.current_prices.read().await.get(&order.symbol) {
            position.market_value = position.quantity * current_price;
            position.pnl = (current_price - position.average_price) * position.quantity;
        }

        // Recalculate totals
        portfolio.total_value = portfolio.cash_balance + portfolio.positions.values()
            .map(|p| p.market_value).sum::<f64>();
        portfolio.total_pnl = portfolio.positions.values()
            .map(|p| p.pnl).sum::<f64>();
    }

    pub async fn refresh_portfolio(&self) {
        let mut portfolio = self.portfolio.write().await;
        let prices = self.current_prices.read().await;

        for (symbol, position) in portfolio.positions.iter_mut() {
            if let Some(&current_price) = prices.get(symbol) {
                position.market_value = position.quantity * current_price;
                position.pnl = (current_price - position.average_price) * position.quantity;
            }
        }

        portfolio.total_value = portfolio.cash_balance + portfolio.positions.values()
            .map(|p| p.market_value).sum::<f64>();
        portfolio.total_pnl = portfolio.positions.values()
            .map(|p| p.pnl).sum::<f64>();
    }

    pub async fn get_portfolio(&self) -> Portfolio {
        self.portfolio.read().await.clone()
    }

    pub async fn get_orders(&self) -> Vec<Order> {
        self.orders.read().await.clone()
    }

    pub async fn get_market_depth(&self) -> HashMap<String, MarketDepth> {
        self.market_depth.read().await.clone()
    }

    pub async fn get_prices(&self) -> HashMap<String, f64> {
        self.current_prices.read().await.clone()
    }
}

// === HTTP Handlers ===

async fn submit_order_handler(
    State(engine): State<TradingEngine>,
    Json(request): Json<OrderRequest>,
) -> Json<ApiResponse<Order>> {
    match engine.submit_order(request).await {
        Ok(order) => Json(ApiResponse {
            success: true,
            data: Some(order),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            data: None,
            error: Some(e),
        }),
    }
}

async fn get_portfolio_handler(
    State(engine): State<TradingEngine>,
) -> Json<ApiResponse<Portfolio>> {
    engine.refresh_portfolio().await;
    let portfolio = engine.get_portfolio().await;
    Json(ApiResponse {
        success: true,
        data: Some(portfolio),
        error: None,
    })
}

async fn get_orders_handler(
    State(engine): State<TradingEngine>,
) -> Json<ApiResponse<Vec<Order>>> {
    let orders = engine.get_orders().await;
    Json(ApiResponse {
        success: true,
        data: Some(orders),
        error: None,
    })
}

async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        data: Some("Live Trading Engine - Binance WebSocket Connected".to_string()),
        error: None,
    })
}

async fn get_prices_handler(
    State(engine): State<TradingEngine>,
) -> Json<ApiResponse<HashMap<String, f64>>> {
    let prices = engine.get_prices().await;
    Json(ApiResponse {
        success: true,
        data: Some(prices),
        error: None,
    })
}

async fn get_depth_handler(
    State(engine): State<TradingEngine>,
) -> Json<ApiResponse<HashMap<String, MarketDepth>>> {
    let depth = engine.get_market_depth().await;
    Json(ApiResponse {
        success: true,
        data: Some(depth),
        error: None,
    })
}

async fn serve_frontend() -> Html<&'static str> {
    Html(include_str!("../web/index.html"))
}

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
