use criterion::{criterion_group, criterion_main, Criterion};

use minisearch::{indexes, Document, Index};

pub fn indexing(c: &mut Criterion) {
    let dataset = std::include_bytes!("../datasets/movies.json");
    let dataset: Vec<Document> = serde_json::from_reader(dataset.as_ref()).unwrap();

    let mut g = c.benchmark_group("Indexing");
    g.sample_size(10); // since indexing is so slow we're only going to run 10 iterations
    g.bench_function("naive", |g| {
        g.iter_with_setup(
            || {
                indexes::Naive::clear_database();
                (indexes::Naive::default(), dataset.clone())
            },
            |(mut index, dataset)| index.add_documents(dataset),
        )
    });
}

criterion_group!(benches, indexing);
criterion_main!(benches);
