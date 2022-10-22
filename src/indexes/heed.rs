use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    io::ErrorKind,
};

use heed::{
    types::{OwnedType, SerdeJson, Str},
    Database, Env, RwTxn,
};
use roaring::RoaringBitmap;

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "heed.db";

/// Database const names for the `IndexScheduler`.
mod db_name {
    pub const DOCUMENTS: &str = "documents";
    pub const WORDS: &str = "words";
}

pub struct Heed {
    env: Env,
    documents: Database<OwnedType<u32>, SerdeJson<Document>>,
    words: Database<Str, SerdeJson<RoaringBitmap>>,
}

impl Heed {
    fn add_document(
        &self,
        wtxn: &mut RwTxn,
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
                        .get(&wtxn, &entry.key())
                        .unwrap()
                        .unwrap_or_default();
                    bitmap.insert(docid);
                    entry.insert(bitmap);
                }
            };
        }

        self.documents.put(wtxn, &docid, &document).unwrap();
    }

    fn delete_document(
        &self,
        wtxn: &mut RwTxn,
        docid: DocId,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        if let Some(document) = self.documents.get(wtxn, &docid).unwrap() {
            self.documents.delete(wtxn, &docid).unwrap();
            // we get all the words in a document and then get rids of our id for each of these words
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
                        let mut bitmap = self.words.get(&wtxn, entry.key()).unwrap().unwrap();
                        bitmap.remove(docid);
                        entry.insert(bitmap);
                    }
                }
            }
        }
    }

    /// Update all the entry in the dirty words.
    fn apply_dirty_words(
        &self,
        wtxn: &mut RwTxn,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        for (word, bitmap) in dirty_words.into_iter() {
            self.words.put(wtxn, word, bitmap).unwrap();
        }
    }
}

impl Default for Heed {
    fn default() -> Self {
        match std::fs::create_dir(DB_NAME) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            e => e.unwrap(),
        }

        let mut options = heed::EnvOpenOptions::new();
        options.max_dbs(3);
        options.map_size(1024 * 1024 * 1024);
        let env = options.open(&DB_NAME).unwrap();

        Self {
            documents: env.create_database(Some(db_name::DOCUMENTS)).unwrap(),
            words: env.create_database(Some(db_name::WORDS)).unwrap(),
            env,
        }
    }
}

impl Index for Heed {
    fn get_documents(&self) -> Vec<Cow<Document>> {
        let rtxn = self.env.read_txn().unwrap();
        self.documents
            .iter(&rtxn)
            .unwrap()
            .map(|entry| Cow::Owned(entry.unwrap().1))
            .collect()
    }

    fn get_document(&self, id: DocId) -> Option<Cow<Document>> {
        let rtxn = self.env.read_txn().unwrap();
        self.documents.get(&rtxn, &id).unwrap().map(Cow::Owned)
    }

    /// Since deser+ser the bitmaps for each insertion is slow we're going to create a
    /// temporary map of the words databases containing only the modified words.
    /// Then we reinsert + serialize all the modified words at once.
    fn add_documents(&mut self, documents: Vec<Document>) {
        let mut wtxn = self.env.write_txn().unwrap();
        let mut dirty_words = HashMap::new();

        documents
            .into_iter()
            .for_each(|document| self.add_document(&mut wtxn, document, &mut dirty_words));

        self.apply_dirty_words(&mut wtxn, &mut dirty_words);
        wtxn.commit().unwrap();
    }

    fn delete_documents(&mut self, docids: Vec<DocId>) {
        let mut wtxn = self.env.write_txn().unwrap();
        let mut dirty_words = HashMap::new();

        for docid in docids {
            self.delete_document(&mut wtxn, docid, &mut dirty_words);
        }
        self.apply_dirty_words(&mut wtxn, &mut dirty_words);
        wtxn.commit().unwrap();
    }

    fn search(&self, query: &Query) -> Vec<DocId> {
        let rtxn = self.env.read_txn().unwrap();
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
