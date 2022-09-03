use big_s::S;
use criterion::{criterion_group, criterion_main, Criterion};

use minisearch::{indexes, Document, Index, Query};

pub fn search(c: &mut Criterion) {
    let dataset = std::include_bytes!("../datasets/movies.json");
    let dataset: Vec<Document> = serde_json::from_reader(dataset.as_ref()).unwrap();
    indexes::Naive::clear_database();
    let mut naive = indexes::Naive::default();
    naive.add_documents(dataset.clone());

    indexes::Roaring::clear_database();
    let mut roaring = indexes::Roaring::default();
    roaring.add_documents(dataset.clone());

    indexes::Sled::clear_database();
    let mut sled = indexes::Sled::default();
    sled.add_documents(dataset);

    #[rustfmt::skip]
    let requests = [
        // 10
        ("No merge - small", Query { q: S("Hello") }),
        // 100
        ("No merge - medium", Query { q: S("tour") }),
        // 1000
        ("No merge - large", Query { q: S("documentary") }),
        // 10_000
        ("No merge - extra_large", Query { q: S("and") }),
        // 10
        ("Merge - small", Query { q: S("Hello lol") }),
        // 100
        ("Merge - medium", Query { q: S("color red") }),
        // 1000
        ("Merge - large", Query { q: S("Hello lol") }),
        // 10_000
        ("Merge - extra_large", Query { q: S("bob and his dog") },
        ),
    ];

    for (name, query) in requests {
        let query = query.clone();
        let mut g = c.benchmark_group(format!("Search: {}", name));
        g.bench_function("naive", |g| g.iter(|| naive.search(&query)));
        g.bench_function("roaring", |g| g.iter(|| roaring.search(&query)));
        g.bench_function("sled", |g| g.iter(|| sled.search(&query)));
    }
}

criterion_group!(benches, search);
criterion_main!(benches);
