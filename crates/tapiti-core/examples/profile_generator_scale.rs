//! Phase timings and output sizes for the complete generator at increasing grid sizes.
//! Run with `cargo run --release -p tapiti-core --features diagnostic-profile --example profile_generator_scale`.

use std::{io::Write, time::Instant};
use tapiti_core::{generate, params, validate};

fn main() {
    for (cols, rows) in [(10, 8), (16, 12), (30, 30)] {
        for seed in [0xD06E_2026, 0xD06E_2027, 0xD06E_2028] {
            let mut p = params::rich_of_size(params::Size::Medium);
            p.cols = cols;
            p.rows = rows;
            println!("starting {cols}x{rows}, seed={seed:#x}");
            std::io::stdout().flush().unwrap();

            let started = Instant::now();
            let floor = generate::generate(seed, &p);
            let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
            let phase_total_ms: f64 = floor.phases.iter().map(|(_, ms)| ms).sum();
            let phases = floor
                .phases
                .iter()
                .map(|(name, ms)| format!("{name}={ms:.2}ms"))
                .collect::<Vec<_>>()
                .join(", ");
            let alive_rooms = floor.rooms.iter().filter(|room| room.alive).count();
            let violations = validate::validate(&floor);
            let shape_fallbacks = floor
                .log
                .iter()
                .filter(|message| message.ends_with("does not fit, rect instead"))
                .count();
            let other_logs = floor
                .log
                .iter()
                .filter(|message| !message.ends_with("does not fit, rect instead"))
                .cloned()
                .collect::<Vec<_>>();

            println!(
                "grid={cols}x{rows} seed={seed:#x} threads={} rooms={}/{} alive pairs={} candidates={} edges={} nodes={} beta={}/{} dropped={} shape_fallbacks={} validation_errors={} wall={elapsed_ms:.2}ms phase_sum={phase_total_ms:.2}ms",
                tapiti_core::par::threads(),
                alive_rooms,
                floor.rooms.len(),
                floor.pairs.len(),
                floor.candidates.len(),
                floor.edges.len(),
                floor.nodes.len(),
                floor.beta_target,
                floor.beta_max,
                floor.dropped.len(),
                shape_fallbacks,
                violations.len(),
            );
            println!("phases: {phases}");
            if !violations.is_empty() {
                println!("validation: {}", violations.join(" | "));
            }
            if !other_logs.is_empty() {
                println!("generator notes: {}", other_logs.join(" | "));
            }
        }
    }
}
