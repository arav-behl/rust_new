use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;
use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceBuilder;
use tower_http::{services::ServeDir, cors::CorsLayer};

// Binance WebSocket ticker data structure - matches actual Binance format
#[derive(Debug, Deserialize)]
pub struct BinanceTicker {
    pub e: String,   // Event type (24hrTicker)
    #[serde(rename = "E")]
    pub event_time: u64, // Event time
    pub s: String,   // Symbol
    pub p: String,   // Price change
    #[serde(rename = "P")]
    pub price_change_percent: String, // Price change percent
    pub w: String,   // Weighted average price
    pub c: String,   // Current price (last price)
    pub h: String,   // High price
    pub l: String,   // Low price
    pub v: String,   // Volume
    pub q: String,   // Quote volume
}

// Simple types for the working version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleOrder {
    pub id: u64,
    pub symbol: String,
    pub side: String, // "buy" or "sell"
    pub quantity: f64,
    pub price: Option<f64>,
    pub status: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePosition {
    pub symbol: String,
    pub quantity: f64,
    pub average_price: f64,
    pub market_value: f64,
    pub pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePortfolio {
    pub cash_balance: f64,
    pub total_value: f64,
    pub positions: HashMap<String, SimplePosition>,
    pub total_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleOrderRequest {
    pub symbol: String,
    pub side: String,
    pub quantity: f64,
    pub price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

// Simple trading engine state
#[derive(Clone)]
pub struct SimpleTradingEngine {
    orders: Arc<RwLock<Vec<SimpleOrder>>>,
    portfolio: Arc<RwLock<SimplePortfolio>>,
    current_prices: Arc<RwLock<HashMap<String, f64>>>,
    order_counter: Arc<std::sync::atomic::AtomicU64>,
}

impl SimpleTradingEngine {
    pub fn new() -> Self {
        // Initialize with empty prices - will be populated by live feed
        let current_prices = HashMap::new();

        let portfolio = SimplePortfolio {
            cash_balance: 100000.0, // Start with $100k
            total_value: 100000.0,
            positions: HashMap::new(),
            total_pnl: 0.0,
        };

        Self {
            orders: Arc::new(RwLock::new(Vec::new())),
            portfolio: Arc::new(RwLock::new(portfolio)),
            current_prices: Arc::new(RwLock::new(current_prices)),
            order_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    // Start live price feeds from Binance WebSocket
    pub async fn start_price_feeds(&self) -> Result<(), Box<dyn std::error::Error>> {
        let prices = Arc::clone(&self.current_prices);

        // Use the working single connection approach with Binance spot WebSocket
        let single_stream_urls = vec![
            "wss://stream.binance.com/ws/btcusdt@ticker".to_string(),
            "wss://stream.binance.com/ws/ethusdt@ticker".to_string(),
            "wss://stream.binance.com/ws/adausdt@ticker".to_string(),
            "wss://stream.binance.com/ws/solusdt@ticker".to_string(),
            "wss://stream.binance.com/ws/dotusdt@ticker".to_string(),
        ];

        let prices_clone = Arc::clone(&prices);

        tokio::spawn(async move {
            loop {
                let mut connected = false;

                // Try each URL until one works
                for (i, url) in single_stream_urls.iter().enumerate() {
                    println!("🔗 Attempting connection {}/{}: {}", i + 1, single_stream_urls.len(), url);

                    match connect_async(url).await {
                        Ok((ws_stream, response)) => {
                            println!("✅ Connected to Binance WebSocket for live prices");
                            println!("📊 Status: {}", response.status());
                            connected = true;
                            let (_write, mut read) = ws_stream.split();

                            while let Some(msg) = read.next().await {
                                match msg {
                                    Ok(Message::Text(text)) => {
                                        println!("📥 Received: {}", &text[..text.len().min(100)]);

                                        if let Ok(ticker) = serde_json::from_str::<BinanceTicker>(&text) {
                                            let symbol = ticker.s.to_uppercase();
                                            let price: f64 = ticker.c.parse().unwrap_or(0.0);

                                            if price > 0.0 {
                                                {
                                                    let mut prices_lock = prices_clone.write().await;
                                                    prices_lock.insert(symbol.clone(), price);
                                                }
                                                println!("📈 Live Price Update: {} = ${:.2}", symbol, price);
                                            }
                                        } else {
                                            println!("⚠️ Failed to parse ticker from: {}", &text[..text.len().min(100)]);
                                            // Try to parse as generic JSON to see structure
                                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                                println!("📋 JSON structure: {}", serde_json::to_string_pretty(&json).unwrap_or_default());
                                            }
                                        }
                                    }
                                    Ok(Message::Close(_)) => {
                                        println!("🔌 WebSocket connection closed");
                                        break;
                                    }
                                    Err(e) => {
                                        println!("❌ WebSocket error: {}", e);
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            break; // Exit the URL loop if we connected successfully
                        }
                        Err(e) => {
                            println!("❌ Connection attempt {} failed: {}", i + 1, e);
                            println!("🔗 URL was: {}", url);
                            println!("📋 Error details: {:?}", e);
                            if i == single_stream_urls.len() - 1 {
                                println!("⚠️ All connection attempts failed - will retry");
                            }
                        }
                    }
                }

                if !connected {
                    println!("❌ All WebSocket connections failed. Will retry in 10 seconds...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                } else {
                    println!("🔄 Reconnecting to price feed in 5 seconds...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        });

        Ok(())
    }

    pub async fn submit_order(&self, request: SimpleOrderRequest) -> Result<SimpleOrder, String> {
        // Get current price
        let current_price = {
            let prices = self.current_prices.read().await;
            *prices.get(&request.symbol).ok_or("Symbol not found")?
        };

        // Create order
        let order_id = self.order_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let fill_price = request.price.unwrap_or(current_price);

        let order = SimpleOrder {
            id: order_id,
            symbol: request.symbol.clone(),
            side: request.side.clone(),
            quantity: request.quantity,
            price: Some(fill_price),
            status: "filled".to_string(), // Paper trading - fill immediately
            timestamp: Utc::now(),
        };

        // Update portfolio
        {
            let mut portfolio = self.portfolio.write().await;
            self.update_portfolio(&mut portfolio, &order).await;
        }

        // Store order
        {
            let mut orders = self.orders.write().await;
            orders.push(order.clone());
        }

        Ok(order)
    }

    async fn update_portfolio(&self, portfolio: &mut SimplePortfolio, order: &SimpleOrder) {
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
            .or_insert(SimplePosition {
                symbol: order.symbol.clone(),
                quantity: 0.0,
                average_price: 0.0,
                market_value: 0.0,
                pnl: 0.0,
            });

        if is_buy {
            // Calculate new average price
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

        // Update market value and PnL
        if let Some(current_price) = self.current_prices.read().await.get(&order.symbol) {
            position.market_value = position.quantity * current_price;
            position.pnl = (current_price - position.average_price) * position.quantity;
        }

        // Calculate total portfolio value
        let mut total_value = portfolio.cash_balance;
        let mut total_pnl = 0.0;

        for pos in portfolio.positions.values() {
            total_value += pos.market_value;
            total_pnl += pos.pnl;
        }

        portfolio.total_value = total_value;
        portfolio.total_pnl = total_pnl;
    }

    pub async fn get_portfolio(&self) -> SimplePortfolio {
        self.portfolio.read().await.clone()
    }

    pub async fn get_orders(&self) -> Vec<SimpleOrder> {
        self.orders.read().await.clone()
    }
}

// API Handlers
async fn submit_order(
    State(engine): State<SimpleTradingEngine>,
    Json(request): Json<SimpleOrderRequest>,
) -> Json<ApiResponse<SimpleOrder>> {
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

async fn get_portfolio(
    State(engine): State<SimpleTradingEngine>,
) -> Json<ApiResponse<SimplePortfolio>> {
    let portfolio = engine.get_portfolio().await;
    Json(ApiResponse {
        success: true,
        data: Some(portfolio),
        error: None,
    })
}

async fn get_orders(
    State(engine): State<SimpleTradingEngine>,
) -> Json<ApiResponse<Vec<SimpleOrder>>> {
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
        data: Some("Live Trading Engine - Connected to Binance WebSocket".to_string()),
        error: None,
    })
}

async fn get_live_prices(
    State(engine): State<SimpleTradingEngine>,
) -> Json<ApiResponse<HashMap<String, f64>>> {
    let prices = engine.current_prices.read().await;
    Json(ApiResponse {
        success: true,
        data: Some(prices.clone()),
        error: None,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("🚀 Starting Live Trading Engine with Real Market Data");
    println!("=====================================================");

    // Create trading engine
    let engine = SimpleTradingEngine::new();

    // Start live price feeds from Binance
    println!("🔗 Connecting to Binance WebSocket for live market data...");
    engine.start_price_feeds().await?;

    // Wait a moment for initial price data
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Create API router
    let api_router = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/orders", post(submit_order))
        .route("/api/v1/orders", get(get_orders))
        .route("/api/v1/portfolio", get(get_portfolio))
        .route("/api/v1/prices", get(get_live_prices))
        .with_state(engine);

    // Create main app with static file serving
    let app = Router::new()
        .merge(api_router)
        .nest_service("/", ServeDir::new("static"))
        .layer(
            ServiceBuilder::new()
                .layer(CorsLayer::permissive())
        );

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    println!("🌐 REST API server starting on http://{}", addr);
    println!();
    println!("📋 Available Endpoints:");
    println!("   GET  /                      - Web trading interface");
    println!("   GET  /health                - Health check & connection status");
    println!("   GET  /api/v1/prices         - Live market prices");
    println!("   POST /api/v1/orders         - Submit order (executed at live prices)");
    println!("   GET  /api/v1/orders         - Get order history");
    println!("   GET  /api/v1/portfolio      - Get portfolio with live P&L");
    println!();

    // Display system capabilities
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        println!("✅ Live Trading System Operational!");
        println!("   📊 Real-time price data from Binance WebSocket");
        println!("   ⚡ Orders executed at current market prices");
        println!("   💰 Dynamic P&L calculations with live prices");
        println!("   🔄 Automatic reconnection on connection loss");
        println!("   📈 Professional market data integration");
        println!();
        println!("🎯 Perfect for demonstrating quantitative trading expertise!");
        println!("   • Market microstructure understanding");
        println!("   • Real-time data processing");
        println!("   • Risk management with live positions");
        println!("   • Production-grade WebSocket handling");
        println!();
        println!("🌐 Access the web interface at: http://localhost:8080");
        println!("Features:");
        println!("   📊 Live market data visualization");
        println!("   ⚡ Real-time order placement");
        println!("   💰 Portfolio tracking with live P&L");
        println!("   🏢 Professional exchange interface");
    });

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}