use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    io::{ErrorKind, Seek, SeekFrom},
};

use serde::{Deserialize, Serialize};

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "naive.db";

#[derive(Debug)]
pub struct Naive {
    inner: Inner,
    file: File,
}

#[derive(Debug, Serialize, Deserialize)]
struct Inner {
    documents: HashMap<DocId, Document>,
    words: HashMap<String, Vec<DocId>>,
}

impl Naive {
    fn persist(&mut self) {
        self.file.seek(SeekFrom::Start(0)).unwrap();
        serde_json::to_writer(&mut self.file, &self.inner)
            .expect("Internal error, can't serialize document");
    }

    fn add_document(&mut self, document: Document) {
        let docid = document.docid();

        // first we delete the old version of the document
        self.delete_document(docid);

        let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
        // if a word is present multiple times in the same field we only count it once
        words.sort_unstable();
        words.dedup();

        for word in words {
            self.inner
                .words
                .entry(word.to_string())
                .or_default()
                .push(docid)
        }
        self.inner.documents.insert(docid, document);
    }

    fn delete_document(&mut self, docid: DocId) {
        if let Some(document) = self.inner.documents.remove(&docid) {
            // we get all the words in a document and then extract get rids of our id for each of these words
            let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
            // if a word is present multiple times in the same field we only count it once
            words.sort_unstable();
            words.dedup();

            words.into_iter().for_each(|word| {
                self.inner
                    .words
                    .get_mut(&word)
                    .map(|ids| ids.retain(|id| *id != docid));
            });
        }
    }
}

impl Default for Naive {
    fn default() -> Self {
        let mut file = match File::open(DB_NAME) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Naive {
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

        let mut this = Self { inner, file };
        this.persist();
        this
    }
}

impl Index for Naive {
    fn get_documents(&self) -> Vec<Cow<Document>> {
        self.inner
            .documents
            .keys()
            .map(|id| self.get_document(*id).expect("Corrupted database").clone())
            .collect()
    }

    fn get_document(&self, id: DocId) -> Option<Cow<Document>> {
        self.inner.documents.get(&id).map(Cow::Borrowed)
    }

    fn add_documents(&mut self, document: Vec<Document>) {
        document
            .into_iter()
            .for_each(|document| self.add_document(document));

        self.persist()
    }

    fn delete_documents(&mut self, docids: Vec<DocId>) {
        for docid in docids {
            self.delete_document(docid);
        }
        self.persist();
    }

    fn search(&self, query: &Query) -> Vec<Cow<Document>> {
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

    fn clear_database() {
        std::fs::remove_file(DB_NAME).unwrap();
    }
}
