pub(crate) mod naive_indexer;

use crate::{Document, Query};

pub trait Index: Send + Sync {
    fn get_document(&self, id: usize) -> Option<Document>;

    fn add(&mut self, document: Document);

    fn search(&self, query: Query) -> Vec<Document>;
}
