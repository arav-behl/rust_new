use async_trait::async_trait;
use std::sync::Arc;

use crate::exchange::*;
use crate::types::*;
use crate::Result;

pub struct CoinbaseConnector {
    config: ConnectionConfig,
    websocket: Arc<WebSocketConnection>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl CoinbaseConnector {
    pub fn new(sandbox: bool) -> Self {
        let config = if sandbox {
            ConnectionConfig {
                exchange_name: "coinbase_sandbox".to_string(),
                websocket_url: "wss://ws-feed-public.sandbox.exchange.coinbase.com".to_string(),
                rest_api_url: "https://api-public.sandbox.exchange.coinbase.com".to_string(),
                testnet: true,
                rate_limit_requests_per_second: 15,
                max_reconnect_attempts: 5,
                reconnect_delay_ms: 3000,
                ..Default::default()
            }
        } else {
            ConnectionConfig {
                exchange_name: "coinbase".to_string(),
                websocket_url: "wss://ws-feed.exchange.coinbase.com".to_string(),
                rest_api_url: "https://api.exchange.coinbase.com".to_string(),
                testnet: false,
                rate_limit_requests_per_second: 10,
                max_reconnect_attempts: 3,
                reconnect_delay_ms: 5000,
                ..Default::default()
            }
        };

        let websocket = Arc::new(WebSocketConnection::new(config.websocket_url.clone()));

        Self {
            config,
            websocket,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl ExchangeConnector for CoinbaseConnector {
    async fn connect(&self) -> Result<()> {
        self.websocket.connect().await
    }

    async fn disconnect(&self) -> Result<()> {
        self.websocket.disconnect().await
    }

    async fn subscribe_market_data(&self, _symbols: Vec<Symbol>) -> Result<()> {
        // Coinbase implementation would go here
        Ok(())
    }

    async fn unsubscribe_market_data(&self, _symbols: Vec<Symbol>) -> Result<()> {
        // Coinbase implementation would go here
        Ok(())
    }

    async fn get_order_book(&self, _symbol: &Symbol) -> Result<OrderBookSnapshot> {
        // Placeholder implementation
        Err(crate::EngineError::Internal("Coinbase order book not implemented in demo".to_string()))
    }

    async fn place_order(&self, _order: &Order) -> Result<ExecutionReport> {
        Err(crate::EngineError::Internal("Order placement not implemented in demo".to_string()))
    }

    async fn cancel_order(&self, _order_id: &OrderId) -> Result<ExecutionReport> {
        Err(crate::EngineError::Internal("Order cancellation not implemented in demo".to_string()))
    }

    async fn get_account_info(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "exchange": "coinbase",
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
            Symbol::new("BTC-USD".to_string()),
            Symbol::new("ETH-USD".to_string()),
            Symbol::new("ADA-USD".to_string()),
        ]
    }

    fn get_latency_stats(&self) -> LatencyDistribution {
        self.websocket.get_latency_distribution()
    }
}