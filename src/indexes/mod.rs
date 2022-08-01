mod naive;
mod roaring;
mod sqlite;

use std::borrow::Cow;

pub use self::roaring::Roaring;
pub use naive::Naive;
pub use sqlite::SQLite;

use crate::{DocId, Document, Query};

pub trait Index: Send + Sync + Default {
    /// Get all the documents in the index
    fn get_documents(&self) -> Vec<Cow<Document>>;

    /// Get one document in the index
    fn get_document(&self, id: DocId) -> Option<Cow<Document>>;

    /// Add a batch of documents
    fn add_documents(&mut self, document: Vec<Document>);

    /// Add a batch of documents
    fn delete_documents(&mut self, documents: Vec<DocId>);

    /// Execute a search
    fn search(&self, query: &Query) -> Vec<Cow<Document>>;

    /// clear the on disk database
    fn clear_database();
}
