pub(crate) mod naive_indexer;

use crate::{Document, Query};

pub trait Index: Send + Sync {
    /// Get all the documents in the index
    fn get_documents(&self) -> Vec<Document>;

    /// Get one document in the index
    fn get_document(&self, id: usize) -> Option<Document>;

    /// Add a batch of documents
    fn add_documents(&mut self, document: Vec<Document>) {
        document
            .into_iter()
            .for_each(|document| self.add_document(document))
    }

    /// Add one document
    fn add_document(&mut self, document: Document);

    fn search(&self, query: Query) -> Vec<Document>;
}
