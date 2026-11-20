use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    io::ErrorKind,
};

use fiole::{
    codec::{OrderedU32, RoaringBitmapCodec, SerdeJson, Str},
    Database, Keyspace, KeyspaceCreateOptions, Root, Wtxn,
};
use roaring::RoaringBitmap;

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "fiole.db";

/// Keyspace names.
mod ks_name {
    pub const DOCUMENTS: &str = "documents";
    pub const WORDS: &str = "words";
}

pub struct Fiole {
    db: Database,
    documents: Keyspace<OrderedU32, SerdeJson<Document>, Root>,
    words: Keyspace<Str, RoaringBitmapCodec, Root>,
}

impl Fiole {
    fn add_document(
        &self,
        wtxn: &mut Wtxn,
        document: Document,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        let docid = document.docid();

        // first we delete the old version of the document
        self.delete_document(wtxn, docid, dirty_words);

        let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
        // if a word is present multiple times in the same field we only count it once
        words.sort_unstable();
        words.dedup();

        for word in words.into_iter().filter(|w| !w.is_empty()) {
            match dirty_words.entry(word) {
                Entry::Occupied(mut bitmap) => {
                    bitmap.get_mut().insert(docid);
                }
                Entry::Vacant(entry) => {
                    // We get the current value from the db
                    let mut bitmap = self
                        .words
                        .get(wtxn, entry.key())
                        .unwrap()
                        .unwrap_or_default();
                    bitmap.insert(docid);
                    entry.insert(bitmap);
                }
            };
        }

        self.documents.insert(wtxn, &docid, &document).unwrap();
    }

    fn delete_document(
        &self,
        wtxn: &mut Wtxn,
        docid: DocId,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        if let Some(document) = self.documents.take(wtxn, &docid).unwrap() {
            // we get all the words in a document and then get rids of our id for each of these words
            let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
            // if a word is present multiple times in the same field we only count it once
            words.sort_unstable();
            words.dedup();

            for word in words.into_iter().filter(|w| !w.is_empty()) {
                match dirty_words.entry(word) {
                    Entry::Occupied(mut bitmap) => {
                        bitmap.get_mut().remove(docid);
                    }
                    Entry::Vacant(entry) => {
                        // We get the current value from the db
                        let mut bitmap = self.words.get(wtxn, entry.key()).unwrap().unwrap();
                        bitmap.remove(docid);
                        entry.insert(bitmap);
                    }
                }
            }
        }
    }

    /// Update all the entry in the dirty words.
    fn apply_dirty_words(&self, wtxn: &mut Wtxn, dirty_words: HashMap<String, RoaringBitmap>) {
        for (word, bitmap) in dirty_words {
            if bitmap.is_empty() {
                self.words.remove(wtxn, &word).unwrap();
            } else {
                self.words.insert(wtxn, &word, &bitmap).unwrap();
            }
        }
    }
}

impl Default for Fiole {
    fn default() -> Self {
        let db = Database::builder(DB_NAME).unwrap();
        let documents = db
            .keyspace(ks_name::DOCUMENTS, KeyspaceCreateOptions::default)
            .unwrap();
        let words = db
            .keyspace(ks_name::WORDS, KeyspaceCreateOptions::default)
            .unwrap();

        Self {
            db,
            documents,
            words,
        }
    }
}

impl Index for Fiole {
    fn get_documents(&self) -> Vec<Cow<Document>> {
        let rtxn = self.db.read_tx();
        self.documents
            .iter(&rtxn)
            .map(|entry| Cow::Owned(entry.value().unwrap()))
            .collect()
    }

    fn get_document(&self, id: DocId) -> Option<Cow<Document>> {
        let rtxn = self.db.read_tx();
        self.documents.get(&rtxn, &id).unwrap().map(Cow::Owned)
    }

    /// Since deser+ser the bitmaps for each insertion is slow we're going to create a
    /// temporary map of the words keyspace containing only the modified words.
    /// Then we reinsert + serialize all the modified words at once.
    fn add_documents(&mut self, documents: Vec<Document>) {
        let mut wtxn = self.db.write_tx().unwrap();
        let mut dirty_words = HashMap::new();

        documents
            .into_iter()
            .for_each(|document| self.add_document(&mut wtxn, document, &mut dirty_words));

        self.apply_dirty_words(&mut wtxn, dirty_words);
        wtxn.commit().unwrap().unwrap();
    }

    fn delete_documents(&mut self, docids: Vec<DocId>) {
        let mut wtxn = self.db.write_tx().unwrap();
        let mut dirty_words = HashMap::new();

        for docid in docids {
            self.delete_document(&mut wtxn, docid, &mut dirty_words);
        }
        self.apply_dirty_words(&mut wtxn, dirty_words);
        wtxn.commit().unwrap().unwrap();
    }

    fn search(&self, query: &Query) -> Vec<DocId> {
        let rtxn = self.db.read_tx();
        let docids = tokenize(query.q.as_deref().unwrap_or(""))
            .filter_map(|word| self.words.get(&rtxn, &word).unwrap())
            .fold(RoaringBitmap::default(), |acc, bitmap| acc | bitmap);

        docids.into_iter().collect()
    }

    fn clear_database() {
        match std::fs::remove_dir_all(DB_NAME) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => panic!("{e}"),
        }
    }
}
