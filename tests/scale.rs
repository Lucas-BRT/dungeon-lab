use dungeon_lab::generate::generate;
use dungeon_lab::params::params_of_level;
use dungeon_lab::stage::stage;

/// A stage on a bigger grid (`cargo test --release scale -- --nocapture`, with and without `--no-default-features`).
#[test]
fn scale() {
    for (cols, rows) in [(6, 5), (10, 8), (16, 12)] {
        let mut p = params_of_level(5);
        p.cols = cols;
        p.rows = rows;
        p.big = cols * rows / 12;
        p.leaves = cols * rows / 6;
        let n = 6u32;
        let t = std::time::Instant::now();
        let mut rooms = 0;
        for seed in 1..=n {
            rooms += generate(seed, &p).rooms.len();
        }
        println!("GRID {cols}x{rows} (~{} rooms): {:.1} ms per stage on {} threads", rooms / n as usize, t.elapsed().as_secs_f64() * 1000.0 / n as f64, dungeon_lab::par::threads());
    }
    // A batch of independent stages: what a gallery or a balance run does.
    let p = params_of_level(5);
    let t = std::time::Instant::now();
    let seeds: Vec<u32> = (1..=200).collect();
    let bad: usize = dungeon_lab::par::pmap(&seeds, |s| usize::from(!stage(*s, &p, 8).violations.is_empty())).iter().sum();
    println!("BATCH 200 stages of level 5: {:.0} ms total ({} invalid) on {} threads", t.elapsed().as_secs_f64() * 1000.0, bad, dungeon_lab::par::threads());
}

/// The phases of one big stage.
#[test]
fn big_phases() {
    let mut p = params_of_level(5);
    p.cols = 16;
    p.rows = 12;
    p.big = 16;
    p.leaves = 32;
    let d = generate(3, &p);
    let line: Vec<String> = d.phases.iter().map(|(k, v)| format!("{k} {v:.1}")).collect();
    println!("PHASES 16x12 on {} threads: {}", dungeon_lab::par::threads(), line.join(" · "));
}
