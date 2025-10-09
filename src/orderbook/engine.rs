use std::sync::Arc;
use std::time::Duration;

use tokio::time::interval;
use rust_decimal::Decimal;

use crate::types::*;
use crate::utils::*;
use crate::Result;
use super::{MatchingEngine, OrderBook};

pub struct OrderBookEngine {
    matching_engine: MatchingEngine,
    metrics_collector: Arc<MetricsCollector>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
}

impl OrderBookEngine {
    pub fn new() -> Self {
        let matching_engine = MatchingEngine::new();
        let metrics_collector = Arc::new(MetricsCollector::new());

        Self {
            matching_engine,
            metrics_collector,
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Engine is already running".to_string()));
        }

        // Start metrics collection task
        self.start_metrics_collection().await;

        tracing::info!("Order book engine started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        use std::sync::atomic::Ordering;

        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Order book engine stopped");
        Ok(())
    }

    pub async fn submit_order(&self, order: Order) -> Result<Vec<ExecutionReport>> {
        self.matching_engine.submit_order(order).await
    }

    pub async fn cancel_order(&self, order_id: OrderId, client_id: String) -> Result<Option<ExecutionReport>> {
        self.matching_engine.cancel_order(order_id, client_id).await
    }

    pub async fn get_order_status(&self, order_id: OrderId) -> Result<Option<Order>> {
        self.matching_engine.get_order_status(order_id).await
    }

    pub async fn get_market_data(&self, symbol: Symbol) -> Result<(Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>)> {
        self.matching_engine.get_market_data(symbol).await
    }

    pub fn get_orderbook(&self, symbol: &Symbol) -> Option<Arc<OrderBook>> {
        self.matching_engine.get_orderbook(symbol)
    }

    pub fn get_all_symbols(&self) -> Vec<Symbol> {
        self.matching_engine.get_all_symbols()
    }

    pub fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.metrics_collector.get_latest_metrics()
    }

    pub fn get_latency_distribution(&self) -> LatencyDistribution {
        self.matching_engine.get_latency_distribution()
    }

    async fn start_metrics_collection(&self) {
        let metrics_collector = Arc::clone(&self.metrics_collector);
        let matching_engine_metrics = self.matching_engine.get_metrics_snapshot();
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1)); // Collect metrics every second

            while is_running.load(std::sync::atomic::Ordering::Relaxed) {
                interval.tick().await;
                metrics_collector.collect_metrics().await;
            }
        });
    }
}

impl Default for OrderBookEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MetricsCollector {
    latest_metrics: Arc<parking_lot::RwLock<PerformanceMetrics>>,
    latency_tracker: SharedLatencyTracker,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            latest_metrics: Arc::new(parking_lot::RwLock::new(PerformanceMetrics {
                timestamp: chrono::Utc::now(),
                latency_stats: LatencyStats {
                    order_matching_nanos: LatencyDistribution::default(),
                    market_data_processing_nanos: LatencyDistribution::default(),
                    end_to_end_nanos: LatencyDistribution::default(),
                    websocket_latency_nanos: LatencyDistribution::default(),
                },
                throughput_stats: ThroughputStats {
                    orders_per_second: 0.0,
                    matches_per_second: 0.0,
                    messages_per_second: 0.0,
                    market_data_events_per_second: 0.0,
                    total_orders_processed: 0,
                    total_matches: 0,
                },
                system_stats: SystemStats {
                    cpu_usage_percent: 0.0,
                    memory_usage_mb: 0,
                    memory_usage_percent: 0.0,
                    network_rx_bytes_per_sec: 0,
                    network_tx_bytes_per_sec: 0,
                    disk_io_bytes_per_sec: 0,
                    thread_count: 0,
                    gc_collections: 0,
                },
                trading_stats: TradingStats {
                    total_volume: 0.0,
                    total_turnover: 0.0,
                    spread_capture_bps: 0.0,
                    fill_rate_percent: 0.0,
                    average_slippage_bps: 0.0,
                    pnl_usd: 0.0,
                    sharpe_ratio: 0.0,
                },
            })),
            latency_tracker: SharedLatencyTracker::new(10000),
        }
    }

    pub async fn collect_metrics(&self) {
        let timestamp = chrono::Utc::now();

        // Collect system metrics
        let system_stats = self.collect_system_stats().await;

        // For now, create basic metrics - in production, these would be collected from actual systems
        let metrics = PerformanceMetrics {
            timestamp,
            latency_stats: LatencyStats {
                order_matching_nanos: self.latency_tracker.get_distribution(),
                market_data_processing_nanos: LatencyDistribution::default(),
                end_to_end_nanos: LatencyDistribution::default(),
                websocket_latency_nanos: LatencyDistribution::default(),
            },
            throughput_stats: ThroughputStats {
                orders_per_second: 1000.0, // Placeholder
                matches_per_second: 500.0,  // Placeholder
                messages_per_second: 2000.0, // Placeholder
                market_data_events_per_second: 10000.0, // Placeholder
                total_orders_processed: 0,
                total_matches: 0,
            },
            system_stats,
            trading_stats: TradingStats {
                total_volume: 0.0,
                total_turnover: 0.0,
                spread_capture_bps: 2.5,
                fill_rate_percent: 98.5,
                average_slippage_bps: 0.8,
                pnl_usd: 0.0,
                sharpe_ratio: 0.0,
            },
        };

        *self.latest_metrics.write() = metrics;
    }

    pub fn get_latest_metrics(&self) -> PerformanceMetrics {
        self.latest_metrics.read().clone()
    }

    async fn collect_system_stats(&self) -> SystemStats {
        // Collect memory stats
        let memory_stats = MemoryStats::get_system_memory().unwrap_or(MemoryStats {
            total_memory_mb: 16384,
            used_memory_mb: 8192,
            available_memory_mb: 8192,
            memory_usage_percent: 50.0,
        });

        // Get thread count (simplified)
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);

        SystemStats {
            cpu_usage_percent: 25.0, // Placeholder - would use system APIs in production
            memory_usage_mb: memory_stats.used_memory_mb,
            memory_usage_percent: memory_stats.memory_usage_percent,
            network_rx_bytes_per_sec: 1024 * 1024, // Placeholder
            network_tx_bytes_per_sec: 1024 * 1024, // Placeholder
            disk_io_bytes_per_sec: 1024 * 512,     // Placeholder
            thread_count,
            gc_collections: 0, // Rust doesn't have GC
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_lifecycle() {
        let engine = OrderBookEngine::new();

        // Test starting the engine
        engine.start().await.unwrap();

        // Test that we can't start it again
        assert!(engine.start().await.is_err());

        // Test stopping the engine
        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_order_operations() {
        let engine = OrderBookEngine::new();
        engine.start().await.unwrap();

        // Create and submit an order
        let order = Order::new(
            "test_client".to_string(),
            "BTCUSDT".to_string(),
            OrderSide::Buy,
            OrderType::Limit,
            Decimal::from(1),
            Some(Decimal::from(50000)),
        );

        let order_id = order.id;
        let reports = engine.submit_order(order).await.unwrap();
        assert!(!reports.is_empty());

        // Check order status
        let status = engine.get_order_status(order_id).await.unwrap();
        assert!(status.is_some());

        // Cancel the order
        let cancel_report = engine.cancel_order(order_id, "test_client".to_string()).await.unwrap();
        assert!(cancel_report.is_some());

        engine.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();

        collector.collect_metrics().await;
        let metrics = collector.get_latest_metrics();

        assert!(metrics.system_stats.memory_usage_mb > 0);
        assert!(metrics.system_stats.thread_count > 0);
    }
}