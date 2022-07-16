use std::collections::HashMap;

use crate::{tokenize, Document, Query};

use super::Index;

#[derive(Debug, Default)]
pub struct NaiveIndexer {
    documents: HashMap<usize, String>,
    words: HashMap<String, Vec<usize>>,
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

    fn add_document(&mut self, document: Document) {
        self.documents.insert(document.id, document.text.clone());
        tokenize(&document.text).for_each(|word| {
            self.words
                .entry(word.to_string())
                .or_default()
                .push(document.id)
        })
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
