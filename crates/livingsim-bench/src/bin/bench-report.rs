//! Prints benchmark results as JSON in the "customSmallerIsBetter" format used by
//! github-action-benchmark: `[{"name": …, "unit": …, "value": …}]`.
//!
//! `cargo run --release -p livingsim-bench --bin bench-report -- [--full]`
//! (`--full` adds the 1M-agent world, ~1 GB of RAM.)

#![allow(clippy::disallowed_types)]

use livingsim_bench::{one_day, synthetic};
use livingsim_core::agents::Agents;
use livingsim_core::snapshot::encode_region;
use std::time::Instant;

fn main() {
    let full = std::env::args().any(|a| a == "--full");
    let mut out = Vec::new();
    let mut push = |name: String, unit: &str, value: f64| {
        eprintln!("{name}: {value:.1} {unit}");
        out.push(
            serde_json::json!({"name": name, "unit": unit, "value": (value * 10.0).round() / 10.0}),
        );
    };
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    eprintln!("threads: {threads}");
    let mut sizes = vec![10_000u32, 100_000];
    if full {
        sizes.push(1_000_000);
    }
    for n in sizes {
        let t0 = Instant::now();
        let mut w = synthetic(n);
        let gen = t0.elapsed();
        let e0 = w.events();
        let t0 = Instant::now();
        one_day(&mut w);
        let day = t0.elapsed();
        let pop = w.population() as f64;
        push(
            format!("1 simulated day, {n} agents"),
            "ms",
            day.as_secs_f64() * 1000.0,
        );
        push(
            format!("generation (incl. warm-up), {n} agents"),
            "ms",
            gen.as_secs_f64() * 1000.0,
        );
        push(
            format!("events per agent per day, {n} agents"),
            "events",
            (w.events() - e0) as f64 / pop,
        );
        if n == 10_000 {
            push(
                "region checkpoint size, 10k agents".into(),
                "KB",
                encode_region(&w.regions[0]).len() as f64 / 1024.0,
            );
        }
    }
    push(
        "memory per agent (columns + journal)".into(),
        "bytes",
        Agents::bytes_per_agent() as f64,
    );
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
