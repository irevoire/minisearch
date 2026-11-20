mod fiole;
mod heed;
mod naive;
mod redb;
mod roaring;
mod sled;
mod sqlite;

use std::{borrow::Cow, io::ErrorKind, path::Path};

pub use self::fiole::Fiole;
pub use self::heed::Heed;
pub use self::redb::Redb;
pub use self::roaring::Roaring;
pub use self::sled::Sled;
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
    fn search(&self, query: &Query) -> Vec<DocId>;

    /// The path of the on disk database, either a file or a directory
    fn database_path() -> &'static str;

    /// clear the on disk database
    fn clear_database();
}

/// The size in bytes the database of an index takes on disk, 0 if it doesn't exist.
pub fn database_size<I: Index>() -> u64 {
    fn size(path: &Path) -> u64 {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == ErrorKind::NotFound => return 0,
            Err(e) => panic!("{e}"),
        };
        if metadata.is_dir() {
            std::fs::read_dir(path)
                .unwrap()
                .map(|entry| size(&entry.unwrap().path()))
                .sum()
        } else {
            metadata.len()
        }
    }
    size(Path::new(I::database_path()))
}
