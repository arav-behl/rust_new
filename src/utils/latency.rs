use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use serde::{Serialize, Deserialize};

pub struct LatencyMeasurement {
    start_time: Instant,
    operation: String,
}

impl LatencyMeasurement {
    pub fn start(operation: &str) -> Self {
        Self {
            start_time: Instant::now(),
            operation: operation.to_string(),
        }
    }

    pub fn finish(self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyDistribution {
    pub min: u64,
    pub max: u64,
    pub mean: f64,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub p999: u64,
    pub sample_count: usize,
}

impl Default for LatencyDistribution {
    fn default() -> Self {
        Self {
            min: u64::MAX,
            max: 0,
            mean: 0.0,
            p50: 0,
            p95: 0,
            p99: 0,
            p999: 0,
            sample_count: 0,
        }
    }
}

#[derive(Debug)]
pub struct LatencyTracker {
    samples: VecDeque<u64>,
    max_samples: usize,
}

impl LatencyTracker {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn record_latency(&mut self, latency_nanos: u64) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(latency_nanos);
    }

    pub fn get_distribution(&self) -> LatencyDistribution {
        if self.samples.is_empty() {
            return LatencyDistribution::default();
        }

        let mut sorted_samples: Vec<u64> = self.samples.iter().cloned().collect();
        sorted_samples.sort_unstable();

        let len = sorted_samples.len();
        let p50_idx = len * 50 / 100;
        let p95_idx = len * 95 / 100;
        let p99_idx = len * 99 / 100;
        let p999_idx = len * 999 / 1000;

        let sum: u64 = sorted_samples.iter().sum();
        let mean = if len > 0 { sum as f64 / len as f64 } else { 0.0 };

        LatencyDistribution {
            min: sorted_samples.first().copied().unwrap_or(0),
            max: sorted_samples.last().copied().unwrap_or(0),
            mean,
            p50: sorted_samples.get(p50_idx).copied().unwrap_or(0),
            p95: sorted_samples.get(p95_idx).copied().unwrap_or(0),
            p99: sorted_samples.get(p99_idx).copied().unwrap_or(0),
            p999: sorted_samples.get(p999_idx).copied().unwrap_or(0),
            sample_count: len,
        }
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_tracker() {
        let mut tracker = LatencyTracker::new(1000);

        tracker.record_latency(1000);
        tracker.record_latency(2000);
        tracker.record_latency(3000);

        let dist = tracker.get_distribution();
        assert_eq!(dist.sample_count, 3);
        assert_eq!(dist.min, 1000);
        assert_eq!(dist.max, 3000);
    }

    #[test]
    fn test_shared_latency_tracker() {
        let tracker = SharedLatencyTracker::new(1000);

        tracker.record_latency(1000);
        tracker.record_latency(2000);
        tracker.record_latency(3000);

        let dist = tracker.get_distribution();
        assert_eq!(dist.sample_count, 3);
        assert_eq!(dist.min, 1000);
        assert_eq!(dist.max, 3000);
    }
}

// Thread-safe wrapper around LatencyTracker
#[derive(Debug, Clone)]
pub struct SharedLatencyTracker {
    inner: Arc<Mutex<LatencyTracker>>,
}

impl SharedLatencyTracker {
    pub fn new(max_samples: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LatencyTracker::new(max_samples))),
        }
    }

    pub fn record_latency(&self, latency_nanos: u64) {
        if let Ok(mut tracker) = self.inner.lock() {
            tracker.record_latency(latency_nanos);
        }
    }

    pub fn get_distribution(&self) -> LatencyDistribution {
        if let Ok(tracker) = self.inner.lock() {
            tracker.get_distribution()
        } else {
            LatencyDistribution::default()
        }
    }

    pub fn reset(&self) {
        if let Ok(mut tracker) = self.inner.lock() {
            tracker.reset();
        }
    }
}