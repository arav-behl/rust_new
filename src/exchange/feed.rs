use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel::{Receiver, Sender};
use tokio::time::{Duration, Interval};

use crate::types::*;
use crate::utils::*;
use crate::Result;

pub struct MarketDataFeed {
    market_data_rx: Receiver<MarketEvent>,
    subscribers: Arc<parking_lot::RwLock<HashMap<String, Sender<MarketEvent>>>>,
    metrics: Arc<AtomicMetrics>,
    latency_tracker: SharedLatencyTracker,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    message_count: Arc<AtomicU64>,
}

impl MarketDataFeed {
    pub fn new(market_data_rx: Receiver<MarketEvent>) -> Self {
        Self {
            market_data_rx,
            subscribers: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            metrics: Arc::new(AtomicMetrics::new()),
            latency_tracker: SharedLatencyTracker::new(10000),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            message_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Market data feed is already running".to_string()));
        }

        let market_data_rx = self.market_data_rx.clone();
        let subscribers = Arc::clone(&self.subscribers);
        let metrics = Arc::clone(&self.metrics);
        let latency_tracker = self.latency_tracker.clone();
        let is_running = Arc::clone(&self.is_running);
        let message_count = Arc::clone(&self.message_count);

        tokio::spawn(async move {
            Self::feed_loop(
                market_data_rx,
                subscribers,
                metrics,
                latency_tracker,
                is_running,
                message_count,
            ).await;
        });

        tracing::info!("Market data feed started");
        Ok(())
    }

    pub fn stop(&self) {
        use std::sync::atomic::Ordering;

        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Market data feed stopped");
    }

    pub fn subscribe(&self, subscriber_id: String) -> Receiver<MarketEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.subscribers.write().insert(subscriber_id.clone(), tx);
        tracing::info!("Added market data subscriber: {}", subscriber_id);
        rx
    }

    pub fn unsubscribe(&self, subscriber_id: &str) {
        if self.subscribers.write().remove(subscriber_id).is_some() {
            tracing::info!("Removed market data subscriber: {}", subscriber_id);
        }
    }

    pub fn get_subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    pub fn get_metrics(&self) -> FeedMetrics {
        FeedMetrics {
            messages_processed: self.message_count.load(Ordering::Relaxed),
            subscriber_count: self.get_subscriber_count(),
            latency_distribution: self.latency_tracker.get_distribution(),
            is_running: self.is_running.load(Ordering::Relaxed),
        }
    }

    async fn feed_loop(
        market_data_rx: Receiver<MarketEvent>,
        subscribers: Arc<parking_lot::RwLock<HashMap<String, Sender<MarketEvent>>>>,
        metrics: Arc<AtomicMetrics>,
        latency_tracker: SharedLatencyTracker,
        is_running: Arc<std::sync::atomic::AtomicBool>,
        message_count: Arc<AtomicU64>,
    ) {
        tracing::info!("Market data feed loop started");

        while is_running.load(Ordering::Relaxed) {
            match market_data_rx.try_recv() {
                Ok(market_event) => {
                    let processing_start = std::time::Instant::now();

                    // Broadcast to all subscribers
                    let subscribers_read = subscribers.read();
                    let mut failed_subscribers = Vec::new();

                    for (subscriber_id, sender) in subscribers_read.iter() {
                        if let Err(_) = sender.try_send(market_event.clone()) {
                            failed_subscribers.push(subscriber_id.clone());
                        }
                    }
                    drop(subscribers_read);

                    // Remove failed subscribers
                    if !failed_subscribers.is_empty() {
                        let mut subscribers_write = subscribers.write();
                        for failed_id in failed_subscribers {
                            subscribers_write.remove(&failed_id);
                            tracing::warn!("Removed failed subscriber: {}", failed_id);
                        }
                    }

                    let processing_time = processing_start.elapsed().as_nanos() as u64;
                    latency_tracker.record_latency(processing_time);
                    metrics.increment_market_data_events();
                    message_count.fetch_add(1, Ordering::Relaxed);
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // No message available, yield to avoid busy waiting
                    tokio::task::yield_now().await;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    tracing::warn!("Market data channel disconnected");
                    break;
                }
            }
        }

        tracing::info!("Market data feed loop stopped");
    }
}

#[derive(Debug, Clone)]
pub struct FeedMetrics {
    pub messages_processed: u64,
    pub subscriber_count: usize,
    pub latency_distribution: LatencyDistribution,
    pub is_running: bool,
}

