use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    io::{BufReader, ErrorKind, Seek, SeekFrom},
};

use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};

use crate::{tokenize, DocId, Document, Query};

use super::Index;

const DB_NAME: &str = "roaring.db";

#[derive(Debug)]
pub struct Roaring {
    inner: Inner,
    file: File,
}

#[derive(Debug, Serialize, Deserialize)]
struct Inner {
    documents: HashMap<DocId, Document>,
    words: HashMap<String, RoaringBitmap>,
}

impl Roaring {
    fn persist(&mut self) {
        self.file.seek(SeekFrom::Start(0)).unwrap();
        let mut writer = std::io::BufWriter::new(&mut self.file);
        serde_json::to_writer(&mut writer, &self.inner)
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
                .insert(docid);
        }
        self.inner.documents.insert(docid, document);
    }

    fn delete_document(&mut self, docid: DocId) {
        if let Some(document) = self.inner.documents.remove(&docid) {
            // we get all the words in a document and then get rids of our id for each of these words
            let mut words: Vec<_> = document.fields().flat_map(tokenize).collect();
            // if a word is present multiple times in the same field we only count it once
            words.sort_unstable();
            words.dedup();

            words.into_iter().for_each(|word| {
                self.inner.words.get_mut(&word).map(|ids| ids.remove(docid));
            });
        }
    }
}

impl Default for Roaring {
    fn default() -> Self {
        let mut file = match File::open(DB_NAME) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let mut index = Roaring {
                    inner: Inner {
                        documents: HashMap::new(),
                        words: HashMap::new(),
                    },
                    file: File::create(DB_NAME).expect("Can't open database"),
                };
                index.persist();
                return index;
            }
            Err(err) => panic!("{}", err),
        };
        let mut reader = BufReader::new(&mut file);
        let inner = serde_json::from_reader(&mut reader).expect("Corrupted database");
        let file = File::create(DB_NAME).expect("Can't write in database");

        let mut this = Self { inner, file };
        this.persist();
        this
    }
}

impl Index for Roaring {
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

    fn search(&self, query: &Query) -> Vec<DocId> {
        let docids = tokenize(&query.q)
            .filter_map(|word| self.inner.words.get(&word))
            .fold(RoaringBitmap::default(), |acc, bitmap| acc | bitmap);

        docids.into_iter().collect()
    }

    fn clear_database() {
        match std::fs::remove_file(DB_NAME) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Ok(()) => (),
            e => e.unwrap(),
        }
    }
}
