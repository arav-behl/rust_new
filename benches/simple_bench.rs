use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wintermute_orderbook_engine::utils::SparseVector;
use wintermute_orderbook_engine::types::*;

fn bench_sparse_vector(c: &mut Criterion) {
    c.bench_function("sparse_vector_operations", |b| {
        b.iter(|| {
            let mut sparse = SparseVector::new(1000);
            for i in (0..100).step_by(10) {
                let _ = sparse.set(i, black_box(format!("value_{}", i)));
            }
            sparse
        });
    });
}

fn bench_order_creation(c: &mut Criterion) {
    c.bench_function("order_creation", |b| {
        b.iter(|| {
            Order::new(
                black_box("test_client".to_string()),
                black_box("BTCUSDT".to_string()),
                black_box(OrderSide::Buy),
                black_box(OrderType::Limit),
                black_box(rust_decimal::Decimal::from(1)),
                black_box(Some(rust_decimal::Decimal::from(50000))),
            )
        });
    });
}

criterion_group!(benches, bench_sparse_vector, bench_order_creation);
criterion_main!(benches);