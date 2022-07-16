use std::{collections::HashMap, fs::File, io::ErrorKind};

use serde::{Deserialize, Serialize};

use crate::{tokenize, Document, Query};

use super::Index;

const DB_NAME: &str = "naive.db";

#[derive(Debug, Serialize, Deserialize)]
pub struct NaiveIndexer {
    documents: HashMap<usize, String>,
    words: HashMap<String, Vec<usize>>,
}

impl NaiveIndexer {
    fn persist(&self) {
        let file = match File::open(DB_NAME) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                File::create(DB_NAME).expect("Can't open database")
            }
            Err(err) => panic!("{}", err),
        };
        bincode::serialize_into(file, self).expect("Can't write on disk")
    }

    fn add_document(&mut self, document: Document) {
        self.documents.insert(document.id, document.text.clone());
        tokenize(&document.text).for_each(|word| {
            self.words
                .entry(word.to_string())
                .or_default()
                .push(document.id)
        })
    }
}

impl Default for NaiveIndexer {
    fn default() -> Self {
        let file = match File::open(DB_NAME) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return NaiveIndexer {
                    documents: HashMap::new(),
                    words: HashMap::new(),
                };
            }
            Err(err) => panic!("{}", err),
        };
        bincode::deserialize_from(file).expect("Corrupted database")
    }
}

impl Index for NaiveIndexer {
    fn get_documents(&self) -> Vec<Document> {
        self.documents
            .keys()
            .map(|id| self.get_document(*id).expect("Corrupted database"))
            .collect()
    }

    fn get_document(&self, id: usize) -> Option<Document> {
        self.documents.get(&id).map(|text| Document {
            id,
            text: text.to_string(),
        })
    }

    fn add_documents(&mut self, document: Vec<Document>) {
        document
            .into_iter()
            .for_each(|document| self.add_document(document));

        self.persist()
    }

    fn search(&self, query: Query) -> Vec<Document> {
        let mut docids: Vec<_> = tokenize(&query.q)
            .filter_map(|word| self.words.get(&word))
            .flatten()
            .collect();

        docids.sort_unstable();
        docids.dedup();

        docids
            .into_iter()
            .map(|docid| {
                self.get_document(*docid)
                    .expect("Internal error. Database corrupted")
            })
            .collect()
    }
}
