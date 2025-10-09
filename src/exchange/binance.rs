use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::exchange::*;
use crate::types::*;
use crate::utils::*;
use crate::Result;

pub struct BinanceConnector {
    config: ConnectionConfig,
    websocket: Arc<WebSocketConnection>,
    rest_client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
    subscribed_symbols: Arc<parking_lot::RwLock<std::collections::HashSet<Symbol>>>,
    is_running: Arc<AtomicBool>,
    message_tx: Option<crossbeam_channel::Sender<MarketEvent>>,
}

impl BinanceConnector {
    pub fn new(testnet: bool) -> Self {
        let config = if testnet {
            ConnectionConfig {
                exchange_name: "binance_testnet".to_string(),
                websocket_url: "wss://testnet.binance.vision/ws".to_string(),
                rest_api_url: "https://testnet.binance.vision".to_string(),
                testnet: true,
                rate_limit_requests_per_second: 20, // Binance testnet allows higher rates
                max_reconnect_attempts: 10,
                reconnect_delay_ms: 2000,
                ..Default::default()
            }
        } else {
            ConnectionConfig {
                exchange_name: "binance".to_string(),
                websocket_url: "wss://stream.binance.com:9443/ws".to_string(),
                rest_api_url: "https://api.binance.com".to_string(),
                testnet: false,
                rate_limit_requests_per_second: 10,
                max_reconnect_attempts: 5,
                reconnect_delay_ms: 5000,
                ..Default::default()
            }
        };

        let websocket = Arc::new(WebSocketConnection::new(config.websocket_url.clone()));
        let rest_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit_requests_per_second));

        Self {
            config,
            websocket,
            rest_client,
            rate_limiter,
            subscribed_symbols: Arc::new(parking_lot::RwLock::new(std::collections::HashSet::new())),
            is_running: Arc::new(AtomicBool::new(false)),
            message_tx: None,
        }
    }

    pub fn with_credentials(mut self, api_key: String, secret_key: String) -> Self {
        self.config.api_key = Some(api_key);
        self.config.secret_key = Some(secret_key);
        self
    }

    pub fn with_message_sender(mut self, sender: crossbeam_channel::Sender<MarketEvent>) -> Self {
        self.message_tx = Some(sender);
        self
    }

    pub async fn start_market_data_stream(&self) -> Result<()> {
        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Binance connector is already running".to_string()));
        }

        let websocket = Arc::clone(&self.websocket);
        let is_running = Arc::clone(&self.is_running);
        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            Self::market_data_loop(websocket, is_running, message_tx).await;
        });

        Ok(())
    }

    async fn market_data_loop(
        websocket: Arc<WebSocketConnection>,
        is_running: Arc<AtomicBool>,
        message_tx: Option<crossbeam_channel::Sender<MarketEvent>>,
    ) {
        while is_running.load(Ordering::Relaxed) {
            match connect_async(&websocket.url).await {
                Ok((ws_stream, _)) => {
                    tracing::info!("Connected to Binance WebSocket stream");
                    websocket.is_connected.store(true, Ordering::Relaxed);

                    let (mut write, mut read) = ws_stream.split();

                    // Send ping periodically to keep connection alive
                    let ping_websocket = Arc::clone(&websocket);
                    let ping_is_running = Arc::clone(&is_running);
                    tokio::spawn(async move {
                        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                        while ping_is_running.load(Ordering::Relaxed) {
                            interval.tick().await;
                            if let Err(e) = write.send(Message::Ping(vec![])).await {
                                tracing::warn!("Failed to send ping: {}", e);
                                break;
                            }
                        }
                    });

                    // Process incoming messages
                    while let Some(msg) = read.next().await {
                        if !is_running.load(Ordering::Relaxed) {
                            break;
                        }

                        match msg {
                            Ok(Message::Text(text)) => {
                                let processing_start = std::time::Instant::now();

                                if let Some(market_event) = Self::parse_binance_message(&text) {
                                    if let Some(ref tx) = message_tx {
                                        if let Err(e) = tx.try_send(market_event) {
                                            tracing::warn!("Failed to send market event: {}", e);
                                        }
                                    }
                                }

                                let processing_time = processing_start.elapsed().as_nanos() as u64;
                                websocket.latency_tracker.record_latency(processing_time);
                                websocket.metrics.increment_market_data_events();
                            }
                            Ok(Message::Pong(_)) => {
                                websocket.last_heartbeat.store(
                                    std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_nanos() as u64,
                                    Ordering::Relaxed,
                                );
                            }
                            Ok(Message::Close(_)) => {
                                tracing::info!("WebSocket connection closed by server");
                                break;
                            }
                            Ok(_) => {
                                // Handle other message types if needed
                            }
                            Err(e) => {
                                tracing::error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to connect to Binance WebSocket: {}", e);
                    websocket.is_connected.store(false, Ordering::Relaxed);
                    websocket.reconnect_attempts.fetch_add(1, Ordering::Relaxed);

                    // Wait before reconnecting
                    tokio::time::sleep(std::time::Duration::from_millis(5000)).await;
                }
            }
        }

        websocket.is_connected.store(false, Ordering::Relaxed);
        tracing::info!("Binance market data stream stopped");
    }

    fn parse_binance_message(message: &str) -> Option<MarketEvent> {
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(message) {
            // Handle different types of Binance WebSocket messages
            if let Some(event_type) = json_value.get("e").and_then(|v| v.as_str()) {
                match event_type {
                    "trade" => Self::parse_trade_event(&json_value),
                    "depthUpdate" => Self::parse_depth_update(&json_value),
                    "24hrTicker" => Self::parse_ticker_event(&json_value),
                    _ => {
                        tracing::trace!("Unknown Binance event type: {}", event_type);
                        None
                    }
                }
            } else {
                tracing::trace!("Message without event type: {}", message);
                None
            }
        } else {
            tracing::warn!("Failed to parse Binance message: {}", message);
            None
        }
    }

    fn parse_trade_event(json: &serde_json::Value) -> Option<MarketEvent> {
        let symbol = json.get("s")?.as_str()?.to_string();
        let price_str = json.get("p")?.as_str()?;
        let quantity_str = json.get("q")?.as_str()?;
        let timestamp_ms = json.get("T")?.as_u64()?;
        let is_buyer_maker = json.get("m")?.as_bool()?;

        let price = rust_decimal::Decimal::from_str_exact(price_str).ok()?;
        let quantity = rust_decimal::Decimal::from_str_exact(quantity_str).ok()?;

        let timestamp = chrono::DateTime::from_timestamp_millis(timestamp_ms as i64)?;

        Some(MarketEvent::Trade(Trade {
            symbol: Symbol::new(symbol),
            exchange: "binance".to_string(),
            price,
            quantity,
            timestamp,
            is_buyer_maker,
            sequence: 0, // Binance doesn't provide sequence numbers in trade events
        }))
    }

    fn parse_depth_update(json: &serde_json::Value) -> Option<MarketEvent> {
        let symbol = json.get("s")?.as_str()?.to_string();
        let bids_array = json.get("b")?.as_array()?;
        let asks_array = json.get("a")?.as_array()?;

        let mut bids = Vec::new();
        for bid in bids_array.iter().take(10) { // Top 10 levels
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price_str = bid_array[0].as_str()?;
                    let quantity_str = bid_array[1].as_str()?;

                    let price = rust_decimal::Decimal::from_str_exact(price_str).ok()?;
                    let quantity = rust_decimal::Decimal::from_str_exact(quantity_str).ok()?;

                    bids.push(BookLevel {
                        price,
                        quantity,
                        order_count: 0, // Binance doesn't provide order count
                    });
                }
            }
        }

        let mut asks = Vec::new();
        for ask in asks_array.iter().take(10) { // Top 10 levels
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price_str = ask_array[0].as_str()?;
                    let quantity_str = ask_array[1].as_str()?;

                    let price = rust_decimal::Decimal::from_str_exact(price_str).ok()?;
                    let quantity = rust_decimal::Decimal::from_str_exact(quantity_str).ok()?;

                    asks.push(BookLevel {
                        price,
                        quantity,
                        order_count: 0,
                    });
                }
            }
        }

        Some(MarketEvent::BookSnapshot(OrderBookSnapshot {
            symbol: Symbol::new(symbol),
            exchange: "binance".to_string(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: json.get("u")?.as_u64().unwrap_or(0),
            last_update_id: json.get("U")?.as_u64().unwrap_or(0),
        }))
    }

    fn parse_ticker_event(json: &serde_json::Value) -> Option<MarketEvent> {
        let symbol = json.get("s")?.as_str()?.to_string();
        let open_price_str = json.get("o")?.as_str()?;
        let high_price_str = json.get("h")?.as_str()?;
        let low_price_str = json.get("l")?.as_str()?;
        let close_price_str = json.get("c")?.as_str()?;
        let volume_str = json.get("v")?.as_str()?;
        let quote_volume_str = json.get("q")?.as_str()?;
        let count = json.get("n")?.as_u64()?;

        let open_price = rust_decimal::Decimal::from_str_exact(open_price_str).ok()?;
        let high_price = rust_decimal::Decimal::from_str_exact(high_price_str).ok()?;
        let low_price = rust_decimal::Decimal::from_str_exact(low_price_str).ok()?;
        let close_price = rust_decimal::Decimal::from_str_exact(close_price_str).ok()?;
        let volume = rust_decimal::Decimal::from_str_exact(volume_str).ok()?;
        let quote_volume = rust_decimal::Decimal::from_str_exact(quote_volume_str).ok()?;

        Some(MarketEvent::Stats(MarketStats {
            symbol: Symbol::new(symbol),
            open_price,
            high_price,
            low_price,
            close_price,
            volume,
            quote_volume,
            timestamp: chrono::Utc::now(),
            count,
        }))
    }

    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);
        self.websocket.disconnect().await?;
        tracing::info!("Binance connector stopped");
        Ok(())
    }
}

