use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    io::ErrorKind,
    sync::Mutex,
};

use roaring::RoaringBitmap;

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "sled.db";

#[derive(Debug)]
pub struct Sled {
    // Since sled 1.0 a `Db` is `Send` but not `Sync`
    documents: Mutex<sled::Db>,
    words: Mutex<sled::Db>,
}

impl Sled {
    fn add_document(
        &mut self,
        document: Document,
        dirty_words: &mut HashMap<String, RoaringBitmap>,
    ) {
        let docid = document.docid();

        // first we delete the old version of the document
        self.delete_document(docid, dirty_words);

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
                    let mut bitmap = match self.words.get_mut().unwrap().get(entry.key()).unwrap() {
                        Some(bytes) => RoaringBitmap::deserialize_from(&*bytes).unwrap(),
                        None => RoaringBitmap::new(),
                    };
                    bitmap.insert(docid);
                    entry.insert(bitmap);
                }
            };
        }
        self.documents
            .get_mut()
            .unwrap()
            .insert(docid.to_ne_bytes(), serde_json::to_vec(&document).unwrap())
            .unwrap();
    }

    fn delete_document(&mut self, docid: DocId, dirty_words: &mut HashMap<String, RoaringBitmap>) {
        let documents = self.documents.get_mut().unwrap();
        if let Some(document) = documents.remove(docid.to_ne_bytes()).unwrap() {
            let document: Document =
                serde_json::from_slice(&document).expect("Can't parse document");
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
                        let bytes = self
                            .words
                            .get_mut()
                            .unwrap()
                            .get(entry.key())
                            .unwrap()
                            .unwrap();
                        let mut bitmap =
                            roaring::RoaringBitmap::deserialize_from(bytes.as_ref()).unwrap();
                        bitmap.remove(docid);
                        entry.insert(bitmap);
                    }
                }
            }
        }
    }

    /// Update all the entry in the dirty words.
    fn apply_dirty_words(&mut self, dirty_words: &mut HashMap<String, RoaringBitmap>) {
        let words = self.words.get_mut().unwrap();
        // we reuse the same allocation for all the documents
        let mut buffer = Vec::new();

        for (word, bitmap) in dirty_words.into_iter() {
            bitmap.serialize_into(&mut buffer).unwrap();
            words.insert(word.as_str(), buffer.as_slice()).unwrap();
            buffer.clear();
        }

        words.flush().unwrap();
    }
}

impl Default for Sled {
    fn default() -> Self {
        match std::fs::create_dir(DB_NAME) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            e => e.unwrap(),
        }
        let doc_mapping: sled::Db = sled::open(format!("{DB_NAME}/doc_mapping.db")).unwrap();
        let words: sled::Db = sled::open(format!("{DB_NAME}/words.db")).unwrap();
        Self {
            documents: Mutex::new(doc_mapping),
            words: Mutex::new(words),
        }
    }
}

impl Index for Sled {
    fn get_documents(&self) -> Vec<Cow<Document>> {
        // decode the values directly, calling `get_document` would deadlock on the mutex
        self.documents
            .lock()
            .unwrap()
            .iter()
            .map(|entry| Cow::Owned(serde_json::from_slice(&entry.unwrap().1).unwrap()))
            .collect()
    }

    fn get_document(&self, id: DocId) -> Option<Cow<Document>> {
        self.documents
            .lock()
            .unwrap()
            .get(&id.to_ne_bytes())
            .unwrap()
            .map(|doc| serde_json::from_slice(&doc).unwrap())
            .map(Cow::Owned)
    }

    /// Since deser+ser the bitmaps for each insertion is slow we're going to create a
    /// temporary map of the words databases containing only the modified words.
    /// Then we reinsert + serialize all the modified words at once.
    fn add_documents(&mut self, documents: Vec<Document>) {
        let mut dirty_words = HashMap::new();

        documents
            .into_iter()
            .for_each(|document| self.add_document(document, &mut dirty_words));

        self.apply_dirty_words(&mut dirty_words);
        self.documents.get_mut().unwrap().flush().unwrap();
    }

    fn delete_documents(&mut self, docids: Vec<DocId>) {
        let mut dirty_words = HashMap::new();

        for docid in docids {
            self.delete_document(docid, &mut dirty_words);
        }
        self.apply_dirty_words(&mut dirty_words);
        self.documents.get_mut().unwrap().flush().unwrap();
    }

    fn search(&self, query: &Query) -> Vec<DocId> {
        let words = self.words.lock().unwrap();
        let docids = tokenize(query.q.as_deref().unwrap_or(""))
            .filter_map(|word| words.get(&word).unwrap())
            .fold(RoaringBitmap::default(), |acc, bitmap| {
                acc | RoaringBitmap::deserialize_from(&*bitmap).unwrap()
            });

        docids.into_iter().collect()
    }

    fn database_path() -> &'static str {
        DB_NAME
    }

    fn clear_database() {
        match std::fs::remove_dir_all(DB_NAME) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => panic!("{e}"),
        }
    }
}
