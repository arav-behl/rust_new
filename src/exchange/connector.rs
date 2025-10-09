use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use futures_util::{SinkExt, StreamExt};

use crate::types::*;
use crate::utils::*;
use crate::Result;

#[async_trait]
pub trait ExchangeConnector: Send + Sync {
    async fn connect(&self) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;
    async fn subscribe_market_data(&self, symbols: Vec<Symbol>) -> Result<()>;
    async fn unsubscribe_market_data(&self, symbols: Vec<Symbol>) -> Result<()>;
    async fn get_order_book(&self, symbol: &Symbol) -> Result<OrderBookSnapshot>;
    async fn place_order(&self, order: &Order) -> Result<ExecutionReport>;
    async fn cancel_order(&self, order_id: &OrderId) -> Result<ExecutionReport>;
    async fn get_account_info(&self) -> Result<serde_json::Value>;

    fn get_name(&self) -> &str;
    fn is_connected(&self) -> bool;
    fn get_supported_symbols(&self) -> Vec<Symbol>;
    fn get_latency_stats(&self) -> LatencyDistribution;
}

pub struct ConnectionConfig {
    pub exchange_name: String,
    pub websocket_url: String,
    pub rest_api_url: String,
    pub api_key: Option<String>,
    pub secret_key: Option<String>,
    pub testnet: bool,
    pub rate_limit_requests_per_second: u32,
    pub max_reconnect_attempts: u32,
    pub reconnect_delay_ms: u64,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            exchange_name: "unknown".to_string(),
            websocket_url: String::new(),
            rest_api_url: String::new(),
            api_key: None,
            secret_key: None,
            testnet: true,
            rate_limit_requests_per_second: 10,
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 1000,
        }
    }
}

pub struct WebSocketConnection {
    pub url: String,
    pub is_connected: Arc<std::sync::atomic::AtomicBool>,
    pub reconnect_attempts: Arc<std::sync::atomic::AtomicU32>,
    pub last_heartbeat: Arc<std::sync::atomic::AtomicU64>,
    pub metrics: Arc<AtomicMetrics>,
    pub latency_tracker: SharedLatencyTracker,
}

impl WebSocketConnection {
    pub fn new(url: String) -> Self {
        Self {
            url,
            is_connected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            reconnect_attempts: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            last_heartbeat: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            metrics: Arc::new(AtomicMetrics::new()),
            latency_tracker: SharedLatencyTracker::new(10000),
        }
    }

    pub async fn connect(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        tracing::info!("Connecting to WebSocket: {}", self.url);

        let connect_start = std::time::Instant::now();

        match connect_async(&self.url).await {
            Ok((ws_stream, response)) => {
                let connect_latency = connect_start.elapsed().as_nanos() as u64;
                self.latency_tracker.record_latency(connect_latency);

                tracing::info!(
                    "WebSocket connected successfully. Status: {}, Latency: {} ns",
                    response.status(),
                    connect_latency
                );

                self.is_connected.store(true, Ordering::Relaxed);
                self.reconnect_attempts.store(0, Ordering::Relaxed);
                self.last_heartbeat.store(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64,
                    Ordering::Relaxed,
                );

                self.metrics.increment_websocket_reconnects();

                Ok(())
            }
            Err(e) => {
                self.is_connected.store(false, Ordering::Relaxed);
                self.reconnect_attempts.fetch_add(1, Ordering::Relaxed);

                tracing::error!("Failed to connect to WebSocket {}: {}", self.url, e);
                Err(crate::EngineError::WebSocket(format!("Connection failed: {}", e)))
            }
        }
    }

    pub async fn disconnect(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        self.is_connected.store(false, Ordering::Relaxed);
        tracing::info!("WebSocket disconnected: {}", self.url);
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_metrics(&self) -> MetricsSnapshot {
        self.metrics.get_snapshot()
    }

    pub fn get_latency_distribution(&self) -> LatencyDistribution {
        self.latency_tracker.get_distribution()
    }
}

pub struct RateLimiter {
    requests_per_second: u32,
    last_request_times: Arc<parking_lot::Mutex<std::collections::VecDeque<std::time::Instant>>>,
}

impl RateLimiter {
    pub fn new(requests_per_second: u32) -> Self {
        Self {
            requests_per_second,
            last_request_times: Arc::new(parking_lot::Mutex::new(std::collections::VecDeque::new())),
        }
    }

