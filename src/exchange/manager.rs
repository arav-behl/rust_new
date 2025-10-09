use std::collections::HashMap;
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::exchange::*;
use crate::types::*;
use crate::utils::*;
use crate::Result;

pub struct ExchangeManager {
    exchanges: HashMap<String, Box<dyn ExchangeConnector>>,
    market_data_tx: Sender<MarketEvent>,
    market_data_rx: Receiver<MarketEvent>,
    metrics: Arc<AtomicMetrics>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl ExchangeManager {
    pub fn new() -> Self {
        let (tx, rx) = unbounded::<MarketEvent>();

        Self {
            exchanges: HashMap::new(),
            market_data_tx: tx,
            market_data_rx: rx,
            metrics: Arc::new(AtomicMetrics::new()),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn add_binance(&mut self, testnet: bool) -> Result<()> {
        let connector = BinanceConnector::new(testnet)
            .with_message_sender(self.market_data_tx.clone());

        self.exchanges.insert(
            connector.get_name().to_string(),
            Box::new(connector)
        );

        tracing::info!("Added Binance connector (testnet: {})", testnet);
        Ok(())
    }

    pub fn add_coinbase(&mut self, sandbox: bool) -> Result<()> {
        let connector = CoinbaseConnector::new(sandbox);

        self.exchanges.insert(
            connector.get_name().to_string(),
            Box::new(connector)
        );

        tracing::info!("Added Coinbase connector (sandbox: {})", sandbox);
        Ok(())
    }

    pub async fn start_all(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Exchange manager is already running".to_string()));
        }

        tracing::info!("Starting all exchange connections...");

        for (name, exchange) in &self.exchanges {
            match exchange.connect().await {
                Ok(_) => {
                    tracing::info!("Connected to exchange: {}", name);

                    // Subscribe to default symbols
                    let symbols = exchange.get_supported_symbols();
                    exchange.subscribe_market_data(symbols.clone()).await?;
                    tracing::info!("Subscribed to {} symbols on {}", symbols.len(), name);
                }
                Err(e) => {
                    tracing::error!("Failed to connect to exchange {}: {}", name, e);
                }
            }
        }

        tracing::info!("Exchange manager started with {} exchanges", self.exchanges.len());
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        self.is_running.store(false, Ordering::SeqCst);

        tracing::info!("Stopping all exchange connections...");

        for (name, exchange) in &self.exchanges {
            if let Err(e) = exchange.disconnect().await {
                tracing::warn!("Error disconnecting from {}: {}", name, e);
            } else {
                tracing::info!("Disconnected from exchange: {}", name);
            }
        }

        tracing::info!("Exchange manager stopped");
        Ok(())
    }

    pub fn get_market_data_receiver(&self) -> Receiver<MarketEvent> {
        self.market_data_rx.clone()
    }

    pub fn get_exchange_health(&self) -> HashMap<String, ExchangeHealth> {
        let mut health_map = HashMap::new();

        for (name, exchange) in &self.exchanges {
            let latency_dist = exchange.get_latency_stats();

            let health = ExchangeHealth {
                is_connected: exchange.is_connected(),
                last_heartbeat: std::time::SystemTime::now(), // Would track actual heartbeats
                reconnect_attempts: 0, // Would track actual reconnection attempts
                average_latency_ms: latency_dist.mean / 1_000_000.0, // Convert nanos to millis
                message_rate: 100.0, // Placeholder
                error_rate: 0.01,    // Placeholder
            };

            health_map.insert(name.clone(), health);
        }

        health_map
    }

    pub fn get_connected_exchanges(&self) -> Vec<String> {
        self.exchanges
            .iter()
            .filter(|(_, exchange)| exchange.is_connected())
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn get_all_supported_symbols(&self) -> HashMap<String, Vec<Symbol>> {
        self.exchanges
            .iter()
            .map(|(name, exchange)| (name.clone(), exchange.get_supported_symbols()))
            .collect()
    }

    pub fn get_metrics(&self) -> MetricsSnapshot {
        self.metrics.get_snapshot()
    }
}

impl Default for ExchangeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_manager_creation() {
        let manager = ExchangeManager::new();
        assert_eq!(manager.exchanges.len(), 0);
        assert_eq!(manager.get_connected_exchanges().len(), 0);
    }

    #[test]
    fn test_add_exchanges() {
        let mut manager = ExchangeManager::new();

        manager.add_binance(true).unwrap();
        manager.add_coinbase(true).unwrap();

        assert_eq!(manager.exchanges.len(), 2);
        assert!(manager.exchanges.contains_key("binance_testnet"));
        assert!(manager.exchanges.contains_key("coinbase_sandbox"));
    }

    #[tokio::test]
    async fn test_exchange_health() {
        let mut manager = ExchangeManager::new();
        manager.add_binance(true).unwrap();

        let health_map = manager.get_exchange_health();
        assert!(health_map.contains_key("binance_testnet"));

        let binance_health = &health_map["binance_testnet"];
        assert!(!binance_health.is_connected); // Should not be connected yet
    }
}