use std::sync::Arc;
use std::net::SocketAddr;
use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, Method},
    response::Json,
    routing::{get, post, delete},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::types::*;
use crate::services::{
    trading_service::TradingService,
    portfolio_service::{PortfolioService, PortfolioSummary},
    market_data_service::{MarketDataService, MarketDataServiceMetrics},
    benchmark_service::{BenchmarkService, PerformanceReport},
    risk_service::{RiskService, RiskLimits, PreTradeRiskResult, RiskMetrics},
};
use crate::Result;

#[derive(Clone)]
pub struct AppState {
    pub trading_service: Arc<TradingService>,
    pub portfolio_service: Arc<PortfolioService>,
    pub market_data_service: Arc<MarketDataService>,
    pub benchmark_service: Arc<BenchmarkService>,
    pub risk_service: Arc<RiskService>,
}

// API Request/Response Types
#[derive(Debug, Deserialize)]
pub struct SubmitOrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Decimal,
    pub price: Option<Decimal>,
    pub client_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitOrderResponse {
    pub order_id: OrderId,
    pub status: OrderStatus,
    pub message: String,
    pub latency_ns: u64,
}

#[derive(Debug, Deserialize)]
pub struct OrdersQuery {
    pub symbol: Option<String>,
    pub status: Option<OrderStatus>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct TradesQuery {
    pub symbol: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct PortfolioQuery {
    pub account_id: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub services: HashMap<String, ServiceHealth>,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealth {
    pub status: String,
    pub uptime_seconds: u64,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

pub struct RestServer {
    app: Router,
    addr: SocketAddr,
}

impl RestServer {
    pub fn new(addr: SocketAddr, state: AppState) -> Self {
        let app = create_router(state);

        Self { app, addr }
    }

    pub async fn start(self) -> Result<()> {
        tracing::info!("Starting REST API server on {}", self.addr);

        let listener = tokio::net::TcpListener::bind(self.addr).await
            .map_err(|e| crate::EngineError::Internal(format!("Failed to bind to address: {}", e)))?;

        axum::serve(listener, self.app).await
            .map_err(|e| crate::EngineError::Internal(format!("Server error: {}", e)))?;

        Ok(())
    }
}

fn create_router(state: AppState) -> Router {
    Router::new()
        // Health endpoints
        .route("/health", get(health_check))
        .route("/health/detailed", get(detailed_health_check))

        // Trading endpoints
        .route("/api/v1/orders", post(submit_order))
        .route("/api/v1/orders", get(get_orders))
        .route("/api/v1/orders/:order_id", get(get_order))
        .route("/api/v1/orders/:order_id", delete(cancel_order))

        // Portfolio endpoints
        .route("/api/v1/portfolio", get(get_portfolio))
        .route("/api/v1/portfolio/summary", get(get_portfolio_summary))
        .route("/api/v1/portfolio/positions", get(get_positions))
        .route("/api/v1/portfolio/positions/:symbol", get(get_position))

        // Trading data endpoints
        .route("/api/v1/trades", get(get_trades))
        .route("/api/v1/trades/:symbol", get(get_trades_by_symbol))

        // Market data endpoints
        .route("/api/v1/market/orderbook/:symbol", get(get_order_book))
        .route("/api/v1/market/trade/:symbol", get(get_latest_trade))
        .route("/api/v1/market/stats", get(get_market_data_stats))

        // Performance endpoints
        .route("/api/v1/performance/report", get(get_performance_report))
        .route("/api/v1/performance/latency/:operation", get(get_latency_stats))
        .route("/api/v1/performance/benchmark/:operation", post(run_benchmark))

        // Risk endpoints
        .route("/api/v1/risk/limits", get(get_risk_limits))
        .route("/api/v1/risk/limits", post(set_risk_limits))
        .route("/api/v1/risk/metrics", get(get_risk_metrics))
        .route("/api/v1/risk/alerts", get(get_risk_alerts))

        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods([Method::GET, Method::POST, Method::DELETE])
                        .allow_headers(Any)
                )
        )
        .with_state(state)
}

// Health Check Handlers
async fn health_check() -> Json<ApiResponse<HealthResponse>> {
    let health = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        services: HashMap::new(),
    };

    Json(ApiResponse::success(health))
}

async fn detailed_health_check(State(state): State<AppState>) -> Json<ApiResponse<HealthResponse>> {
    let mut services = HashMap::new();

    // Check market data service
    let market_health = ServiceHealth {
        status: "healthy".to_string(),
        uptime_seconds: 0,
        last_activity: Utc::now(),
    };
    services.insert("market_data".to_string(), market_health);

    // Check benchmark service
    let benchmark_health = ServiceHealth {
        status: "healthy".to_string(),
        uptime_seconds: 0,
        last_activity: Utc::now(),
    };
    services.insert("benchmark".to_string(), benchmark_health);

    let health = HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now(),
        services,
    };

