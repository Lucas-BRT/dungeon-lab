use dungeon_lab::generate::generate;
use dungeon_lab::metrics::metrics_of;
use dungeon_lab::params::params_of_level;
use dungeon_lab::stage::stage;
use dungeon_lab::validate::validate;

/// The model's promise (`docs/dungeon-model.md` §9): every level's stages pass the validator.
#[test]
fn every_level_gives_only_valid_stages() {
    for level in 1..=5 {
        let p = params_of_level(level);
        for seed in [101u32, 202, 303, 404, 505] {
            let s = stage(seed, &p, 8);
            assert!(s.violations.is_empty(), "level {level} seed {seed}: {:?}", s.violations);
        }
    }
}

#[test]
fn the_same_seed_and_lambda_give_the_same_stage() {
    let p = params_of_level(4);
    let a = serde_json::to_string(&generate(7, &p)).unwrap();
    let b = serde_json::to_string(&generate(7, &p)).unwrap();
    assert_eq!(a, b);
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
    assert!(beta[4] > beta[0] + 2.0, "level 5 should have more cycles than level 1: {beta:?}");
}

#[test]
fn the_raw_generator_is_valid_most_of_the_time() {
    let p = params_of_level(3);
    let bad = (1..=20u32).filter(|s| !validate(&generate(*s, &p)).is_empty()).count();
    assert!(bad <= 4, "{bad} of 20 raw draws invalid");
}

/// A timing to compare with the TypeScript prototype (run with `cargo test --release bench -- --nocapture`).
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
        println!("L{level}: invalid {bad}/{n} β {:.2} junc {:.2} leafCh {:.2} ms/stage {:.1}", beta / n as f64, junc / n as f64, leaf / n as f64, t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }
}
