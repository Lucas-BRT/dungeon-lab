//! Stopping a generation: `Cancel` makes the cancellable entry points give up with `None`, quickly even on a huge floor, and a token
//! nobody uses changes nothing.

use std::time::{Duration, Instant};
use tapiti_core::cancel::Cancel;
use tapiti_core::generate::{generate, generate_cancellable};
use tapiti_core::params::{params_of_size, rich_of_size, Layout, Params, Size};
use tapiti_core::stage::{stage, stage_cancellable};

fn without_clock(floor: &tapiti_core::generate::Floor) -> serde_json::Value {
    let mut v = serde_json::to_value(floor).unwrap();
    v.as_object_mut().unwrap().remove("phases");
    v
}

#[test]
fn a_token_nobody_calls_changes_nothing() {
    for layout in [Layout::Grid, Layout::Growth] {
        let mut p = rich_of_size(Size::Medium);
        p.layout = layout;
        let cancel = Cancel::new();
        let floor = generate_cancellable(7, &p, &cancel, &mut |_| {}).unwrap();
        assert_eq!(without_clock(&floor), without_clock(&generate(7, &p)));
        let staged = stage_cancellable(7, &p, 8, &cancel, &mut |_| {}).unwrap();
        assert_eq!(staged.draws, stage(7, &p, 8).draws);
    }
}

#[test]
fn a_cancelled_token_gives_nothing() {
    let p = params_of_size(Size::Medium);
    let cancel = Cancel::new();
    cancel.cancel();
    assert!(generate_cancellable(1, &p, &cancel, &mut |_| {}).is_none());
    assert!(stage_cancellable(1, &p, 8, &cancel, &mut |_| {}).is_none());
}

/// A grid of 64 by 64 blocks takes seconds; cancelling after a moment ends it long before.
fn huge(layout: Layout) -> Params {
    let mut p = params_of_size(Size::Huge);
    p.cols = 64;
    p.rows = 64;
    p.leaves = 100;
    p.layout = layout;
    p
}

#[test]
fn a_huge_floor_stops_soon_after_the_cancel() {
    for layout in [Layout::Grid, Layout::Growth] {
        let p = huge(layout);
        let cancel = Cancel::new();
        let stopper = cancel.clone();
        let started = Instant::now();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            stopper.cancel();
        });
        let result = stage_cancellable(1, &p, 8, &cancel, &mut |_| {});
        handle.join().unwrap();
        let took = started.elapsed();
        assert!(
            result.is_none(),
            "{layout:?}: the floor was finished in {took:?}"
        );
        assert!(
            took < Duration::from_secs(5),
            "{layout:?}: stopped after {took:?}"
        );
    }
}
