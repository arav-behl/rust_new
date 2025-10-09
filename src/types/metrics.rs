use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::utils::latency::LatencyDistribution;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub timestamp: DateTime<Utc>,
    pub latency_stats: LatencyStats,
    pub throughput_stats: ThroughputStats,
    pub system_stats: SystemStats,
    pub trading_stats: TradingStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub order_matching_nanos: LatencyDistribution,
    pub market_data_processing_nanos: LatencyDistribution,
    pub end_to_end_nanos: LatencyDistribution,
    pub websocket_latency_nanos: LatencyDistribution,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputStats {
    pub orders_per_second: f64,
    pub matches_per_second: f64,
    pub messages_per_second: f64,
    pub market_data_events_per_second: f64,
    pub total_orders_processed: u64,
    pub total_matches: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_usage_percent: f64,
    pub network_rx_bytes_per_sec: u64,
    pub network_tx_bytes_per_sec: u64,
    pub disk_io_bytes_per_sec: u64,
    pub thread_count: usize,
    pub gc_collections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingStats {
    pub total_volume: f64,
    pub total_turnover: f64,
    pub spread_capture_bps: f64,
    pub fill_rate_percent: f64,
    pub average_slippage_bps: f64,
    pub pnl_usd: f64,
    pub sharpe_ratio: f64,
}

#[derive(Debug)]
pub struct AtomicMetrics {
    pub orders_processed: AtomicU64,
    pub orders_matched: AtomicU64,
    pub orders_cancelled: AtomicU64,
    pub orders_rejected: AtomicU64,
    pub messages_received: AtomicU64,
    pub messages_sent: AtomicU64,
    pub market_data_events: AtomicU64,
    pub websocket_reconnects: AtomicU64,
    pub last_reset: AtomicU64,
}

impl AtomicMetrics {
    pub fn new() -> Self {
        Self {
            orders_processed: AtomicU64::new(0),
            orders_matched: AtomicU64::new(0),
            orders_cancelled: AtomicU64::new(0),
            orders_rejected: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            messages_sent: AtomicU64::new(0),
            market_data_events: AtomicU64::new(0),
            websocket_reconnects: AtomicU64::new(0),
            last_reset: AtomicU64::new(Utc::now().timestamp_nanos() as u64),
        }
    }

    pub fn increment_orders_processed(&self) -> u64 {
        self.orders_processed.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_orders_matched(&self) -> u64 {
        self.orders_matched.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_orders_cancelled(&self) -> u64 {
        self.orders_cancelled.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_orders_rejected(&self) -> u64 {
        self.orders_rejected.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_messages_received(&self) -> u64 {
        self.messages_received.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_messages_sent(&self) -> u64 {
        self.messages_sent.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_market_data_events(&self) -> u64 {
        self.market_data_events.fetch_add(1, Ordering::Relaxed)
    }

    pub fn increment_websocket_reconnects(&self) -> u64 {
        self.websocket_reconnects.fetch_add(1, Ordering::Relaxed)
    }

    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            orders_processed: self.orders_processed.load(Ordering::Relaxed),
            orders_matched: self.orders_matched.load(Ordering::Relaxed),
            orders_cancelled: self.orders_cancelled.load(Ordering::Relaxed),
            orders_rejected: self.orders_rejected.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            market_data_events: self.market_data_events.load(Ordering::Relaxed),
            websocket_reconnects: self.websocket_reconnects.load(Ordering::Relaxed),
            timestamp: Utc::now(),
        }
    }

    pub fn reset(&self) {
        self.orders_processed.store(0, Ordering::Relaxed);
        self.orders_matched.store(0, Ordering::Relaxed);
        self.orders_cancelled.store(0, Ordering::Relaxed);
        self.orders_rejected.store(0, Ordering::Relaxed);
        self.messages_received.store(0, Ordering::Relaxed);
        self.messages_sent.store(0, Ordering::Relaxed);
        self.market_data_events.store(0, Ordering::Relaxed);
        self.websocket_reconnects.store(0, Ordering::Relaxed);
        self.last_reset.store(Utc::now().timestamp_nanos() as u64, Ordering::Relaxed);
    }
}

impl Default for AtomicMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub orders_processed: u64,
    pub orders_matched: u64,
    pub orders_cancelled: u64,
    pub orders_rejected: u64,
    pub messages_received: u64,
    pub messages_sent: u64,
    pub market_data_events: u64,
    pub websocket_reconnects: u64,
    pub timestamp: DateTime<Utc>,
}

pub type SharedMetrics = Arc<AtomicMetrics>;