use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::types::{orders::*, execution::*, Symbol, Price, Quantity, BookLevel, OrderBookSnapshot, MarketEvent, Trade};
use crate::exchange::{BinanceConnector, ExchangeConnector};
use crate::utils::latency::LatencyDistribution;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataSubscription {
    pub subscriber_id: String,
    pub symbols: Vec<String>,
    pub data_types: Vec<MarketDataType>,
    pub exchange: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketDataType {
    Trade,
    OrderBook,
    Ticker,
    Candle,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataStats {
    pub exchange: String,
    pub symbol: String,
    pub messages_received: u64,
    pub last_update: DateTime<Utc>,
    pub avg_latency_ns: u64,
    pub connection_status: ConnectionStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidatedOrderBook {
    pub symbol: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub exchanges: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub spread: Decimal,
    pub mid_price: Decimal,
}

pub struct MarketDataService {
    // Exchange connectors - using concrete type instead of trait object
    connectors: Arc<RwLock<HashMap<String, BinanceConnector>>>,

    // Market data distribution
    market_data_tx: Sender<MarketEvent>,
    subscribers: Arc<RwLock<HashMap<String, MarketDataSubscription>>>,
    subscriber_channels: Arc<RwLock<HashMap<String, Sender<MarketEvent>>>>,

    // Data consolidation
    consolidated_books: Arc<RwLock<HashMap<String, ConsolidatedOrderBook>>>,
    latest_trades: Arc<RwLock<HashMap<String, Trade>>>,

    // Statistics and monitoring
    stats: Arc<RwLock<HashMap<String, MarketDataStats>>>,
    message_count: Arc<AtomicU64>,
    latency_tracker: SharedLatencyTracker,

    // Control
    is_running: Arc<AtomicBool>,
}

impl MarketDataService {
    pub fn new(market_data_tx: Sender<MarketEvent>) -> Self {
        Self {
            connectors: Arc::new(RwLock::new(HashMap::new())),
            market_data_tx,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            subscriber_channels: Arc::new(RwLock::new(HashMap::new())),
            consolidated_books: Arc::new(RwLock::new(HashMap::new())),
            latest_trades: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
            message_count: Arc::new(AtomicU64::new(0)),
            latency_tracker: SharedLatencyTracker::new(10000),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Market data service is already running".to_string()));
        }

        // Add exchange connectors
        self.add_exchange_connector("binance", BinanceConnector::new(false)).await;
        // Add more exchanges here as needed

        // Start all connectors
        for (exchange_name, connector) in self.connectors.read().await.iter() {
            match connector.connect().await {
                Ok(_) => {
                    tracing::info!("Connected to exchange: {}", exchange_name);

                    // Update stats
                    self.update_connection_status(exchange_name, ConnectionStatus::Connected).await;
                }
                Err(e) => {
                    tracing::error!("Failed to connect to {}: {}", exchange_name, e);
                    self.update_connection_status(exchange_name, ConnectionStatus::Error).await;
                }
            }
        }

        tracing::info!("Market data service started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);

        // Disconnect all exchanges
        for (exchange_name, connector) in self.connectors.read().await.iter() {
            if let Err(e) = connector.disconnect().await {
                tracing::warn!("Error disconnecting from {}: {}", exchange_name, e);
            } else {
                tracing::info!("Disconnected from exchange: {}", exchange_name);
            }
            self.update_connection_status(exchange_name, ConnectionStatus::Disconnected).await;
        }

        tracing::info!("Market data service stopped");
        Ok(())
    }

    async fn add_exchange_connector(
        &self,
        name: &str,
        connector: BinanceConnector,
    ) {
        self.connectors.write().await.insert(name.to_string(), connector);

        // Initialize stats
        let stats = MarketDataStats {
            exchange: name.to_string(),
            symbol: "ALL".to_string(),
            messages_received: 0,
            last_update: Utc::now(),
            avg_latency_ns: 0,
            connection_status: ConnectionStatus::Disconnected,
        };

        self.stats.write().await.insert(name.to_string(), stats);
    }

    pub async fn subscribe_market_data(
        &self,
        subscriber_id: String,
        symbols: Vec<String>,
        data_types: Vec<MarketDataType>,
        exchange: String,
    ) -> Result<Receiver<MarketEvent>> {
        // Create subscription
        let subscription = MarketDataSubscription {
            subscriber_id: subscriber_id.clone(),
            symbols: symbols.clone(),
            data_types,
            exchange: exchange.clone(),
            created_at: Utc::now(),
        };

        // Store subscription
        self.subscribers.write().await.insert(subscriber_id.clone(), subscription);

        // Create channel for this subscriber
        let (tx, rx) = crossbeam_channel::unbounded();
        self.subscriber_channels.write().await.insert(subscriber_id.clone(), tx);

        // Subscribe to exchange feeds
        if let Some(connector) = self.connectors.read().await.get(&exchange) {
            let symbols_vec: Vec<Symbol> = symbols.iter().map(|s| Symbol::new(s.clone())).collect();
            connector.subscribe_market_data(symbols_vec).await?;

            tracing::info!(
                "Subscribed {} to {} symbols on {}",
                subscriber_id,
                symbols.len(),
                exchange
            );
        } else {
            return Err(crate::EngineError::Internal(
                format!("Exchange {} not found", exchange)
            ));
        }

        Ok(rx)
    }

    pub async fn unsubscribe_market_data(&self, subscriber_id: &str) -> Result<()> {
        // Remove subscription and channel
        let subscription = self.subscribers.write().await.remove(subscriber_id);
        self.subscriber_channels.write().await.remove(subscriber_id);

        if let Some(sub) = subscription {
            // Optionally unsubscribe from exchange if no other subscribers
            tracing::info!("Unsubscribed {} from market data", subscriber_id);
        }

        Ok(())
    }

    pub async fn process_market_event(&self, event: MarketEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Update statistics
        self.message_count.fetch_add(1, Ordering::Relaxed);

        // Update data structures based on event type
        match &event {
            MarketEvent::Trade(trade) => {
                self.latest_trades.write().await.insert(trade.symbol.0.clone(), trade.clone());
                self.update_exchange_stats(&trade.exchange, &trade.symbol.0).await;
            }
            MarketEvent::BookSnapshot(snapshot) => {
                self.update_consolidated_order_book(snapshot).await;
                self.update_exchange_stats(&snapshot.exchange, &snapshot.symbol.0).await;
            }
            MarketEvent::Tick(tick) => {
                self.update_exchange_stats(&tick.exchange, &tick.symbol.0).await;
            }
            MarketEvent::Stats(stats) => {
                self.update_exchange_stats("binance", &stats.symbol.0).await; // Assuming binance for stats
            }
        }

        // Distribute to main market data channel
        if let Err(e) = self.market_data_tx.try_send(event.clone()) {
            tracing::warn!("Failed to send market event to main channel: {}", e);
        }

        // Distribute to subscribers
        self.distribute_to_subscribers(&event).await;

        // Record processing latency
        let processing_time = start_time.elapsed().as_nanos() as u64;
        self.latency_tracker.record_latency(processing_time);

        Ok(())
    }

    async fn update_consolidated_order_book(&self, snapshot: &OrderBookSnapshot) {
        let symbol = &snapshot.symbol.0;

        let mid_price = if let (Some(best_bid), Some(best_ask)) = (
            snapshot.bids.first(),
            snapshot.asks.first(),
        ) {
            (best_bid.price + best_ask.price) / Decimal::TWO
        } else {
            Decimal::ZERO
        };

        let spread = if let (Some(best_bid), Some(best_ask)) = (
            snapshot.bids.first(),
            snapshot.asks.first(),
        ) {
            best_ask.price - best_bid.price
        } else {
            Decimal::ZERO
        };

        let consolidated_book = ConsolidatedOrderBook {
            symbol: symbol.clone(),
            bids: snapshot.bids.clone(),
            asks: snapshot.asks.clone(),
            exchanges: vec![snapshot.exchange.clone()],
            timestamp: snapshot.timestamp,
            spread,
            mid_price,
        };

        self.consolidated_books.write().await.insert(symbol.clone(), consolidated_book);
    }

    async fn distribute_to_subscribers(&self, event: &MarketEvent) {
        let subscribers = self.subscribers.read().await;
        let channels = self.subscriber_channels.read().await;

        for (subscriber_id, subscription) in subscribers.iter() {
            // Check if subscriber is interested in this event
            if self.should_forward_event(subscription, event) {
                if let Some(channel) = channels.get(subscriber_id) {
                    if let Err(_) = channel.try_send(event.clone()) {
                        tracing::debug!("Subscriber {} channel is full or closed", subscriber_id);
                    }
                }
            }
        }
    }

    fn should_forward_event(&self, subscription: &MarketDataSubscription, event: &MarketEvent) -> bool {
        // Check symbol filter
        let symbol = match event {
            MarketEvent::Trade(trade) => &trade.symbol.0,
            MarketEvent::BookSnapshot(snapshot) => &snapshot.symbol.0,
            MarketEvent::Tick(tick) => &tick.symbol.0,
            MarketEvent::Stats(stats) => &stats.symbol.0,
        };

        if !subscription.symbols.is_empty() && !subscription.symbols.contains(symbol) {
            return false;
        }

        // Check data type filter
        let event_type = match event {
            MarketEvent::Trade(_) => MarketDataType::Trade,
            MarketEvent::BookSnapshot(_) => MarketDataType::OrderBook,
            MarketEvent::Tick(_) => MarketDataType::Trade, // Ticks are similar to trades
            MarketEvent::Stats(_) => MarketDataType::Ticker,
        };

        subscription.data_types.contains(&MarketDataType::All) ||
        subscription.data_types.contains(&event_type)
    }

    async fn update_exchange_stats(&self, exchange: &str, symbol: &str) {
        let mut stats = self.stats.write().await;
        let key = format!("{}_{}", exchange, symbol);

        let stat_entry = stats.entry(key).or_insert_with(|| MarketDataStats {
            exchange: exchange.to_string(),
            symbol: symbol.to_string(),
            messages_received: 0,
            last_update: Utc::now(),
            avg_latency_ns: 0,
            connection_status: ConnectionStatus::Connected,
        });

        stat_entry.messages_received += 1;
        stat_entry.last_update = Utc::now();
        stat_entry.avg_latency_ns = self.latency_tracker.get_distribution().mean as u64;
    }

    async fn update_connection_status(&self, exchange: &str, status: ConnectionStatus) {
        let mut stats = self.stats.write().await;
        if let Some(stat) = stats.get_mut(exchange) {
            stat.connection_status = status;
        }
    }

    pub async fn get_latest_trade(&self, symbol: &str) -> Option<Trade> {
        self.latest_trades.read().await.get(symbol).cloned()
    }

    pub async fn get_consolidated_order_book(&self, symbol: &str) -> Option<ConsolidatedOrderBook> {
        self.consolidated_books.read().await.get(symbol).cloned()
    }

    pub async fn get_market_data_stats(&self) -> HashMap<String, MarketDataStats> {
        self.stats.read().await.clone()
    }

    pub async fn get_service_metrics(&self) -> MarketDataServiceMetrics {
        MarketDataServiceMetrics {
            total_messages_processed: self.message_count.load(Ordering::Relaxed),
            active_subscribers: self.subscribers.read().await.len(),
            connected_exchanges: self.get_connected_exchange_count().await,
            avg_processing_latency_ns: self.latency_tracker.get_distribution().mean as u64,
            uptime_seconds: 0, // Would need start time tracking
            timestamp: Utc::now(),
        }
    }

    async fn get_connected_exchange_count(&self) -> usize {
        let stats = self.stats.read().await;
        stats.values()
            .filter(|s| s.connection_status == ConnectionStatus::Connected)
            .count()
    }

    pub async fn health_check(&self) -> MarketDataHealthCheck {
        let connected_exchanges = self.get_connected_exchange_count().await;
        let total_exchanges = self.connectors.read().await.len();

        let is_healthy = connected_exchanges > 0 &&
                        self.message_count.load(Ordering::Relaxed) > 0;

        MarketDataHealthCheck {
            is_healthy,
            connected_exchanges,
            total_exchanges,
            last_message_time: Utc::now(), // Simplified
            error_rate: 0.0, // Would need error tracking
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataServiceMetrics {
    pub total_messages_processed: u64,
    pub active_subscribers: usize,
    pub connected_exchanges: usize,
    pub avg_processing_latency_ns: u64,
    pub uptime_seconds: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataHealthCheck {
    pub is_healthy: bool,
    pub connected_exchanges: usize,
    pub total_exchanges: usize,
    pub last_message_time: DateTime<Utc>,
    pub error_rate: f64,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[tokio::test]
    async fn test_market_data_service() {
        let (tx, _rx) = unbounded();
        let service = MarketDataService::new(tx);

        service.start().await.unwrap();

        // Subscribe to market data
        let subscription_rx = service.subscribe_market_data(
            "test_subscriber".to_string(),
            vec!["BTCUSDT".to_string()],
            vec![MarketDataType::All],
            "binance".to_string(),
        ).await.unwrap();

        // Test metrics
        let metrics = service.get_service_metrics().await;
        assert_eq!(metrics.active_subscribers, 1);

        // Test health check
        let health = service.health_check().await;
        assert!(health.total_exchanges > 0);

        service.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_event_filtering() {
        let (tx, _rx) = unbounded();
        let service = MarketDataService::new(tx);

        let subscription = MarketDataSubscription {
            subscriber_id: "test".to_string(),
            symbols: vec!["BTCUSDT".to_string()],
            data_types: vec![MarketDataType::Trade],
            exchange: "binance".to_string(),
            created_at: Utc::now(),
        };

        let trade_event = MarketEvent::Trade(Trade {
            symbol: Symbol::new("BTCUSDT".to_string()),
            exchange: "binance".to_string(),
            price: Decimal::from(50000),
            quantity: Decimal::from(1),
            timestamp: Utc::now(),
            is_buyer_maker: false,
            sequence: 1,
        });

        // Should forward trade event for BTCUSDT
        assert!(service.should_forward_event(&subscription, &trade_event));

        let other_trade_event = MarketEvent::Trade(Trade {
            symbol: Symbol::new("ETHUSDT".to_string()),
            exchange: "binance".to_string(),
            price: Decimal::from(3000),
            quantity: Decimal::from(1),
            timestamp: Utc::now(),
            is_buyer_maker: false,
            sequence: 1,
        });

        // Should not forward trade event for ETHUSDT
        assert!(!service.should_forward_event(&subscription, &other_trade_event));
    }
}