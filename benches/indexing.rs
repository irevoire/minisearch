use criterion::{criterion_group, criterion_main, Bencher, Criterion};

use minisearch::{indexes, Document, Index};

pub fn indexing(c: &mut Criterion) {
    let dataset = std::include_bytes!("../datasets/movies.json");
    let dataset: Vec<Document> = serde_json::from_reader(dataset.as_ref()).unwrap();

    let mut g = c.benchmark_group("Indexing");
    g.sample_size(10); // since indexing is so slow we're only going to run 10 iterations

    // we can't bench the sqlite implementation because it takes ~10 minutes
    g.bench_function("naive", |g| bench_index::<indexes::Naive>(g, &dataset));
    g.bench_function("roaring", |g| bench_index::<indexes::Roaring>(g, &dataset));
    g.bench_function("sled", |g| bench_index::<indexes::Sled>(g, &dataset));
    g.bench_function("heed", |g| bench_index::<indexes::Heed>(g, &dataset));
}

fn bench_index<I: Index>(bencher: &mut Bencher, dataset: &[Document]) {
    bencher.iter_with_setup(
        || {
            I::clear_database();
            (I::default(), dataset.to_vec())
        },
        |(mut index, dataset)| index.add_documents(dataset),
    )
}

criterion_group!(benches, indexing);
criterion_main!(benches);
