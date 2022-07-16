mod api;
mod index;

pub use api::{Document, Query};
pub use index::Index;

#[tokio::main]
async fn main() {
    api::run().await;
}
