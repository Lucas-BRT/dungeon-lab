//! The grown layout (`layout: "growth"`): determinism, validity, how many of the grid's rooms it reaches, leaf chambers and the
//! topology profiles. The thresholds are floors under what the generator reaches today (measured over many seeds); they are raised
//! on purpose when the layout improves, and a test failing means the layout got worse.

use tapiti_core::generate::generate;
use tapiti_core::params::{rich_of_size, Layout, Params, Size, TopologyProfile};
use tapiti_core::stage::stage;

fn grown(size: Size) -> Params {
    let mut p = rich_of_size(size);
    p.layout = Layout::Growth;
    p
}

fn without_clock(seed: u32, p: &Params) -> serde_json::Value {
    let mut v = serde_json::to_value(generate(seed, p)).unwrap();
    v.as_object_mut().unwrap().remove("phases");
    v
}

#[test]
fn a_seed_and_a_lambda_draw_the_same_grown_floor() {
    let mut islands = grown(Size::Medium);
    islands.growth.islands = 3;
    for p in [grown(Size::Small), grown(Size::Medium), islands] {
        assert_eq!(without_clock(7, &p), without_clock(7, &p));
    }
}

/// How many of `seeds` give a valid floor on the first draw, and with eight draws.
fn valid_counts(p: &Params, seeds: u32) -> (u32, u32) {
    let first = (1..=seeds)
        .filter(|s| stage(*s, p, 1).violations.is_empty())
        .count() as u32;
    let eight = (1..=seeds)
        .filter(|s| stage(*s, p, 8).violations.is_empty())
        .count() as u32;
    (first, eight)
}

#[test]
fn grown_floors_are_valid_within_eight_draws() {
    for size in [Size::Medium, Size::Large] {
        let (first, eight) = valid_counts(&grown(size), 20);
        println!(
            "{}: first draw {first}/20, eight draws {eight}/20",
            size.name()
        );
        assert_eq!(eight, 20, "{}", size.name());
    }
}

#[test]
fn grown_floors_are_often_valid_on_the_first_draw() {
    let (first, _) = valid_counts(&grown(Size::Medium), 20);
    assert!(first >= 10, "medium: {first}/20 valid on the first draw");
    let (first, _) = valid_counts(&grown(Size::Large), 20);
    assert!(first >= 17, "large: {first}/20 valid on the first draw");
}

/// Rooms that are not leaf chambers, as a share of the grid layout's, over `seeds` seeds.
fn room_share(size: Size, seeds: u32) -> f64 {
    let count = |p: &Params, seed: u32| {
        generate(seed, p)
            .rooms
            .iter()
            .filter(|r| r.alive && !r.leaf)
            .count() as f64
    };
    let g = grown(size);
    let mut plain = g.clone();
    plain.layout = Layout::Grid;
    let (a, b) = (1..=seeds).fold((0.0, 0.0), |(a, b), s| {
        (a + count(&g, s), b + count(&plain, s))
    });
    a / b
}

#[test]
fn the_grown_layout_reaches_most_of_the_grids_rooms() {
    for (size, floor) in [(Size::Medium, 0.8), (Size::Large, 0.8), (Size::Huge, 0.8)] {
        let share = room_share(size, 10);
        println!("{}: {share:.2} of the grid's rooms", size.name());
        assert!(share >= floor, "{}: {share:.2} < {floor}", size.name());
    }
}

#[test]
fn the_grown_layout_makes_some_leaf_chambers() {
    let p = grown(Size::Huge);
    let (made, wanted) = (1..=8u32).fold((0usize, 0usize), |(m, w), s| {
        let d = generate(s, &p);
        (
            m + d.rooms.iter().filter(|r| r.leaf).count(),
            w + p.leaves as usize,
        )
    });
    println!("huge: {made} leaf chambers of {wanted} wanted");
    assert!(made * 100 >= wanted * 12, "{made} of {wanted}");
}

#[test]
fn the_profiles_that_do_not_depend_on_a_path_hold_on_grown_floors() {
    for profile in [TopologyProfile::Classic, TopologyProfile::Hub] {
        let mut p = grown(Size::Medium);
        p.topology.profile = profile;
        let (_, eight) = valid_counts(&p, 20);
        assert_eq!(eight, 20, "{profile:?}");
    }
}
