use criterion::{black_box, criterion_group, criterion_main, Criterion};
use node_semver_rs::{compare, satisfies, Options};

fn bench_compare(c: &mut Criterion) {
    c.bench_function("compare", |b| {
        b.iter(|| {
            compare(
                black_box("1.2.3-alpha.1"),
                black_box("1.2.3-alpha.2"),
                Options::EMPTY,
            )
        })
    });

    c.bench_function("satisfies", |b| {
        b.iter(|| {
            satisfies(
                black_box("1.2.3"),
                black_box("^1.0.0 || >=2.5.0 || 5.0.0 - 7.2.3"),
                Options::EMPTY,
            )
        })
    });
}

criterion_group!(benches, bench_compare);
criterion_main!(benches);
