//! Placeholder benchmark file.
//! Real benchmarks will be added as modules are implemented.
//!
//! Run with: cargo bench

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("placeholder_noop", |b| b.iter(|| std::hint::black_box(42)));
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
