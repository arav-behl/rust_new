use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_decimal::Decimal;
use std::time::Duration;

use wintermute_orderbook_engine::types::*;
use wintermute_orderbook_engine::orderbook::*;
use wintermute_orderbook_engine::engine::*;

fn bench_order_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_creation");

    for &order_count in &[100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(order_count as u64));
        group.bench_with_input(
            BenchmarkId::new("create_orders", order_count),
            &order_count,
            |b, &order_count| {
                b.iter(|| {
                    let mut orders = Vec::with_capacity(order_count);
                    for i in 0..order_count {
                        let order = Order::new(
                            format!("client_{}", i),
                            "BTCUSDT".to_string(),
                            if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                            OrderType::Limit,
                            Decimal::from(1),
                            Some(Decimal::from(50000 + i as i64)),
                        );
                        orders.push(black_box(order));
                    }
                    orders
                });
            },
        );
    }
    group.finish();
}

fn bench_orderbook_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("orderbook_operations");
    group.measurement_time(Duration::from_secs(20));

    let symbol = Symbol::new("BTCUSDT".to_string());
    let orderbook = OrderBook::new(symbol);

    // Pre-populate orderbook with some orders
    for i in 0..1000 {
        let order = Order::new(
            format!("client_{}", i),
            "BTCUSDT".to_string(),
            if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
            OrderType::Limit,
            Decimal::from(1),
            Some(Decimal::from(49000 + (i % 100) as i64)),
        );
        let _ = orderbook.add_order(order);
    }

    group.bench_function("add_order", |b| {
        let mut counter = 0;
        b.iter(|| {
            let order = Order::new(
                format!("bench_client_{}", counter),
                "BTCUSDT".to_string(),
                OrderSide::Buy,
                OrderType::Limit,
                Decimal::from(1),
                Some(Decimal::from(50000)),
            );
            counter += 1;
            black_box(orderbook.add_order(order))
        });
    });

    group.bench_function("get_market_depth", |b| {
        b.iter(|| black_box(orderbook.get_market_depth(10)));
    });

    group.bench_function("get_spread", |b| {
        b.iter(|| black_box(orderbook.get_spread()));
    });

    group.finish();
}

fn bench_matching_engine(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("matching_engine");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("submit_order", |b| {
        let engine = MatchingEngine::new();
        let mut counter = 0;

        b.to_async(&rt).iter(|| async {
            let order = Order::new(
                format!("bench_client_{}", counter),
                "BTCUSDT".to_string(),
                if counter % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                OrderType::Limit,
                Decimal::from(1),
                Some(Decimal::from(50000 + (counter % 100) as i64)),
            );
            counter += 1;
            black_box(engine.submit_order(order).await)
        });
    });

    group.finish();
}

fn bench_sparse_vector(c: &mut Criterion) {
    use wintermute_orderbook_engine::utils::SparseVector;

    let mut group = c.benchmark_group("sparse_vector");

    for &size in &[1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("set_operations", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut sparse = SparseVector::new(size);
                    for i in (0..size).step_by(10) {
                        sparse.set(i, format!("value_{}", i)).unwrap();
                    }
                    black_box(sparse)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_operations", size),
            &size,
            |b, &size| {
                let mut sparse = SparseVector::new(size);
                // Pre-populate
                for i in (0..size).step_by(10) {
                    sparse.set(i, format!("value_{}", i)).unwrap();
                }

                b.iter(|| {
                    let mut results = Vec::new();
                    for i in (0..size).step_by(20) {
                        results.push(black_box(sparse.get(i)));
                    }
                    results
                });
            },
        );
    }

    group.finish();
}

fn bench_latency_tracking(c: &mut Criterion) {
    use wintermute_orderbook_engine::utils::LatencyTracker;

    let mut group = c.benchmark_group("latency_tracking");

    group.bench_function("record_latency", |b| {
        let mut tracker = LatencyTracker::new(10000);
        let mut counter = 1000u64;

        b.iter(|| {
            tracker.record_latency(counter);
            counter += 100;
            black_box(&tracker)
        });
    });

    group.bench_function("get_distribution", |b| {
        let mut tracker = LatencyTracker::new(10000);
        // Pre-populate with some data
        for i in 1000..2000u64 {
            tracker.record_latency(i);
        }

        b.iter(|| black_box(tracker.get_distribution()));
    });

    group.finish();
}

