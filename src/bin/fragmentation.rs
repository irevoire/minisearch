//! Measure how the backends degrade over time by fragmenting their database.
//!
//! We index 70% of the dataset, then, for every cycle, delete 10% of the dataset at random from the
//! index and add back 10% of the documents that aren't indexed.

use std::{
    fmt::Write as _,
    path::PathBuf,
    time::{Duration, Instant},
};

use clap::{Parser, ValueEnum};
use minisearch::{
    indexes::{self, database_size},
    Document, Index,
};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};

#[derive(Parser)]
#[command(about = "Repeatedly delete and add documents to measure the fragmentation of the databases")]
struct Opt {
    /// The engines to benchmark, by default heed, sled, fiole and redb.
    #[arg(value_enum)]
    engines: Vec<Engine>,

    /// The number of delete then add cycles to run.
    #[arg(long, default_value_t = 100)]
    cycles: usize,

    /// The seed used to shuffle the documents, the same for every engine.
    #[arg(long, default_value_t = 36)]
    seed: u64,

    /// The dataset to index, a json array of documents.
    #[arg(long, default_value = "datasets/movies.json")]
    dataset: PathBuf,

    /// Where to write the gnuplot data file.
    #[arg(long, default_value = "fragmentation.dat")]
    output: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Engine {
    Heed,
    Sled,
    Fiole,
    Redb,
    Naive,
    Roaring,
    Sqlite,
}

impl Engine {
    /// The engines that are run when none are specified, naive, roaring and sqlite are in a
    /// different category.
    const DEFAULT: [Engine; 4] = [Engine::Heed, Engine::Sled, Engine::Fiole, Engine::Redb];

    fn name(self) -> &'static str {
        match self {
            Engine::Heed => "heed",
            Engine::Sled => "sled",
            Engine::Fiole => "fiole",
            Engine::Redb => "redb",
            Engine::Naive => "naive",
            Engine::Roaring => "roaring",
            Engine::Sqlite => "sqlite",
        }
    }

    fn run(self, dataset: &[Document], opt: &Opt) -> Vec<Sample> {
        match self {
            Engine::Heed => run::<indexes::Heed>(self, dataset, opt),
            Engine::Sled => run::<indexes::Sled>(self, dataset, opt),
            Engine::Fiole => run::<indexes::Fiole>(self, dataset, opt),
            Engine::Redb => run::<indexes::Redb>(self, dataset, opt),
            Engine::Naive => run::<indexes::Naive>(self, dataset, opt),
            Engine::Roaring => run::<indexes::Roaring>(self, dataset, opt),
            Engine::Sqlite => run::<indexes::SQLite>(self, dataset, opt),
        }
    }
}

/// What we measured during one cycle, the cycle 0 is the initial indexing.
struct Sample {
    delete: Duration,
    add: Duration,
    /// The size of the database on disk at the end of the cycle, in bytes.
    size: u64,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut opt = Opt::parse();
    if opt.engines.is_empty() {
        opt.engines = Engine::DEFAULT.to_vec();
    }
    // running the same engine twice would overwrite its own database and columns
    let mut engines = Vec::new();
    for engine in &opt.engines {
        if !engines.contains(engine) {
            engines.push(*engine);
        }
    }

    let file = std::fs::File::open(&opt.dataset).unwrap();
    let mut dataset: Vec<Document> = serde_json::from_reader(std::io::BufReader::new(file)).unwrap();
    // a document sharing its id with another one would be deleted along with it, and we would lose
    // track of what's indexed
    let mut ids = std::collections::HashSet::new();
    dataset.retain(|document| ids.insert(document.docid()));
    log::info!("Loaded {} documents from {}", dataset.len(), opt.dataset.display());

    let results: Vec<_> = engines
        .iter()
        .map(|engine| (*engine, engine.run(&dataset, &opt)))
        .collect();

    std::fs::write(&opt.output, gnuplot(&results)).unwrap();
    log::info!("Results written to {}", opt.output.display());
}