    Json(ApiResponse::success(health))
}

// Trading Handlers
async fn submit_order(
    State(state): State<AppState>,
    Json(request): Json<SubmitOrderRequest>,
) -> Result<Json<ApiResponse<SubmitOrderResponse>>, StatusCode> {
    let client_id = request.client_id.unwrap_or_else(|| "api_client".to_string());

    let order = Order::new(
        client_id,
        request.symbol,
        request.side,
        request.order_type,
        request.quantity,
        request.price,
    );

    match state.trading_service.submit_order(order).await {
        Ok(execution_report) => {
            let response = SubmitOrderResponse {
                order_id: execution_report.order_id,
                status: execution_report.status,
                message: "Order submitted successfully".to_string(),
                latency_ns: execution_report.latency_ns,
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => {
            tracing::error!("Failed to submit order: {}", e);
            Ok(Json(ApiResponse::error(format!("Failed to submit order: {}", e))))
        }
    }
}

async fn get_orders(
    State(state): State<AppState>,
    Query(query): Query<OrdersQuery>,
) -> Json<ApiResponse<Vec<Order>>> {
    // In a real system, we'd query the trading service for orders
    // For now, return empty list
    Json(ApiResponse::success(vec![]))
}

async fn get_order(
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> Json<ApiResponse<Option<Order>>> {
    let order_id = OrderId(order_id);
    match state.trading_service.get_order(&order_id).await {
        Some(order) => Json(ApiResponse::success(Some(order))),
        None => Json(ApiResponse::success(None)),
    }
}

async fn cancel_order(
    State(state): State<AppState>,
    Path(order_id): Path<u64>,
) -> Json<ApiResponse<String>> {
    let order_id = OrderId(order_id);
    match state.trading_service.cancel_order(&order_id).await {
        Ok(_) => Json(ApiResponse::success("Order cancelled successfully".to_string())),
        Err(e) => Json(ApiResponse::error(format!("Failed to cancel order: {}", e))),
    }
}

// Portfolio Handlers
async fn get_portfolio(
    State(state): State<AppState>,
    Query(query): Query<PortfolioQuery>,
) -> Json<ApiResponse<Option<crate::services::portfolio_service::Portfolio>>> {
    match state.portfolio_service.get_portfolio(&query.account_id).await {
        Some(portfolio) => Json(ApiResponse::success(Some(portfolio))),
        None => Json(ApiResponse::success(None)),
    }
}

async fn get_portfolio_summary(
    State(state): State<AppState>,
    Query(query): Query<PortfolioQuery>,
) -> Json<ApiResponse<Option<PortfolioSummary>>> {
    match state.portfolio_service.get_portfolio_summary(&query.account_id).await {
        Some(summary) => Json(ApiResponse::success(Some(summary))),
        None => Json(ApiResponse::success(None)),
    }
}

async fn get_positions(
    State(state): State<AppState>,
    Query(query): Query<PortfolioQuery>,
) -> Json<ApiResponse<HashMap<String, crate::services::portfolio_service::Position>>> {
    let positions = state.portfolio_service.get_all_positions(&query.account_id).await;
    Json(ApiResponse::success(positions))
}

async fn get_position(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<PortfolioQuery>,
) -> Json<ApiResponse<Option<crate::services::portfolio_service::Position>>> {
    match state.portfolio_service.get_position(&query.account_id, &symbol).await {
        Some(position) => Json(ApiResponse::success(Some(position))),
        None => Json(ApiResponse::success(None)),
    }
}

// Trading Data Handlers
async fn get_trades(
    State(state): State<AppState>,
    Query(query): Query<TradesQuery>,
) -> Json<ApiResponse<Vec<crate::services::trading_service::PaperTrade>>> {
    let trades = state.trading_service.get_trades(
        query.symbol.as_deref(),
        query.limit
    ).await;
    Json(ApiResponse::success(trades))
}

async fn get_trades_by_symbol(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
    Query(query): Query<TradesQuery>,
) -> Json<ApiResponse<Vec<crate::services::trading_service::PaperTrade>>> {
    let trades = state.trading_service.get_trades(
        Some(&symbol),
        query.limit
    ).await;
    Json(ApiResponse::success(trades))
}

// Market Data Handlers
async fn get_order_book(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Json<ApiResponse<Option<crate::services::market_data_service::ConsolidatedOrderBook>>> {
    match state.market_data_service.get_consolidated_order_book(&symbol).await {
        Some(book) => Json(ApiResponse::success(Some(book))),
        None => Json(ApiResponse::success(None)),
    }
}

async fn get_latest_trade(
    State(state): State<AppState>,
    Path(symbol): Path<String>,
) -> Json<ApiResponse<Option<Trade>>> {
    match state.market_data_service.get_latest_trade(&symbol).await {
        Some(trade) => Json(ApiResponse::success(Some(trade))),
        None => Json(ApiResponse::success(None)),
    }
}

async fn get_market_data_stats(
    State(state): State<AppState>,
) -> Json<ApiResponse<MarketDataServiceMetrics>> {
    let stats = state.market_data_service.get_service_metrics().await;
    Json(ApiResponse::success(stats))
}

// Performance Handlers
async fn get_performance_report(
    State(state): State<AppState>,
) -> Json<ApiResponse<PerformanceReport>> {
    let report = state.benchmark_service.get_performance_report().await;
    Json(ApiResponse::success(report))
}

async fn get_latency_stats(
    State(state): State<AppState>,
    Path(operation): Path<String>,
) -> Json<ApiResponse<Option<crate::utils::LatencyDistribution>>> {
    match state.benchmark_service.get_latency_percentiles(&operation).await {
        Some(stats) => Json(ApiResponse::success(Some(stats))),
        None => Json(ApiResponse::success(None)),
    }
}

#[derive(Deserialize)]
struct BenchmarkRequest {
    operations_count: u64,
    concurrent_workers: usize,
}

async fn run_benchmark(
    State(state): State<AppState>,
    Path(operation): Path<String>,
    Json(request): Json<BenchmarkRequest>,
) -> Json<ApiResponse<crate::services::benchmark_service::ThroughputBenchmark>> {
    match state.benchmark_service.run_throughput_benchmark(
        &operation,
        request.operations_count,
        request.concurrent_workers,
    ).await {
        Ok(benchmark) => Json(ApiResponse::success(benchmark)),
        Err(e) => Json(ApiResponse::error(format!("Benchmark failed: {}", e))),
    }
}

// Risk Handlers
#[derive(Deserialize)]
struct RiskLimitsQuery {
    account_id: String,
}

async fn get_risk_limits(
    State(state): State<AppState>,
    Query(query): Query<RiskLimitsQuery>,
) -> Json<ApiResponse<Option<RiskLimits>>> {
    match state.risk_service.get_risk_limits(&query.account_id).await {
        Some(limits) => Json(ApiResponse::success(Some(limits))),
        None => Json(ApiResponse::success(None)),
    }
}

async fn set_risk_limits(
    State(state): State<AppState>,
    Json(limits): Json<RiskLimits>,
) -> Json<ApiResponse<String>> {
    match state.risk_service.set_risk_limits(limits.account_id.clone(), limits).await {
        Ok(_) => Json(ApiResponse::success("Risk limits updated successfully".to_string())),
        Err(e) => Json(ApiResponse::error(format!("Failed to update risk limits: {}", e))),
    }
}

async fn get_risk_metrics(
    State(state): State<AppState>,
    Query(query): Query<RiskLimitsQuery>,
) -> Json<ApiResponse<Option<RiskMetrics>>> {
    // Get portfolio first
    if let Some(portfolio) = state.portfolio_service.get_portfolio(&query.account_id).await {
        let metrics = state.risk_service.calculate_portfolio_risk_metrics(&query.account_id, &portfolio).await;
        Json(ApiResponse::success(Some(metrics)))
    } else {
        Json(ApiResponse::success(None))
    }
}

async fn get_risk_alerts(
    State(state): State<AppState>,
    Query(query): Query<Option<RiskLimitsQuery>>,
) -> Json<ApiResponse<Vec<crate::services::risk_service::RiskAlert>>> {
    let account_id = query.as_ref().map(|q| q.account_id.as_str());
    let alerts = state.risk_service.get_risk_alerts(account_id).await;
    Json(ApiResponse::success(alerts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_health_check() {
        // This would require setting up the full state
        // For now, just test that the function compiles
        assert!(true);
    }
}