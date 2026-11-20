use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    io::ErrorKind,
};

use redb::{Database, ReadableDatabase, ReadableTable, Table, TableDefinition};
use roaring::RoaringBitmap;

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "redb.db";

/// The documents are stored as json.
const DOCUMENTS: TableDefinition<u32, &[u8]> = TableDefinition::new("documents");
/// The bitmaps are stored in the roaring portable format.
const WORDS: TableDefinition<&str, &[u8]> = TableDefinition::new("words");

type Documents<'txn> = Table<'txn, u32, &'static [u8]>;
type Words<'txn> = Table<'txn, &'static str, &'static [u8]>;

pub struct Redb {
    db: Database,
}

impl Redb {
    fn add_document(
        documents: &mut Documents,
        words: &mut Words,
        document: Document,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        let docid = document.docid();

        // first we delete the old version of the document
        Self::delete_document(documents, words, docid, dirty_words);

        let mut doc_words: Vec<_> = document.fields().flat_map(tokenize).collect();
        // if a word is present multiple times in the same field we only count it once
        doc_words.sort_unstable();
        doc_words.dedup();

        for word in doc_words.into_iter().filter(|w| !w.is_empty()) {
            match dirty_words.entry(word) {
                Entry::Occupied(mut bitmap) => {
                    bitmap.get_mut().insert(docid);
                }
                Entry::Vacant(entry) => {
                    // We get the current value from the db
                    let mut bitmap = match words.get(entry.key().as_str()).unwrap() {
                        Some(bytes) => RoaringBitmap::deserialize_from(bytes.value()).unwrap(),
                        None => RoaringBitmap::new(),
                    };
                    bitmap.insert(docid);
                    entry.insert(bitmap);
                }
            };
        }

        let document = serde_json::to_vec(&document).unwrap();
        documents.insert(docid, document.as_slice()).unwrap();
    }

    fn delete_document(
        documents: &mut Documents,
        words: &mut Words,
        docid: DocId,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        let document = documents.remove(docid).unwrap();
        if let Some(document) = document {
            let document: Document =
                serde_json::from_slice(document.value()).expect("Can't parse document");
            // we get all the words in a document and then get rids of our id for each of these words
            let mut doc_words: Vec<_> = document.fields().flat_map(tokenize).collect();
            // if a word is present multiple times in the same field we only count it once
            doc_words.sort_unstable();
            doc_words.dedup();

            for word in doc_words.into_iter().filter(|w| !w.is_empty()) {
                match dirty_words.entry(word) {
                    Entry::Occupied(mut bitmap) => {
                        bitmap.get_mut().remove(docid);
                    }
                    Entry::Vacant(entry) => {
                        // We get the current value from the db
                        let bytes = words.get(entry.key().as_str()).unwrap().unwrap();
                        let mut bitmap = RoaringBitmap::deserialize_from(bytes.value()).unwrap();
                        bitmap.remove(docid);
                        entry.insert(bitmap);
                    }
                }
            }
        }
    }

    /// Update all the entry in the dirty words.
    fn apply_dirty_words(words: &mut Words, dirty_words: &mut HashMap<String, RoaringBitmap>) {
        // we reuse the same allocation for all the words
        let mut buffer = Vec::new();

        for (word, bitmap) in dirty_words.iter() {
            bitmap.serialize_into(&mut buffer).unwrap();
            words.insert(word.as_str(), buffer.as_slice()).unwrap();
            buffer.clear();
        }
    }

    /// Open the tables in a single write transaction, run `f` on them and commit.
    fn write(&self, f: impl FnOnce(&mut Documents, &mut Words)) {
        let wtxn = self.db.begin_write().unwrap();
        {
            let mut documents = wtxn.open_table(DOCUMENTS).unwrap();
            let mut words = wtxn.open_table(WORDS).unwrap();
            f(&mut documents, &mut words);
        }
        wtxn.commit().unwrap();
    }
}

impl Default for Redb {
    fn default() -> Self {
        let db = Database::create(DB_NAME).unwrap();
        let this = Self { db };
        // the tables must exist before we can open them in a read transaction
        this.write(|_, _| ());
        this
    }
}

impl Index for Redb {
    fn get_documents(&self) -> Vec<Cow<Document>> {
        let rtxn = self.db.begin_read().unwrap();
        let documents = rtxn.open_table(DOCUMENTS).unwrap();
        documents
            .iter()
            .unwrap()
            .map(|entry| Cow::Owned(serde_json::from_slice(entry.unwrap().1.value()).unwrap()))
            .collect()
    }

    fn get_document(&self, id: DocId) -> Option<Cow<Document>> {
        let rtxn = self.db.begin_read().unwrap();
        let documents = rtxn.open_table(DOCUMENTS).unwrap();
        let document = documents.get(id).unwrap();
        document.map(|doc| Cow::Owned(serde_json::from_slice(doc.value()).unwrap()))
    }

    /// Since deser+ser the bitmaps for each insertion is slow we're going to create a
    /// temporary map of the words databases containing only the modified words.
    /// Then we reinsert + serialize all the modified words at once.
    fn add_documents(&mut self, documents: Vec<Document>) {
        self.write(|db_documents, words| {
            let mut dirty_words = HashMap::new();
            for document in documents {
                Self::add_document(db_documents, words, document, &mut dirty_words);
            }
            Self::apply_dirty_words(words, &mut dirty_words);
        });
    }

    fn delete_documents(&mut self, docids: Vec<DocId>) {
        self.write(|documents, words| {
            let mut dirty_words = HashMap::new();
            for docid in docids {
                Self::delete_document(documents, words, docid, &mut dirty_words);
            }
            Self::apply_dirty_words(words, &mut dirty_words);
        });
    }

    fn search(&self, query: &Query) -> Vec<DocId> {
        let rtxn = self.db.begin_read().unwrap();
        let words = rtxn.open_table(WORDS).unwrap();
        let docids = tokenize(query.q.as_deref().unwrap_or(""))
            .filter_map(|word| {
                let bytes = words.get(word.as_str()).unwrap()?;
                Some(RoaringBitmap::deserialize_from(bytes.value()).unwrap())
            })
            .fold(RoaringBitmap::default(), |acc, bitmap| acc | bitmap);

        docids.into_iter().collect()
    }

    fn clear_database() {
        match std::fs::remove_file(DB_NAME) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => panic!("{e}"),
        }
    }
}
