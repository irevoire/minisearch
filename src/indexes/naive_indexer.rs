use std::{
    collections::HashMap,
    fs::File,
    io::{ErrorKind, Seek, SeekFrom},
};

use serde::{Deserialize, Serialize};

use crate::{tokenize, Document, Query};

use super::Index;

const DB_NAME: &str = "naive.db";

#[derive(Debug)]
pub struct NaiveIndexer {
    inner: Inner,
    file: File,
}

#[derive(Debug, Serialize, Deserialize)]
struct Inner {
    documents: HashMap<u32, Document>,
    words: HashMap<String, Vec<u32>>,
}

impl NaiveIndexer {
    fn persist(&mut self) {
        self.file.seek(SeekFrom::Start(0)).unwrap();
        serde_json::to_writer(&mut self.file, &self.inner)
            .expect("Internal error, can't serialize document");
    }

    fn add_document(&mut self, document: Document) {
        let docid = document.docid();
        for field in document.fields() {
            tokenize(field).for_each(|word| {
                self.inner
                    .words
                    .entry(word.to_string())
                    .or_default()
                    .push(docid)
            })
        }
        self.inner.documents.insert(docid, document);
    }
}

impl Default for NaiveIndexer {
    fn default() -> Self {
        let mut file = match File::open(DB_NAME) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return NaiveIndexer {
                    inner: Inner {
                        documents: HashMap::new(),
                        words: HashMap::new(),
                    },
                    file: File::create(DB_NAME).expect("Can't open database"),
                };
            }
            Err(err) => panic!("{}", err),
        };
        let inner = serde_json::from_reader(&mut file).expect("Corrupted database");
        let file = File::create(DB_NAME).expect("Can't write in database");
        Self { inner, file }
    }
}

impl Index for NaiveIndexer {
    fn get_documents(&self) -> Vec<Document> {
        self.inner
            .documents
            .keys()
            .map(|id| self.get_document(*id).expect("Corrupted database").clone())
            .collect()
    }

    fn get_document(&self, id: u32) -> Option<Document> {
        self.inner.documents.get(&id).cloned()
    }

    fn add_documents(&mut self, document: Vec<Document>) {
        document
            .into_iter()
            .for_each(|document| self.add_document(document));

        self.persist()
    }

    fn search(&self, query: Query) -> Vec<Document> {
        let mut docids: Vec<_> = tokenize(&query.q)
            .filter_map(|word| self.inner.words.get(&word))
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
