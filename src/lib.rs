mod api;
mod tokenizer;

pub use api::{run, Document, Query};
pub use tokenizer::tokenize;

pub mod indexes;

pub use crate::indexes::Index;

type DocId = u32;
