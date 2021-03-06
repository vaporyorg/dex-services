#[path = "../data/mod.rs"]
mod data;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use pricegraph::{Orderbook, TokenPair};

pub fn read_default_orderbook() -> Orderbook {
    Orderbook::read(&*data::DEFAULT_ORDERBOOK).expect("error reading orderbook")
}

pub fn read(c: &mut Criterion) {
    c.bench_function("Orderbook::read", |b| b.iter(read_default_orderbook));
}

pub fn is_overlapping(c: &mut Criterion) {
    let orderbook = read_default_orderbook();

    c.bench_function("Orderbook::is_overlapping", |b| {
        b.iter(|| orderbook.is_overlapping())
    });
}

pub fn reduce_overlapping_orders(c: &mut Criterion) {
    c.bench_function("Orderbook::reduce_overlapping_orders", |b| {
        let orderbook = read_default_orderbook();
        b.iter_batched(
            || orderbook.clone(),
            |mut orderbook| orderbook.reduce_overlapping_orders(),
            BatchSize::SmallInput,
        )
    });
}

pub fn fill_market_order(c: &mut Criterion) {
    let dai_weth = TokenPair { buy: 7, sell: 1 };
    let eth = 10.0f64.powi(18);
    let volumes = &[0.1 * eth, eth, 10.0 * eth, 100.0 * eth, 1000.0 * eth];

    let mut group = c.benchmark_group("Orderbook::fill_market_order");
    for volume in volumes {
        group.bench_with_input(BenchmarkId::from_parameter(volume), volume, |b, &volume| {
            let orderbook = read_default_orderbook();
            b.iter_batched(
                || orderbook.clone(),
                |mut orderbook| orderbook.fill_market_order(black_box(dai_weth), volume),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();

    let mut group = c.benchmark_group("Orderbook::fill_market_order(reduced)");
    for volume in volumes {
        group.bench_with_input(BenchmarkId::from_parameter(volume), volume, |b, &volume| {
            let reduced_orderbook = {
                let mut orderbook = read_default_orderbook();
                orderbook.reduce_overlapping_orders();
                orderbook
            };
            b.iter_batched(
                || reduced_orderbook.clone(),
                |mut orderbook| orderbook.fill_market_order(black_box(dai_weth), volume),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    read,
    is_overlapping,
    reduce_overlapping_orders,
    fill_market_order,
);
criterion_main!(benches);
