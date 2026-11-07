//! Phase timings and output sizes for the complete generator at increasing grid sizes.
//! Run with `cargo run --release -p dungeon-lab-core --features diagnostic-profile --example profile_generator_scale`.

use dungeon_lab_core::{generate, params, validate};
use std::{io::Write, time::Instant};

fn main() {
    for (cols, rows) in [(10, 8), (16, 12), (30, 30)] {
        for seed in [0xD06E_2026, 0xD06E_2027, 0xD06E_2028] {
            let mut p = params::params_of_level(5);
            p.cols = cols;
            p.rows = rows;
            println!("starting {cols}x{rows}, seed={seed:#x}");
            std::io::stdout().flush().unwrap();

            let started = Instant::now();
            let dungeon = generate::generate(seed, &p);
            let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
            let phase_total_ms: f64 = dungeon.phases.iter().map(|(_, ms)| ms).sum();
            let phases = dungeon
                .phases
                .iter()
                .map(|(name, ms)| format!("{name}={ms:.2}ms"))
                .collect::<Vec<_>>()
                .join(", ");
            let alive_rooms = dungeon.rooms.iter().filter(|room| room.alive).count();
            let violations = validate::validate(&dungeon);
            let shape_fallbacks = dungeon
                .log
                .iter()
                .filter(|message| message.ends_with("does not fit, rect instead"))
                .count();
            let other_logs = dungeon
                .log
                .iter()
                .filter(|message| !message.ends_with("does not fit, rect instead"))
                .cloned()
                .collect::<Vec<_>>();

            println!(
                "grid={cols}x{rows} seed={seed:#x} threads={} rooms={}/{} alive pairs={} candidates={} edges={} nodes={} beta={}/{} dropped={} shape_fallbacks={} validation_errors={} wall={elapsed_ms:.2}ms phase_sum={phase_total_ms:.2}ms",
                dungeon_lab_core::par::threads(),
                alive_rooms,
                dungeon.rooms.len(),
                dungeon.pairs.len(),
                dungeon.candidates.len(),
                dungeon.edges.len(),
                dungeon.nodes.len(),
                dungeon.beta_target,
                dungeon.beta_max,
                dungeon.dropped.len(),
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
