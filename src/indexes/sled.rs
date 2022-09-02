use std::borrow::Cow;

use roaring::RoaringBitmap;

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "sled.db";

#[derive(Debug)]
pub struct Sled {
    documents: sled::Db,
    words: sled::Db,
    // documents: HashMap<DocId, Document>,
    // words: HashMap<String, RoaringBitmap>,
}

impl Sled {
    fn add_document(&mut self, document: Document) {
        let docid = document.docid();

        // first we delete the old version of the document
        self.delete_document(docid);

        let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
        // if a word is present multiple times in the same field we only count it once
        words.sort_unstable();
        words.dedup();

        for word in words {
            self.words
                .fetch_and_update(word, |word| {
                    let mut ids = match word {
                        Some(word) => roaring::RoaringBitmap::deserialize_from(word).unwrap(),
                        None => RoaringBitmap::new(),
                    };
                    ids.insert(docid);
                    let mut buff = Vec::with_capacity(ids.serialized_size());
                    ids.serialize_into(&mut buff).unwrap();
                    Some(buff)
                })
                .unwrap();
        }
        self.documents
            .insert(docid.to_ne_bytes(), serde_json::to_vec(&document).unwrap())
            .unwrap();
    }

    fn delete_document(&mut self, docid: DocId) {
        if let Some(document) = self.documents.remove(docid.to_ne_bytes()).unwrap() {
            let document: Document =
                serde_json::from_slice(&document).expect("Can't parse document");
            // we get all the words in a document and then get rids of our id for each of these words
            let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
            // if a word is present multiple times in the same field we only count it once
            words.sort_unstable();
            words.dedup();

            words.into_iter().for_each(|word| {
                self.words
                    .fetch_and_update(&word, |bitmap| {
                        let mut ids =
                            roaring::RoaringBitmap::deserialize_from(bitmap.unwrap()).unwrap();
                        ids.remove(docid);
                        let mut buff = Vec::with_capacity(ids.serialized_size());
                        ids.serialize_into(&mut buff).unwrap();
                        Some(buff)
                    })
                    .unwrap();
            });
        }
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
            documents: doc_mapping,
            words,
        }
    }
}

impl Index for Sled {
    fn get_documents(&self) -> Vec<Cow<Document>> {
        self.documents
            .iter()
            .map(|id| {
                self.get_document(u32::from_ne_bytes((*id.unwrap().0).try_into().unwrap()))
                    .expect("Corrupted database")
                    .clone()
            })
            .collect()
    }

    fn get_document(&self, id: DocId) -> Option<Cow<Document>> {
        self.documents
            .get(&id.to_ne_bytes())
            .unwrap()
            .map(|doc| serde_json::from_slice(&doc).unwrap())
            .map(Cow::Owned)
    }

    fn add_documents(&mut self, document: Vec<Document>) {
        document
            .into_iter()
            .for_each(|document| self.add_document(document));

        self.documents.flush().unwrap();
        self.words.flush().unwrap();
    }

    fn delete_documents(&mut self, docids: Vec<DocId>) {
        for docid in docids {
            self.delete_document(docid);
        }

        self.documents.flush().unwrap();
        self.words.flush().unwrap();
    }

    fn search(&self, query: &Query) -> Vec<Cow<Document>> {
        let docids = tokenize(&query.q)
            .filter_map(|word| self.words.get(&word).unwrap())
            .fold(RoaringBitmap::default(), |acc, bitmap| {
                acc | RoaringBitmap::deserialize_from(&*bitmap).unwrap()
            });

        docids
            .into_iter()
            .map(|docid| {
                self.get_document(docid)
                    .expect("Internal error. Database corrupted")
            })
            .collect()
    }

    fn clear_database() {
        std::fs::remove_dir_all(DB_NAME).unwrap();
    }
}
