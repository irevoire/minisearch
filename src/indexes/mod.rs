pub(crate) mod naive_indexer;

use crate::{DocId, Document, Query};

pub trait Index: Send + Sync {
    /// Get all the documents in the index
    fn get_documents(&self) -> Vec<Document>;

    /// Get one document in the index
    fn get_document(&self, id: DocId) -> Option<Document>;

    /// Add a batch of documents
    fn add_documents(&mut self, document: Vec<Document>);

    /// Add a batch of documents
    fn delete_documents(&mut self, documents: Vec<DocId>);

    /// Execute a search
    fn search(&self, query: Query) -> Vec<Document>;
}
