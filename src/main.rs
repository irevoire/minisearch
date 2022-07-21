use minisearch::indexes;

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut args = std::env::args();
    if args.len() > 2 {
        eprintln!("Usage:\n\t{} [engine]", args.nth(0).unwrap());
        return;
    }
    match args.nth(1).as_deref() {
        None | Some("naive") => minisearch::run(indexes::Naive::default()).await,
        Some("roaring") => minisearch::run(indexes::Roaring::default()).await,
        Some(engine) => {
            eprintln!("Unknown engine {engine}. Available engine are `naive` and `roaring`.");
        }
    }
}