// Multi-level caching as mentioned in the requirements
pub struct CachedMarketData {
    l1_cache: Arc<parking_lot::RwLock<HashMap<Symbol, MarketEvent>>>, // In-memory cache
    l2_cache: Option<redis::Client>, // Redis cache (optional)
    cache_hit_count: Arc<AtomicU64>,
    cache_miss_count: Arc<AtomicU64>,
}

impl CachedMarketData {
    pub fn new() -> Self {
        Self {
            l1_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            l2_cache: None,
            cache_hit_count: Arc::new(AtomicU64::new(0)),
            cache_miss_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_redis(mut self, redis_url: &str) -> Result<Self> {
        match redis::Client::open(redis_url) {
            Ok(client) => {
                self.l2_cache = Some(client);
                tracing::info!("Connected to Redis L2 cache: {}", redis_url);
                Ok(self)
            }
            Err(e) => {
                tracing::warn!("Failed to connect to Redis: {}", e);
                Ok(self) // Continue without Redis
            }
        }
    }

    pub fn get(&self, symbol: &Symbol) -> Option<MarketEvent> {
        // Try L1 cache first (fastest ~10µs)
        if let Some(event) = self.l1_cache.read().get(symbol) {
            self.cache_hit_count.fetch_add(1, Ordering::Relaxed);
            return Some(event.clone());
        }

        // Try L2 cache if available (Redis ~100µs)
        if let Some(ref redis_client) = self.l2_cache {
            // Redis implementation would go here
            // For now, just increment miss count
            self.cache_miss_count.fetch_add(1, Ordering::Relaxed);
        }

        None
    }

    pub fn put(&self, symbol: Symbol, event: MarketEvent) {
        // Update L1 cache
        self.l1_cache.write().insert(symbol.clone(), event.clone());

        // Update L2 cache if available
        if let Some(ref _redis_client) = self.l2_cache {
            // Redis implementation would go here
            // Serialize and store in Redis with TTL
        }
    }

    pub fn get_cache_stats(&self) -> CacheStats {
        let hits = self.cache_hit_count.load(Ordering::Relaxed);
        let misses = self.cache_miss_count.load(Ordering::Relaxed);
        let total = hits + misses;

        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        CacheStats {
            l1_size: self.l1_cache.read().len(),
            l2_connected: self.l2_cache.is_some(),
            hit_count: hits,
            miss_count: misses,
            hit_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub l1_size: usize,
    pub l2_connected: bool,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
}

impl Default for CachedMarketData {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_market_data_feed() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let feed = MarketDataFeed::new(rx);

        feed.start().await.unwrap();

        // Add a subscriber
        let subscriber_rx = feed.subscribe("test_subscriber".to_string());
        assert_eq!(feed.get_subscriber_count(), 1);

        // Send a market event
        let market_event = MarketEvent::Tick(MarketTick {
            symbol: Symbol::new("BTCUSDT".to_string()),
            exchange: "binance".to_string(),
            price: rust_decimal::Decimal::from(50000),
            quantity: rust_decimal::Decimal::from(1),
            side: TickSide::Trade,
            timestamp: chrono::Utc::now(),
            sequence: 1,
        });

        tx.send(market_event.clone()).unwrap();

        // Wait for processing
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Check that subscriber received the event
        let received_event = subscriber_rx.try_recv().unwrap();
        match (market_event, received_event) {
            (MarketEvent::Tick(sent), MarketEvent::Tick(received)) => {
                assert_eq!(sent.symbol, received.symbol);
                assert_eq!(sent.price, received.price);
            }
            _ => panic!("Event type mismatch"),
        }

        feed.stop();
    }

    #[test]
    fn test_cached_market_data() {
        let cache = CachedMarketData::new();
        let symbol = Symbol::new("BTCUSDT".to_string());

        // Test cache miss
        assert!(cache.get(&symbol).is_none());

        // Add to cache
        let market_event = MarketEvent::Tick(MarketTick {
            symbol: symbol.clone(),
            exchange: "binance".to_string(),
            price: rust_decimal::Decimal::from(50000),
            quantity: rust_decimal::Decimal::from(1),
            side: TickSide::Trade,
            timestamp: chrono::Utc::now(),
            sequence: 1,
        });

        cache.put(symbol.clone(), market_event.clone());

        // Test cache hit
        let cached_event = cache.get(&symbol).unwrap();
        match (market_event, cached_event) {
            (MarketEvent::Tick(original), MarketEvent::Tick(cached)) => {
                assert_eq!(original.symbol, cached.symbol);
                assert_eq!(original.price, cached.price);
            }
            _ => panic!("Event type mismatch"),
        }

        let stats = cache.get_cache_stats();
        assert_eq!(stats.hit_count, 1);
        assert_eq!(stats.l1_size, 1);
    }
}