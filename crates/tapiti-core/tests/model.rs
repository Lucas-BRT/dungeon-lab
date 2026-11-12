use tapiti_core::generate::generate;
use tapiti_core::metrics::metrics_of;
use tapiti_core::params::{params_of_size, rich_of_size, Size};
use tapiti_core::stage::stage;
use tapiti_core::validate::validate;

/// The model's promise (`docs/model.md` §9): every size's stages pass the validator, with the defaults and with every option on.
#[test]
fn every_size_gives_only_valid_stages() {
    for size in Size::ALL {
        for p in [params_of_size(size), rich_of_size(size)] {
            for seed in [101u32, 202, 303, 404, 505] {
                let s = stage(seed, &p, 8);
                assert!(
                    s.violations.is_empty(),
                    "{} seed {seed}: {:?}",
                    size.name(),
                    s.violations
                );
            }
        }
    }
}

/// The stage, without the clock readings of `phases` (the only thing that differs between two runs).
fn without_clock(seed: u32, size: Size) -> serde_json::Value {
    let mut v = serde_json::to_value(generate(seed, &rich_of_size(size))).unwrap();
    v.as_object_mut().unwrap().remove("phases");
    v
}

#[test]
fn the_same_seed_and_lambda_give_the_same_stage() {
    for size in [Size::Small, Size::Medium] {
        assert_eq!(without_clock(7, size), without_clock(7, size));
    }
}

#[test]
fn density_follows_delta() {
    let mut beta = [0.0f64; 2];
    for (i, delta) in [0.15, 0.55].into_iter().enumerate() {
        let mut p = rich_of_size(Size::Medium);
        p.delta = delta;
        for seed in 1..=15u32 {
            beta[i] += metrics_of(&stage(seed, &p, 8).d).beta as f64 / 15.0;
        }
    }
    assert!(
        beta[1] > beta[0] + 2.0,
        "delta 0.55 should have more cycles than delta 0.15: {beta:?}"
    );
}

#[test]
fn the_raw_generator_is_valid_most_of_the_time() {
    let p = rich_of_size(Size::Medium);
    let bad = (1..=20u32)
        .filter(|s| !validate(&generate(*s, &p)).is_empty())
        .count();
    assert!(bad <= 4, "{bad} of 20 raw draws invalid");
}

/// A timing of the generator (run with `cargo test --release -p tapiti-core bench -- --nocapture`).
#[test]
fn bench() {
    for size in Size::ALL {
        let p = rich_of_size(size);
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
            "{}: invalid {bad}/{n} β {:.2} junc {:.2} leafCh {:.2} ms/stage {:.1}",
            size.name(),
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
    let mut p = rich_of_size(Size::Medium);
    p.cols = 10;
    p.rows = 8;
    p.layout_jitter = 0.15;
    let s = stage(0xA000 + 48, &p, 8);
    assert!(s.violations.is_empty(), "{:?}", s.violations);
    assert!(s.draws > 1);
}

/// A class of weight 0 never appears, even when the caps are too tight for the floor (30x30 rooms against caps of 99 per class).
#[test]
fn zero_weight_classes_never_appear_when_caps_are_exceeded() {
    use tapiti_core::params::{ByClass, ShapeClass};
    let mut p = params_of_size(Size::Small);
    p.cols = 30;
    p.rows = 30;
    p.max_of = ByClass {
        rect: 99.0,
        disc: 99.0,
        oct: 99.0,
        l: 99.0,
        star: 99.0,
    };
    let f = generate(146343314, &p);
    assert!(f.rooms.iter().all(|r| r.cls == ShapeClass::Rect));
}

/// With the default quotas a big grid is not capped: the small size stays all rectangles on 30x30 with no cap warning.
#[test]
fn default_quotas_do_not_bind_on_a_big_grid() {
    let mut p = params_of_size(Size::Small);
    p.cols = 30;
    p.rows = 30;
    let f = generate(146343314, &p);
    assert!(!f.log.iter().any(|m| m.contains("caps cannot fit")));
}

/// `layout` picks the room layout in the Λ's JSON: absent means the grid, `growth` grows the rooms.
#[test]
fn the_layout_field_picks_the_room_layout() {
    use tapiti_core::params::{Layout, Params};
    let mut value = serde_json::to_value(params_of_size(Size::Small)).unwrap();
    value.as_object_mut().unwrap().remove("layout");
    let p: Params = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(p.layout, Layout::Grid);

    value["layout"] = "growth".into();
    value["growth"]["rooms"] = 30.into();
    let grown: Params = serde_json::from_value(value).unwrap();
    assert_eq!(grown.layout, Layout::Growth);
    let f = generate(7, &grown);
    assert!(f.rooms.iter().filter(|r| r.alive).count() > 1);
    assert_ne!(
        serde_json::to_string(&f.rooms).unwrap(),
        serde_json::to_string(&generate(7, &p).rooms).unwrap()
    );
}

/// The entrance and the exit are optional, in both layouts: without them a stage has no such room, no main path, and is still valid.
#[test]
fn the_entrance_and_the_exit_can_be_left_out() {
    use tapiti_core::generate::Role;
    use tapiti_core::params::Layout;
    for layout in [Layout::Grid, Layout::Growth] {
        for (entrance, exit_room) in [(true, true), (true, false), (false, true), (false, false)] {
            let mut p = rich_of_size(Size::Medium);
            p.layout = layout;
            p.entrance = entrance;
            p.exit_room = exit_room;
            for seed in [1u32, 2, 3, 4] {
                let s = stage(seed, &p, 8);
                let tag = format!("{layout:?} entrance {entrance} exit {exit_room} seed {seed}");
                assert!(s.violations.is_empty(), "{tag}: {:?}", s.violations);
                let has = |role: Role| s.d.rooms.iter().any(|r| r.role == role);
                assert_eq!(s.d.entrance.is_some(), entrance, "{tag}");
                assert_eq!(s.d.exit.is_some(), entrance && exit_room, "{tag}");
                assert_eq!(has(Role::Entrance), entrance, "{tag}");
                assert_eq!(has(Role::Exit), entrance && exit_room, "{tag}");
                assert_eq!(s.d.main_path.is_empty(), !(entrance && exit_room), "{tag}");
                let m = metrics_of(&s.d);
                assert_eq!(m.routes_to_exit.is_some(), entrance && exit_room, "{tag}");
            }
        }
    }
}
