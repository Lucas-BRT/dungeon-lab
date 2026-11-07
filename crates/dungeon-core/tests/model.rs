use dungeon_lab_core::generate::generate;
use dungeon_lab_core::metrics::metrics_of;
use dungeon_lab_core::params::params_of_level;
use dungeon_lab_core::stage::stage;
use dungeon_lab_core::validate::validate;

/// The model's promise (`docs/dungeon-model.md` §9): every level's stages pass the validator.
#[test]
fn every_level_gives_only_valid_stages() {
    for level in 1..=5 {
        let p = params_of_level(level);
        for seed in [101u32, 202, 303, 404, 505] {
            let s = stage(seed, &p, 8);
            assert!(
                s.violations.is_empty(),
                "level {level} seed {seed}: {:?}",
                s.violations
            );
        }
    }
}

/// The stage, without the clock readings of `phases` (the only thing that differs between two runs).
fn without_clock(seed: u32, level: usize) -> serde_json::Value {
    let mut v = serde_json::to_value(generate(seed, &params_of_level(level))).unwrap();
    v.as_object_mut().unwrap().remove("phases");
    v
}

#[test]
fn the_same_seed_and_lambda_give_the_same_stage() {
    for level in [3, 5] {
        assert_eq!(without_clock(7, level), without_clock(7, level));
    }
}

#[test]
fn density_follows_delta() {
    let mut beta = [0.0f64; 5];
    for level in 1..=5 {
        let p = params_of_level(level);
        for seed in 1..=15u32 {
            beta[level - 1] += metrics_of(&stage(seed, &p, 8).d).beta as f64 / 15.0;
        }
    }
    assert!(
        beta[4] > beta[0] + 2.0,
        "level 5 should have more cycles than level 1: {beta:?}"
    );
}

#[test]
fn the_raw_generator_is_valid_most_of_the_time() {
    let p = params_of_level(3);
    let bad = (1..=20u32)
        .filter(|s| !validate(&generate(*s, &p)).is_empty())
        .count();
    assert!(bad <= 4, "{bad} of 20 raw draws invalid");
}

/// A timing to compare with the TypeScript prototype (run with `cargo test --release -p dungeon-lab-core bench -- --nocapture`).
#[test]
fn bench() {
    for level in 1..=5 {
        let p = params_of_level(level);
        let n = 25u32;
        let t0 = std::time::Instant::now();
        let mut bad = 0;
        let (mut beta, mut junc, mut leaf) = (0.0, 0.0, 0.0);
        for seed in 1..=n {
            let s = stage(seed, &p, 8);
            if !s.violations.is_empty() {
                bad += 1;
            }
            let m = metrics_of(&s.d);
            beta += m.beta as f64;
            junc += m.junctions as f64;
            leaf += m.leaf_chambers as f64;
        }
        println!(
            "L{level}: invalid {bad}/{n} β {:.2} junc {:.2} leafCh {:.2} ms/stage {:.1}",
            beta / n as f64,
            junc / n as f64,
            leaf / n as f64,
            t0.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
    }
}

/// Seed 0xA030 on a 10×8 grid picks a start room no corridor can leave; the generator used to panic, now the stage redraws.
#[test]
fn an_isolated_start_room_is_redrawn_not_a_panic() {
    let mut p = params_of_level(5);
    p.cols = 10;
    p.rows = 8;
    p.layout_jitter = 0.15;
    let s = stage(0xA000 + 48, &p, 8);
    assert!(s.violations.is_empty(), "{:?}", s.violations);
    assert!(s.draws > 1);
}
