use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use tapiti_core::{
    generate, metrics,
    params::{self, Size, TopologyProfile},
    stage, validate,
};

fn generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("generate");
    group.throughput(Throughput::Elements(1));
    for size in Size::ALL {
        let p = params::rich_of_size(size);
        group.bench_with_input(BenchmarkId::new("size", size.name()), &p, |b, p| {
            b.iter(|| generate::generate(black_box(0xD06E_0000 + size as u32), black_box(p)))
        });
    }

    let mut extended = params::rich_of_size(Size::Medium);
    extended.layout_jitter = 0.2;
    extended.rotate_rooms = true;
    extended.p_w = 0.5;
    extended.max_bends = 6;
    group.bench_function("medium-extended-routes", |b| {
        b.iter(|| generate::generate(black_box(0xD06E_0005), black_box(&extended)))
    });

    let mut backbone = params::rich_of_size(Size::Medium);
    backbone.topology.profile = TopologyProfile::Backbone;
    backbone.topology.min_branch_depth = 2;
    group.bench_function("medium-backbone", |b| {
        b.iter(|| generate::generate(black_box(0xD06E_0006), black_box(&backbone)))
    });
    group.finish();
}

fn complete_stage(c: &mut Criterion) {
    let mut group = c.benchmark_group("stage_with_validation");
    group.throughput(Throughput::Elements(1));
    for size in [Size::Small, Size::Medium, Size::Large] {
        let p = params::rich_of_size(size);
        group.bench_with_input(BenchmarkId::new("size", size.name()), &p, |b, p| {
            b.iter(|| stage::stage(black_box(0x51A6_0000 + size as u32), black_box(p), 8))
        });
    }
    group.finish();
}

fn validation_and_metrics(c: &mut Criterion) {
    let p = params::rich_of_size(Size::Medium);
    let floor = generate::generate(0x0C01_1EC7, &p);

    c.bench_function("validate/medium", |b| {
        b.iter(|| validate::validate(black_box(&floor)))
    });
    c.bench_function("metrics/medium", |b| {
        b.iter(|| metrics::metrics_of(black_box(&floor)))
    });
}

criterion_group!(benches, generation, complete_stage, validation_and_metrics);
criterion_main!(benches);
