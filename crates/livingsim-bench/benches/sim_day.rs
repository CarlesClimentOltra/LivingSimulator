//! `cargo bench -p livingsim-bench` — cost of one simulated day.
//! The 1M-agent case is slow; enable it with `LIVINGSIM_BENCH_1M=1`.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use livingsim_bench::{one_day, synthetic};

fn sim_day(c: &mut Criterion) {
    let mut sizes = vec![10_000u32, 100_000];
    if std::env::var("LIVINGSIM_BENCH_1M").is_ok() {
        sizes.push(1_000_000);
    }
    let mut g = c.benchmark_group("simulated_day");
    g.sample_size(10);
    for n in sizes {
        let world = synthetic(n);
        g.bench_function(format!("{n}_agents"), |b| {
            b.iter_batched(
                || world.clone(),
                |mut w| one_day(&mut w),
                BatchSize::LargeInput,
            )
        });
    }
    g.finish();
}

criterion_group!(benches, sim_day);
criterion_main!(benches);
