use minisearch::indexes;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut args = std::env::args();
    if args.len() > 2 {
        eprintln!("Usage:\n\t{} [engine]", args.nth(0).unwrap());
        return;
    }
    match args.nth(1).as_deref() {
        Some("sqlite") => minisearch::run(indexes::SQLite::default()).await,
        Some("naive") => minisearch::run(indexes::Naive::default()).await,
        Some("roaring") => minisearch::run(indexes::Roaring::default()).await,
        Some("sled") | None => minisearch::run(indexes::Sled::default()).await,
        Some(engine) => {
            eprintln!(
                "Unknown engine {engine}. Available engine are `sqlite`, `naive`, `roaring`, `sled`."
            );
        }
    }
}