#[async_trait]
impl ExchangeConnector for BinanceConnector {
    async fn connect(&self) -> Result<()> {
        self.websocket.connect().await
    }

    async fn disconnect(&self) -> Result<()> {
        self.websocket.disconnect().await
    }

    async fn subscribe_market_data(&self, symbols: Vec<Symbol>) -> Result<()> {
        let mut subscribed = self.subscribed_symbols.write();

        for symbol in symbols {
            subscribed.insert(symbol.clone());
            tracing::info!("Subscribed to Binance market data for {}", symbol);
        }

        Ok(())
    }

    async fn unsubscribe_market_data(&self, symbols: Vec<Symbol>) -> Result<()> {
        let mut subscribed = self.subscribed_symbols.write();

        for symbol in symbols {
            subscribed.remove(&symbol);
            tracing::info!("Unsubscribed from Binance market data for {}", symbol);
        }

        Ok(())
    }

    async fn get_order_book(&self, symbol: &Symbol) -> Result<OrderBookSnapshot> {
        self.rate_limiter.wait_if_needed().await?;

        let url = format!("{}/api/v3/depth?symbol={}&limit=100", self.config.rest_api_url, symbol.0);

        let response = self.rest_client
            .get(&url)
            .send()
            .await
            .map_err(|e| crate::EngineError::Network(e))?;

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| crate::EngineError::Serialization(serde_json::Error::custom(e.to_string())))?;

