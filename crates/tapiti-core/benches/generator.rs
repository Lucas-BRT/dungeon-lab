use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tapiti_core::{
    generate, metrics,
    params::{self, TopologyProfile},
    stage, validate,
};
use std::hint::black_box;

fn generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate");
    group.throughput(Throughput::Elements(1));
    for level in 1..=5 {
        let p = params::params_of_level(level);
        group.bench_with_input(BenchmarkId::new("level", level), &p, |b, p| {
            b.iter(|| generate::generate(black_box(0xD06E_0000 + level as u32), black_box(p)))
        });
    }

    let mut extended = params::params_of_level(5);
    extended.layout_jitter = 0.2;
    extended.rotate_rooms = true;
    extended.p_w = 0.5;
    extended.max_bends = 6;
    group.bench_function("level5-extended-routes", |b| {
        b.iter(|| generate::generate(black_box(0xD06E_0005), black_box(&extended)))
    });

    let mut progression = params::params_of_level(5);
    progression.topology.profile = TopologyProfile::Backbone;
    progression.topology.min_branch_depth = 2;
    progression.progression.key_locks = 2;
    progression.progression.switch_locks = 1;
    progression.pacing.enabled = true;
    group.bench_function("level5-progression", |b| {
        b.iter(|| generate::generate(black_box(0xD06E_0006), black_box(&progression)))
    });
    group.finish();
}

fn complete_stage(c: &mut Criterion) {
    let mut group = c.benchmark_group("stage_with_validation");
    group.throughput(Throughput::Elements(1));
    for level in [1, 3, 5] {
        let p = params::params_of_level(level);
        group.bench_with_input(BenchmarkId::new("level", level), &p, |b, p| {
            b.iter(|| stage::stage(black_box(0x51A6_0000 + level as u32), black_box(p), 8))
        });
    }
    group.finish();
}

fn validation_and_metrics(c: &mut Criterion) {
    let p = params::params_of_level(5);
    let dungeon = generate::generate(0xC011_EC7, &p);

    c.bench_function("validate/level5", |b| {
        b.iter(|| validate::validate(black_box(&dungeon)))
    });
    c.bench_function("metrics/level5", |b| {
        b.iter(|| metrics::metrics_of(black_box(&dungeon)))
    });
}

criterion_group!(benches, generation, complete_stage, validation_and_metrics);
criterion_main!(benches);