fn bench_message_bus(c: &mut Criterion) {
    use wintermute_orderbook_engine::engine::{MessageBus, EngineMessage, EngineType};

    let mut group = c.benchmark_group("message_bus");

    group.bench_function("send_message", |b| {
        let bus = MessageBus::new();
        let mut counter = 0;

        b.iter(|| {
            let message = EngineMessage::Heartbeat(counter);
            counter += 1;
            black_box(bus.send_to_engine(EngineType::MarketData, message))
        });
    });

    group.bench_function("receive_message", |b| {
        let bus = MessageBus::new();
        // Pre-populate with messages
        for i in 0..1000 {
            let message = EngineMessage::Heartbeat(i);
            let _ = bus.send_to_engine(EngineType::MarketData, message);
        }

        b.iter(|| black_box(bus.receive_from_engine(EngineType::MarketData)));
    });

    group.finish();
}

fn bench_market_data_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_data_processing");

    let market_tick = MarketTick {
        symbol: Symbol::new("BTCUSDT".to_string()),
        exchange: "binance".to_string(),
        price: Decimal::from(50000),
        quantity: Decimal::from(1),
        side: TickSide::Trade,
        timestamp: chrono::Utc::now(),
        sequence: 12345,
    };

    group.bench_function("serialize_tick", |b| {
        b.iter(|| black_box(serde_json::to_string(&market_tick).unwrap()));
    });

    group.bench_function("deserialize_tick", |b| {
        let serialized = serde_json::to_string(&market_tick).unwrap();
        b.iter(|| {
            let tick: MarketTick = serde_json::from_str(&serialized).unwrap();
            black_box(tick)
        });
    });

    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");
    group.measurement_time(Duration::from_secs(20));

    for &thread_count in &[1, 2, 4, 8] {
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new("concurrent_order_submission", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.to_async(&rt).iter(|| async move {
                    let engine = MatchingEngine::new();
                    let handles: Vec<_> = (0..thread_count)
                        .map(|thread_id| {
                            let engine_clone = std::sync::Arc::new(engine);
                            tokio::spawn(async move {
                                for i in 0..250 {
                                    let order = Order::new(
                                        format!("client_{}_{}", thread_id, i),
                                        "BTCUSDT".to_string(),
                                        if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
                                        OrderType::Limit,
                                        Decimal::from(1),
                                        Some(Decimal::from(50000 + i as i64)),
                                    );
                                    let _ = engine_clone.submit_order(order).await;
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        let _ = handle.await;
                    }

                    black_box(())
                });
            },
        );
    }

    group.finish();
}

fn bench_memory_operations(c: &mut Criterion) {
    use wintermute_orderbook_engine::utils::ObjectPool;
    use tempfile::tempdir;
    use wintermute_orderbook_engine::utils::MemoryMappedRegion;

    let mut group = c.benchmark_group("memory_operations");

    // Object pool benchmarks
    group.bench_function("object_pool_get_return", |b| {
        let pool = ObjectPool::with_factory(1000, || String::from("test_string"));

        b.iter(|| {
            let obj = pool.get();
            black_box(&*obj); // Use the object
            // Object automatically returned when dropped
        });
    });

    // Memory mapped operations
    group.bench_function("memory_mapped_write", |b| {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("bench.mmap");
        let mut region = MemoryMappedRegion::create(file_path, 1024 * 1024).unwrap(); // 1MB

        let test_data = b"Hello, memory mapped world! This is a test string for benchmarking.";

        b.iter(|| {
            for offset in (0..1000).step_by(test_data.len()) {
                region.write_at(offset, test_data).unwrap();
            }
            black_box(&region);
        });
    });

    group.finish();
}

fn bench_end_to_end_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("end_to_end_latency");
    group.measurement_time(Duration::from_secs(25));

    // This benchmark measures the complete end-to-end latency from order submission
    // through matching engine to execution report
    group.bench_function("order_to_execution_report", |b| {
        let engine = OrderBookEngine::new();
        let rt_handle = rt.handle().clone();

        b.to_async(&rt).iter(|| async {
            rt_handle.spawn(async {
                let _ = engine.start().await;
            });

            let start_time = std::time::Instant::now();

            let order = Order::new(
                "benchmark_client".to_string(),
                "BTCUSDT".to_string(),
                OrderSide::Buy,
                OrderType::Limit,
                Decimal::from(1),
                Some(Decimal::from(50000)),
            );

            let reports = engine.submit_order(order).await.unwrap();
            let end_time = std::time::Instant::now();

            let latency_nanos = end_time.duration_since(start_time).as_nanos();

            black_box((reports, latency_nanos))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_order_creation,
    bench_orderbook_operations,
    bench_matching_engine,
    bench_sparse_vector,
    bench_latency_tracking,
    bench_message_bus,
    bench_market_data_processing,
    bench_concurrent_operations,
    bench_memory_operations,
    bench_end_to_end_latency
);

criterion_main!(benches);