        // Parse the order book response
        let bids_array = json.get("bids")
            .and_then(|v| v.as_array())
            .ok_or_else(|| crate::EngineError::Internal("Invalid order book format".to_string()))?;

        let asks_array = json.get("asks")
            .and_then(|v| v.as_array())
            .ok_or_else(|| crate::EngineError::Internal("Invalid order book format".to_string()))?;

        let mut bids = Vec::new();
        for bid in bids_array {
            if let Some(bid_array) = bid.as_array() {
                if bid_array.len() >= 2 {
                    let price_str = bid_array[0].as_str().unwrap_or("0");
                    let quantity_str = bid_array[1].as_str().unwrap_or("0");

                    let price = rust_decimal::Decimal::from_str_exact(price_str)
                        .unwrap_or(rust_decimal::Decimal::ZERO);
                    let quantity = rust_decimal::Decimal::from_str_exact(quantity_str)
                        .unwrap_or(rust_decimal::Decimal::ZERO);

                    bids.push(BookLevel {
                        price,
                        quantity,
                        order_count: 0,
                    });
                }
            }
        }

        let mut asks = Vec::new();
        for ask in asks_array {
            if let Some(ask_array) = ask.as_array() {
                if ask_array.len() >= 2 {
                    let price_str = ask_array[0].as_str().unwrap_or("0");
                    let quantity_str = ask_array[1].as_str().unwrap_or("0");

                    let price = rust_decimal::Decimal::from_str_exact(price_str)
                        .unwrap_or(rust_decimal::Decimal::ZERO);
                    let quantity = rust_decimal::Decimal::from_str_exact(quantity_str)
                        .unwrap_or(rust_decimal::Decimal::ZERO);

                    asks.push(BookLevel {
                        price,
                        quantity,
                        order_count: 0,
                    });
                }
            }
        }

