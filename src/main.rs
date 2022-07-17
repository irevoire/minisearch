mod api;
mod tokenizer;

pub use api::{Document, Query};
pub use tokenizer::tokenize;

mod indexes;

use crate::indexes::Index;

use std::sync::Arc;

use indexes::naive_indexer::NaiveIndexer;
use tokio::sync::Mutex;

type DocId = u32;
type Indexer = Arc<Mutex<dyn Index>>;

fn get_indexer() -> Indexer {
    Arc::new(Mutex::new(NaiveIndexer::default()))
}

#[tokio::main]
async fn main() {
    let index = get_indexer();
    println!("Starting http server on `http://localhost:3000`");

    api::run(index).await;
}
