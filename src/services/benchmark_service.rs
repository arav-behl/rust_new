use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::utils::*;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBenchmark {
    pub operation_type: String,
    pub latency_ns: u64,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputBenchmark {
    pub operation_type: String,
    pub operations_per_second: f64,
    pub total_operations: u64,
    pub duration_seconds: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub latency_stats: HashMap<String, LatencyDistribution>,
    pub throughput_stats: HashMap<String, ThroughputBenchmark>,
    pub system_metrics: SystemMetrics,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub network_io_mbps: f64,
    pub disk_io_mbps: f64,
    pub active_connections: u32,
    pub thread_count: u32,
}

pub struct BenchmarkService {
    latency_trackers: Arc<RwLock<HashMap<String, SharedLatencyTracker>>>,
    throughput_counters: Arc<RwLock<HashMap<String, Arc<AtomicU64>>>>,
    benchmark_history: Arc<RwLock<Vec<LatencyBenchmark>>>,
    is_running: Arc<std::sync::atomic::AtomicBool>,
    start_time: Instant,
}

impl BenchmarkService {
    pub fn new() -> Self {
        Self {
            latency_trackers: Arc::new(RwLock::new(HashMap::new())),
            throughput_counters: Arc::new(RwLock::new(HashMap::new())),
            benchmark_history: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            start_time: Instant::now(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_running.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_err() {
            return Err(crate::EngineError::Internal("Benchmark service is already running".to_string()));
        }

        // Initialize common operation trackers
        self.register_operation("order_submission").await;
        self.register_operation("order_cancellation").await;
        self.register_operation("market_data_processing").await;
        self.register_operation("position_update").await;
        self.register_operation("websocket_roundtrip").await;
        self.register_operation("rest_api_call").await;

        tracing::info!("Benchmark service started");
        Ok(())
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        tracing::info!("Benchmark service stopped");
    }

    pub async fn register_operation(&self, operation_type: &str) {
        let mut trackers = self.latency_trackers.write().await;
        let mut counters = self.throughput_counters.write().await;

        trackers.entry(operation_type.to_string())
            .or_insert_with(|| SharedLatencyTracker::new(10000));

        counters.entry(operation_type.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)));
    }

    pub async fn record_latency(&self, operation_type: &str, latency_ns: u64) {
        self.record_latency_with_metadata(operation_type, latency_ns, true, HashMap::new()).await;
    }

    pub async fn record_latency_with_metadata(
        &self,
        operation_type: &str,
        latency_ns: u64,
        success: bool,
        metadata: HashMap<String, String>,
    ) {
        // Record in tracker
        if let Some(tracker) = self.latency_trackers.read().await.get(operation_type) {
            tracker.record_latency(latency_ns);
        }

        // Increment throughput counter
        if let Some(counter) = self.throughput_counters.read().await.get(operation_type) {
            counter.fetch_add(1, Ordering::Relaxed);
        }

        // Store in history (keep last 10000 records)
        let benchmark = LatencyBenchmark {
            operation_type: operation_type.to_string(),
            latency_ns,
            timestamp: Utc::now(),
            success,
            metadata,
        };

        let mut history = self.benchmark_history.write().await;
        history.push(benchmark);
        if history.len() > 10000 {
            history.remove(0);
        }
    }

    pub async fn measure_operation<F, T>(&self, operation_type: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let start = Instant::now();
        let result = operation.await;
        let latency_ns = start.elapsed().as_nanos() as u64;

        let success = result.is_ok();
        self.record_latency_with_metadata(
            operation_type,
            latency_ns,
            success,
            HashMap::new(),
        ).await;

        result
    }

    pub async fn benchmark_websocket_roundtrip(&self, websocket_url: &str) -> Result<u64> {
        use tokio_tungstenite::{connect_async, tungstenite::Message};
        use futures_util::{SinkExt, StreamExt};

        let start = Instant::now();

        let (ws_stream, _) = connect_async(websocket_url).await
            .map_err(|e| crate::EngineError::Internal(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        // Send ping
        write.send(Message::Ping(vec![])).await
            .map_err(|e| crate::EngineError::Internal(e.to_string()))?;

        // Wait for pong
        while let Some(msg) = read.next().await {
            match msg.map_err(|e| crate::EngineError::Internal(e.to_string()))? {
                Message::Pong(_) => break,
                _ => continue,
            }
        }

        let latency_ns = start.elapsed().as_nanos() as u64;

        self.record_latency_with_metadata(
            "websocket_roundtrip",
            latency_ns,
            true,
            [("url".to_string(), websocket_url.to_string())].into(),
        ).await;

        Ok(latency_ns)
    }

    pub async fn benchmark_rest_api_latency(&self, url: &str) -> Result<u64> {
        let client = reqwest::Client::new();
        let start = Instant::now();

        let response = client.get(url).send().await
            .map_err(|e| crate::EngineError::Network(e))?;

        let latency_ns = start.elapsed().as_nanos() as u64;
        let success = response.status().is_success();

        self.record_latency_with_metadata(
            "rest_api_call",
            latency_ns,
            success,
            [
                ("url".to_string(), url.to_string()),
                ("status".to_string(), response.status().to_string()),
            ].into(),
        ).await;

        Ok(latency_ns)
    }

    pub async fn run_throughput_benchmark(
        &self,
        operation_type: &str,
        operations_count: u64,
        concurrent_workers: usize,
    ) -> Result<ThroughputBenchmark> {
        let start = Instant::now();
        let operations_per_worker = operations_count / concurrent_workers as u64;

        // Spawn concurrent workers
        let mut handles = Vec::new();
        for _ in 0..concurrent_workers {
            let operation_type = operation_type.to_string();
            let benchmark_service = self.clone_for_worker();

            let handle = tokio::spawn(async move {
                for _ in 0..operations_per_worker {
                    // Simulate some operation
                    let op_start = Instant::now();
                    tokio::task::yield_now().await;
                    let op_latency = op_start.elapsed().as_nanos() as u64;

                    benchmark_service.record_latency(&operation_type, op_latency).await;
                }
            });

            handles.push(handle);
        }

        // Wait for all workers to complete
        for handle in handles {
            handle.await.map_err(|e| crate::EngineError::Internal(e.to_string()))?;
        }

        let duration = start.elapsed();
        let duration_seconds = duration.as_secs_f64();
        let ops_per_second = operations_count as f64 / duration_seconds;

        let benchmark = ThroughputBenchmark {
            operation_type: operation_type.to_string(),
            operations_per_second: ops_per_second,
            total_operations: operations_count,
            duration_seconds,
            timestamp: Utc::now(),
        };

        tracing::info!(
            "Throughput benchmark completed: {} ops/sec for {}",
            ops_per_second as u64,
            operation_type
        );

        Ok(benchmark)
    }

    fn clone_for_worker(&self) -> BenchmarkServiceWorker {
        BenchmarkServiceWorker {
            latency_trackers: Arc::clone(&self.latency_trackers),
            throughput_counters: Arc::clone(&self.throughput_counters),
            benchmark_history: Arc::clone(&self.benchmark_history),
        }
    }

    pub async fn get_performance_report(&self) -> PerformanceReport {
        let mut latency_stats = HashMap::new();
        let mut throughput_stats = HashMap::new();

        // Collect latency stats
        let trackers = self.latency_trackers.read().await;
        for (operation, tracker) in trackers.iter() {
            latency_stats.insert(operation.clone(), tracker.get_distribution());
        }

        // Calculate throughput stats
        let counters = self.throughput_counters.read().await;
        let elapsed = self.start_time.elapsed();
        let duration_seconds = elapsed.as_secs_f64();

        for (operation, counter) in counters.iter() {
            let total_operations = counter.load(Ordering::Relaxed);
            let ops_per_second = if duration_seconds > 0.0 {
                total_operations as f64 / duration_seconds
            } else {
                0.0
            };

            throughput_stats.insert(operation.clone(), ThroughputBenchmark {
                operation_type: operation.clone(),
                operations_per_second: ops_per_second,
                total_operations,
                duration_seconds,
                timestamp: Utc::now(),
            });
        }

        PerformanceReport {
            latency_stats,
            throughput_stats,
            system_metrics: self.get_system_metrics().await,
            timestamp: Utc::now(),
        }
    }

    async fn get_system_metrics(&self) -> SystemMetrics {
        // In a real implementation, these would use system APIs
        // For now, we'll provide placeholder values
        SystemMetrics {
            cpu_usage_percent: 25.5,
            memory_usage_mb: 128.0,
            network_io_mbps: 10.5,
            disk_io_mbps: 5.2,
            active_connections: 42,
            thread_count: std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(8),
        }
    }

    pub async fn get_latency_percentiles(&self, operation_type: &str) -> Option<LatencyDistribution> {
        self.latency_trackers
            .read()
            .await
            .get(operation_type)
            .map(|tracker| tracker.get_distribution())
    }

    pub async fn get_benchmark_history(
        &self,
        operation_type: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<LatencyBenchmark> {
        let history = self.benchmark_history.read().await;

        let filtered: Vec<LatencyBenchmark> = if let Some(op_type) = operation_type {
            history.iter()
                .filter(|b| b.operation_type == op_type)
                .cloned()
                .collect()
        } else {
            history.clone()
        };

        let mut result = filtered;
        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Most recent first

        if let Some(limit) = limit {
            result.truncate(limit);
        }

        result
    }

    pub async fn print_performance_summary(&self) {
        let report = self.get_performance_report().await;

        println!("🚀 Performance Benchmark Report");
        println!("================================");
        println!();

        println!("📊 Latency Statistics:");
        for (operation, stats) in &report.latency_stats {
            if stats.sample_count > 0 {
                println!("  {} ({} samples):", operation, stats.sample_count);
                println!("    Mean:     {:.1}μs", stats.mean / 1000.0);
                println!("    P50:      {:.1}μs", stats.p50 as f64 / 1000.0);
                println!("    P95:      {:.1}μs", stats.p95 as f64 / 1000.0);
                println!("    P99:      {:.1}μs", stats.p99 as f64 / 1000.0);
                println!("    P99.9:    {:.1}μs", stats.p999 as f64 / 1000.0);
                println!();
            }
        }

        println!("🏎️ Throughput Statistics:");
        for (operation, stats) in &report.throughput_stats {
            if stats.total_operations > 0 {
                println!("  {}: {:.0} ops/sec ({} total operations)",
                    operation,
                    stats.operations_per_second,
                    stats.total_operations
                );
            }
        }
        println!();

        println!("💻 System Metrics:");
        println!("  CPU Usage:     {:.1}%", report.system_metrics.cpu_usage_percent);
        println!("  Memory Usage:  {:.1} MB", report.system_metrics.memory_usage_mb);
        println!("  Threads:       {}", report.system_metrics.thread_count);
        println!("  Connections:   {}", report.system_metrics.active_connections);
        println!();
    }
}

// Helper struct for worker threads
struct BenchmarkServiceWorker {
    latency_trackers: Arc<RwLock<HashMap<String, SharedLatencyTracker>>>,
    throughput_counters: Arc<RwLock<HashMap<String, Arc<AtomicU64>>>>,
    benchmark_history: Arc<RwLock<Vec<LatencyBenchmark>>>,
}

impl BenchmarkServiceWorker {
    async fn record_latency(&self, operation_type: &str, latency_ns: u64) {
        if let Some(tracker) = self.latency_trackers.read().await.get(operation_type) {
            tracker.record_latency(latency_ns);
        }

        if let Some(counter) = self.throughput_counters.read().await.get(operation_type) {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_benchmark_service() {
        let benchmark = BenchmarkService::new();
        benchmark.start().await.unwrap();

        // Record some test latencies
        benchmark.record_latency("test_operation", 5000).await; // 5μs
        benchmark.record_latency("test_operation", 10000).await; // 10μs
        benchmark.record_latency("test_operation", 15000).await; // 15μs

        let stats = benchmark.get_latency_percentiles("test_operation").await.unwrap();
        assert_eq!(stats.sample_count, 3);
        assert!(stats.mean > 0.0);

        let report = benchmark.get_performance_report().await;
        assert!(report.latency_stats.contains_key("test_operation"));

        benchmark.stop();
    }

    #[tokio::test]
    async fn test_throughput_benchmark() {
        let benchmark = BenchmarkService::new();
        benchmark.start().await.unwrap();

        let result = benchmark.run_throughput_benchmark("test_throughput", 1000, 4).await.unwrap();
        assert_eq!(result.total_operations, 1000);
        assert!(result.operations_per_second > 0.0);

        benchmark.stop();
    }
}