        Ok(OrderBookSnapshot {
            symbol: symbol.clone(),
            exchange: "binance".to_string(),
            bids,
            asks,
            timestamp: chrono::Utc::now(),
            sequence: json.get("lastUpdateId").and_then(|v| v.as_u64()).unwrap_or(0),
            last_update_id: json.get("lastUpdateId").and_then(|v| v.as_u64()).unwrap_or(0),
        })
    }

    async fn place_order(&self, _order: &Order) -> Result<ExecutionReport> {
        // This would require authentication and proper order placement
        // For demo purposes, return a placeholder
        Err(crate::EngineError::Internal("Order placement not implemented in demo".to_string()))
    }

    async fn cancel_order(&self, _order_id: &OrderId) -> Result<ExecutionReport> {
        // This would require authentication and proper order cancellation
        // For demo purposes, return a placeholder
        Err(crate::EngineError::Internal("Order cancellation not implemented in demo".to_string()))
    }

    async fn get_account_info(&self) -> Result<serde_json::Value> {
        // This would require authentication
        // For demo purposes, return a placeholder
        Ok(json!({
            "exchange": "binance",
            "status": "demo_mode",
            "message": "Account info requires authentication"
        }))
    }

    fn get_name(&self) -> &str {
        &self.config.exchange_name
    }

    fn is_connected(&self) -> bool {
        self.websocket.is_connected()
    }

    fn get_supported_symbols(&self) -> Vec<Symbol> {
        vec![
            Symbol::new("BTCUSDT".to_string()),
            Symbol::new("ETHUSDT".to_string()),
            Symbol::new("SOLUSDT".to_string()),
            Symbol::new("ADAUSDT".to_string()),
            Symbol::new("DOTUSDT".to_string()),
            Symbol::new("LINKUSDT".to_string()),
        ]
    }

    fn get_latency_stats(&self) -> LatencyDistribution {
        self.websocket.get_latency_distribution()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_connector_creation() {
        let connector = BinanceConnector::new(true);
        assert_eq!(connector.get_name(), "binance_testnet");
        assert!(!connector.is_connected());

        let supported_symbols = connector.get_supported_symbols();
        assert!(!supported_symbols.is_empty());
        assert!(supported_symbols.contains(&Symbol::new("BTCUSDT".to_string())));
    }

    #[test]
    fn test_parse_trade_message() {
        let message = r#"{
            "e": "trade",
            "E": 123456789,
            "s": "BTCUSDT",
            "t": 12345,
            "p": "50000.00",
            "q": "0.001",
            "b": 88,
            "a": 50,
            "T": 1642691234567,
            "m": true,
            "M": true
        }"#;

        let market_event = BinanceConnector::parse_binance_message(message);
        assert!(market_event.is_some());

        if let Some(MarketEvent::Trade(trade)) = market_event {
            assert_eq!(trade.symbol.0, "BTCUSDT");
            assert_eq!(trade.price, rust_decimal::Decimal::from_str_exact("50000.00").unwrap());
            assert_eq!(trade.quantity, rust_decimal::Decimal::from_str_exact("0.001").unwrap());
            assert!(trade.is_buyer_maker);
        } else {
            panic!("Expected trade event");
        }
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let connector = BinanceConnector::new(true);

        // Test that rate limiter waits appropriately
        let start = std::time::Instant::now();

        // This should complete quickly since we're within limits
        connector.rate_limiter.wait_if_needed().await.unwrap();

        let duration = start.elapsed();
        assert!(duration < std::time::Duration::from_millis(100));
    }
}