    pub async fn wait_if_needed(&self) -> Result<()> {
        let now = std::time::Instant::now();
        let one_second_ago = now - std::time::Duration::from_secs(1);

        let mut times = self.last_request_times.lock();

        // Remove requests older than 1 second
        while let Some(&front_time) = times.front() {
            if front_time < one_second_ago {
                times.pop_front();
            } else {
                break;
            }
        }

        // Check if we've exceeded the rate limit
        if times.len() >= self.requests_per_second as usize {
            // Calculate how long we need to wait
            if let Some(&oldest_recent) = times.front() {
                let wait_until = oldest_recent + std::time::Duration::from_secs(1);
                if now < wait_until {
                    let wait_duration = wait_until - now;
                    tracing::debug!("Rate limit reached, waiting {:?}", wait_duration);
                    tokio::time::sleep(wait_duration).await;
                }
            }

            // Re-clean the queue after waiting
            let now_after_wait = std::time::Instant::now();
            let one_second_ago_after_wait = now_after_wait - std::time::Duration::from_secs(1);
            times.retain(|&time| time >= one_second_ago_after_wait);
        }

        // Record this request
        times.push_back(now);

        Ok(())
    }

    pub fn get_current_rate(&self) -> f64 {
        let now = std::time::Instant::now();
        let one_second_ago = now - std::time::Duration::from_secs(1);

        let times = self.last_request_times.lock();
        let recent_requests = times.iter().filter(|&&time| time >= one_second_ago).count();

        recent_requests as f64
    }
}

pub struct ExchangeHealth {
    pub is_connected: bool,
    pub last_heartbeat: std::time::SystemTime,
    pub reconnect_attempts: u32,
    pub average_latency_ms: f64,
    pub message_rate: f64,
    pub error_rate: f64,
}

impl ExchangeHealth {
    pub fn is_healthy(&self) -> bool {
        self.is_connected
            && self.average_latency_ms < 1000.0  // Less than 1 second
            && self.error_rate < 0.05            // Less than 5% error rate
            && self.reconnect_attempts < 10      // Less than 10 reconnection attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_rate_limiter() {
        let rate_limiter = RateLimiter::new(5); // 5 requests per second

        // Make 5 requests quickly - should not be rate limited
        let start = std::time::Instant::now();
        for _ in 0..5 {
            rate_limiter.wait_if_needed().await.unwrap();
        }
        let duration = start.elapsed();

        // Should complete quickly since we're within the rate limit
        assert!(duration < Duration::from_millis(100));

        // The 6th request should be rate limited
        let start_6th = std::time::Instant::now();
        rate_limiter.wait_if_needed().await.unwrap();
        let duration_6th = start_6th.elapsed();

        // Should take close to 1 second due to rate limiting
        assert!(duration_6th > Duration::from_millis(900));
    }

    #[test]
    fn test_connection_config() {
        let config = ConnectionConfig::default();
        assert_eq!(config.exchange_name, "unknown");
        assert!(config.testnet);
        assert_eq!(config.rate_limit_requests_per_second, 10);
    }

    #[test]
    fn test_websocket_connection_creation() {
        let conn = WebSocketConnection::new("wss://example.com".to_string());
        assert!(!conn.is_connected());
        assert_eq!(conn.url, "wss://example.com");
    }

    #[test]
    fn test_exchange_health() {
        let health = ExchangeHealth {
            is_connected: true,
            last_heartbeat: std::time::SystemTime::now(),
            reconnect_attempts: 0,
            average_latency_ms: 50.0,
            message_rate: 100.0,
            error_rate: 0.01,
        };

        assert!(health.is_healthy());

        let unhealthy = ExchangeHealth {
            is_connected: false,
            last_heartbeat: std::time::SystemTime::now(),
            reconnect_attempts: 15,
            average_latency_ms: 5000.0,
            message_rate: 1.0,
            error_rate: 0.10,
        };

        assert!(!unhealthy.is_healthy());
    }
}