fn run<I: Index>(engine: Engine, dataset: &[Document], opt: &Opt) -> Vec<Sample> {
    let name = engine.name();
    // every engine gets its own rng so they all see exactly the same sequence of operations
    let mut rng = StdRng::seed_from_u64(opt.seed);
    let mut indexed = dataset.to_vec();
    indexed.shuffle(&mut rng);
    let mut pool = indexed.split_off(indexed.len() * 7 / 10);
    let chunk = dataset.len() / 10;

    I::clear_database();
    let mut index = I::default();
    let mut samples = Vec::with_capacity(opt.cycles + 1);

    log::info!("[{name}] indexing {} documents", indexed.len());
    let now = Instant::now();
    index.add_documents(indexed.clone());
    let add = now.elapsed();
    let size = database_size::<I>();
    log::info!("[{name}] initial indexing took {add:.2?}, database size {}", mb(size));
    samples.push(Sample { delete: Duration::ZERO, add, size });

    for cycle in 1..=opt.cycles {
        indexed.shuffle(&mut rng);
        let removed = indexed.split_off(indexed.len() - chunk);
        let ids = removed.iter().map(Document::docid).collect();
        let now = Instant::now();
        index.delete_documents(ids);
        let delete = now.elapsed();

        // we pick the documents to add before putting back the removed ones in the pool, that way
        // we never re-index the documents we just deleted
        pool.shuffle(&mut rng);
        let added = pool.split_off(pool.len() - chunk);
        let now = Instant::now();
        index.add_documents(added.clone());
        let add = now.elapsed();
        indexed.extend(added);
        pool.extend(removed);

        let size = database_size::<I>();
        log::info!(
            "[{name}] cycle {cycle}/{}: delete {delete:.2?}, add {add:.2?}, database size {}",
            opt.cycles,
            mb(size)
        );
        samples.push(Sample { delete, add, size });
    }

    assert_eq!(index.get_documents().len(), indexed.len());
    summary(name, &samples);
    samples
}

fn summary(name: &str, samples: &[Sample]) {
    let (initial, cycles) = samples.split_first().unwrap();
    log::info!("[{name}] initial indexing {:.2?}", initial.add);
    if !cycles.is_empty() {
        let delete: Vec<_> = cycles.iter().map(|s| s.delete).collect();
        let add: Vec<_> = cycles.iter().map(|s| s.add).collect();
        for (step, durations) in [("delete", delete), ("add", add)] {
            let min = durations.iter().min().unwrap();
            let max = durations.iter().max().unwrap();
            let mean = durations.iter().sum::<Duration>() / durations.len() as u32;
            let first = durations.first().unwrap();
            let last = durations.last().unwrap();
            log::info!(
                "[{name}] {step}: min {min:.2?}, mean {mean:.2?}, max {max:.2?}, first {first:.2?}, last {last:.2?}"
            );
        }
    }
    let last = samples.last().unwrap();
    log::info!("[{name}] database size {} -> {}", mb(initial.size), mb(last.size));
}

/// A whitespace separated table with one row per cycle and three columns per engine.
fn gnuplot(results: &[(Engine, Vec<Sample>)]) -> String {
    let mut out = String::from("cycle");
    for (engine, _) in results {
        let name = engine.name();
        write!(out, " {name}_delete_ms {name}_add_ms {name}_size_mb").unwrap();
    }
    out.push('\n');

    let cycles = results.first().map_or(0, |(_, samples)| samples.len());
    for cycle in 0..cycles {
        write!(out, "{cycle}").unwrap();
        for (_, samples) in results {
            let sample = &samples[cycle];
            write!(
                out,
                " {:.3} {:.3} {:.3}",
                sample.delete.as_secs_f64() * 1000.,
                sample.add.as_secs_f64() * 1000.,
                sample.size as f64 / 1_000_000.,
            )
            .unwrap();
        }
        out.push('\n');
    }
    out
}

fn mb(bytes: u64) -> String {
    format!("{:.2}MB", bytes as f64 / 1_000_000.)
}
