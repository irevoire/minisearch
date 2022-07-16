use std::collections::HashMap;

use crate::{Document, Query};

#[derive(Debug, Default)]
pub struct Index {
    documents: HashMap<usize, String>,
    words: HashMap<String, Vec<usize>>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_document(&self, id: usize) -> Option<Document> {
        self.documents.get(&id).map(|text| Document {
            id,
            text: text.to_string(),
        })
    }

    pub fn add(&mut self, document: Document) {
        self.documents.insert(document.id, document.text.clone());
        document.text.split_whitespace().for_each(|word| {
            self.words
                .entry(word.to_string())
                .or_default()
                .push(document.id)
        })
    }

    pub fn search(&self, query: Query) -> Vec<Document> {
        let mut docids: Vec<_> = query
            .q
            .split_whitespace()
            .filter_map(|word| self.words.get(word))
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
