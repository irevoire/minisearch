mod api;
mod index;
mod tokenizer;

pub use api::{Document, Query};
pub use index::Index;
pub use tokenizer::tokenize;

#[tokio::main]
async fn main() {
    api::run().await;
}
