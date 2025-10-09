use crate::engine::TradingEngine;
use crate::types::*;
use axum::{
    extract::{State, WebSocketUpgrade},
    extract::ws::{WebSocket, Message},
    http::StatusCode,
    Json, response::Response,
    routing::{get, post},
    Router,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::{services::ServeDir, cors::CorsLayer};
use rust_decimal::Decimal;

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderRequest {
    pub client_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: OrderId,
    pub matches: Vec<Order>,
    pub latency_ns: u64,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_orders: u64,
    pub total_matches: u64,
    pub latency_distribution: crate::utils::LatencyDistribution,
    pub active_symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct OrderBookResponse {
    pub symbol: String,
    pub bids: Vec<crate::engine::PriceLevel>,
    pub asks: Vec<crate::engine::PriceLevel>,
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub spread: Option<Decimal>,
    pub mid_price: Option<Decimal>,
}

pub struct WebState {
    pub engine: Arc<TradingEngine>,
}

pub fn create_router(engine: Arc<TradingEngine>) -> Router {
    let state = WebState { engine };

    Router::new()
        .route("/", get(serve_index))
        .route("/api/order", post(submit_order))
        .route("/api/stats", get(get_stats))
        .route("/api/orderbook/:symbol", get(get_orderbook))
        .route("/ws", get(websocket_handler))
        .nest_service("/static", ServeDir::new("src/web/static"))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state))
}

async fn serve_index() -> &'static str {
    include_str!("web/index.html")
}

async fn submit_order(
    State(state): State<Arc<WebState>>,
    Json(req): Json<OrderRequest>,
) -> Result<Json<OrderResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    let order = Order::new(
        req.client_id,
        req.symbol,
        req.side,
        req.order_type,
        req.quantity,
        req.price,
    );

    let order_id = order.id;

    match state.engine.process_order(order).await {
        Ok(matches) => {
            let latency_ns = start_time.elapsed().as_nanos() as u64;
            Ok(Json(OrderResponse {
                order_id,
                matches,
                latency_ns,
            }))
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn get_stats(
    State(state): State<Arc<WebState>>,
) -> Json<StatsResponse> {
    let total_orders = *state.engine.total_orders.read().await;
    let total_matches = *state.engine.total_matches.read().await;
    let latency_distribution = state.engine.latency_tracker.read().await.get_distribution();

    Json(StatsResponse {
        total_orders,
        total_matches,
        latency_distribution,
        active_symbols: vec!["BTCUSDT".to_string(), "ETHUSDT".to_string(), "SOLUSDT".to_string()],
    })
}

async fn get_orderbook(
    State(state): State<Arc<WebState>>,
    axum::extract::Path(symbol): axum::extract::Path<String>,
) -> Result<Json<OrderBookResponse>, StatusCode> {
    if let Some((bids, asks)) = state.engine.get_order_book_snapshot(&symbol).await {
        let order_books = state.engine.order_books.read().await;
        let book = order_books.get(&symbol).unwrap();

        Ok(Json(OrderBookResponse {
            symbol: symbol.clone(),
            bids,
            asks,
            best_bid: book.get_best_bid(),
            best_ask: book.get_best_ask(),
            spread: book.get_spread(),
            mid_price: book.get_mid_price(),
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebState>>,
) -> Response {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}

async fn websocket_connection(mut socket: WebSocket, state: Arc<WebState>) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let stats = get_live_stats(&state).await;
                if let Ok(msg) = serde_json::to_string(&stats) {
                    if socket.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                if msg.is_none() {
                    break;
                }
            }
        }
    }
}

async fn get_live_stats(state: &WebState) -> serde_json::Value {
    let total_orders = *state.engine.total_orders.read().await;
    let total_matches = *state.engine.total_matches.read().await;
    let latency_distribution = state.engine.latency_tracker.read().await.get_distribution();

    // Get order book data for all major symbols
    let btc_book = state.engine.get_order_book_snapshot("BTCUSDT").await;
    let eth_book = state.engine.get_order_book_snapshot("ETHUSDT").await;
    let sol_book = state.engine.get_order_book_snapshot("SOLUSDT").await;

    serde_json::json!({
        "type": "stats_update",
        "timestamp": chrono::Utc::now(),
        "total_orders": total_orders,
        "total_matches": total_matches,
        "latency": {
            "mean_ns": latency_distribution.mean,
            "p50_ns": latency_distribution.p50,
            "p95_ns": latency_distribution.p95,
            "p99_ns": latency_distribution.p99,
            "sample_count": latency_distribution.sample_count
        },
        "order_books": {
            "BTCUSDT": btc_book.map(|(bids, asks)| serde_json::json!({
                "bids": bids.into_iter().take(5).collect::<Vec<_>>(),
                "asks": asks.into_iter().take(5).collect::<Vec<_>>(),
                "spread": calculate_spread(&bids, &asks),
                "mid_price": calculate_mid_price(&bids, &asks)
            })),
            "ETHUSDT": eth_book.map(|(bids, asks)| serde_json::json!({
                "bids": bids.into_iter().take(5).collect::<Vec<_>>(),
                "asks": asks.into_iter().take(5).collect::<Vec<_>>(),
                "spread": calculate_spread(&bids, &asks),
                "mid_price": calculate_mid_price(&bids, &asks)
            })),
            "SOLUSDT": sol_book.map(|(bids, asks)| serde_json::json!({
                "bids": bids.into_iter().take(5).collect::<Vec<_>>(),
                "asks": asks.into_iter().take(5).collect::<Vec<_>>(),
                "spread": calculate_spread(&bids, &asks),
                "mid_price": calculate_mid_price(&bids, &asks)
            }))
        }
    })
}

// Helper functions for calculating spread and mid-price
fn calculate_spread(bids: &[crate::engine::PriceLevel], asks: &[crate::engine::PriceLevel]) -> Option<f64> {
    if let (Some(best_bid), Some(best_ask)) = (bids.first(), asks.first()) {
        Some(best_ask.price.to_f64().unwrap_or(0.0) - best_bid.price.to_f64().unwrap_or(0.0))
    } else {
        None
    }
}

fn calculate_mid_price(bids: &[crate::engine::PriceLevel], asks: &[crate::engine::PriceLevel]) -> Option<f64> {
    if let (Some(best_bid), Some(best_ask)) = (bids.first(), asks.first()) {
        let bid_price = best_bid.price.to_f64().unwrap_or(0.0);
        let ask_price = best_ask.price.to_f64().unwrap_or(0.0);
        Some((bid_price + ask_price) / 2.0)
    } else {
        None
    }
}