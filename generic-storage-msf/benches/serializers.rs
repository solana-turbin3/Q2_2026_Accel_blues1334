use borsh::{BorshDeserialize, BorshSerialize};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use generic_storage_msf::{Borsh, Json, Serializer, Wincode};
use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

#[derive(
    Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, SchemaWrite, SchemaRead,
)]
struct Person {
    name: String,
    age: u32,
    tags: Vec<String>,
}

fn sample() -> Person {
    Person {
        name: "Marcelo".to_string(),
        age: 40,
        tags: (0..16).map(|i| format!("tag-{i}")).collect(),
    }
}

fn bench_serialize(c: &mut Criterion) {
    let value = sample();
    let mut group = c.benchmark_group("serialize");
    group.bench_function("borsh", |b| {
        b.iter(|| Borsh.to_bytes(black_box(&value)).unwrap())
    });
    group.bench_function("wincode", |b| {
        b.iter(|| Wincode.to_bytes(black_box(&value)).unwrap())
    });
    group.bench_function("json", |b| {
        b.iter(|| Json.to_bytes(black_box(&value)).unwrap())
    });
    group.finish();
}

fn bench_deserialize(c: &mut Criterion) {
    let value = sample();
    let borsh_bytes = Borsh.to_bytes(&value).unwrap();
    let wincode_bytes = Wincode.to_bytes(&value).unwrap();
    let json_bytes = Json.to_bytes(&value).unwrap();

    let mut group = c.benchmark_group("deserialize");
    group.bench_function("borsh", |b| {
        b.iter(|| {
            let _: Person = Borsh.from_bytes(black_box(&borsh_bytes)).unwrap();
        })
    });
    group.bench_function("wincode", |b| {
        b.iter(|| {
            let _: Person = Wincode.from_bytes(black_box(&wincode_bytes)).unwrap();
        })
    });
    group.bench_function("json", |b| {
        b.iter(|| {
            let _: Person = Json.from_bytes(black_box(&json_bytes)).unwrap();
        })
    });
    group.finish();
}

criterion_group!(benches, bench_serialize, bench_deserialize);
criterion_main!(benches);
