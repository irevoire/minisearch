use axum::{extract, response, routing::get, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Indexer;

pub async fn run(index: Indexer) {
    // our router
    let app = Router::new()
        .route("/", get(root))
        .route("/documents/:docid", get(get_document))
        .route("/documents", get(get_documents).post(add_documents))
        .route("/search", get(search))
        .layer(extract::Extension(index));

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// which calls one of these handlers
async fn root() -> &'static str {
    "Call `/documents` or `/search`"
}

async fn get_document(
    extract::Extension(index): extract::Extension<Indexer>,
    extract::Path(docid): extract::Path<u32>,
) -> response::Json<Option<Document>> {
    response::Json(index.lock().await.get_document(docid))
}

async fn get_documents(
    extract::Extension(index): extract::Extension<Indexer>,
) -> response::Json<Vec<Document>> {
    response::Json(index.lock().await.get_documents())
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Documents {
    One(Document),
    Multiple(Vec<Document>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document(serde_json::Map<String, serde_json::Value>);

impl Document {
    pub fn docid(&self) -> u32 {
        for (field, value) in self.0.iter() {
            if field.contains("id") {
                return value
                    .as_u64()
                    .map(|value| value as u32)
                    .or_else(|| value.as_str().map(|s| s.parse::<u32>().ok()).flatten())
                    .expect("Document id is supposed to be an integer")
                    as u32;
            }
        }
        panic!(
            "Document {} do not contain a document id",
            serde_json::to_string_pretty(&self.0).unwrap()
        );
    }

    pub fn fields(&self) -> impl Iterator<Item = &str> {
        self.0.values().flat_map(|value| match value {
            Value::String(s) => {
                Box::new(std::iter::once(s.as_ref())) as Box<dyn Iterator<Item = &str>>
            }
            Value::Array(arr) => Box::new(arr.into_iter().flat_map(|el| Self::_fields(el)))
                as Box<dyn Iterator<Item = &str>>,
            Value::Object(obj) => Box::new(obj.values().flat_map(|value| Self::_fields(value)))
                as Box<dyn Iterator<Item = &str>>,
            _ => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = &str>>,
        })
    }

    fn _fields(value: &Value) -> impl Iterator<Item = &str> {
        match value {
            Value::String(s) => {
                Box::new(std::iter::once(s.as_ref())) as Box<dyn Iterator<Item = &str>>
            }
            Value::Array(arr) => Box::new(arr.into_iter().flat_map(|el| Self::_fields(el)))
                as Box<dyn Iterator<Item = &str>>,
            Value::Object(obj) => Box::new(obj.values().flat_map(|value| Self::_fields(value)))
                as Box<dyn Iterator<Item = &str>>,
            _ => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = &str>>,
        }
    }
}

async fn add_documents(
    extract::Extension(index): extract::Extension<Indexer>,
    extract::Json(documents): extract::Json<Documents>,
) {
    let mut index = index.lock().await;
    match documents {
        Documents::One(document) => index.add_documents(vec![document]),
        Documents::Multiple(documents) => index.add_documents(documents),
    }
}

#[derive(Deserialize)]
pub struct Query {
    pub q: String,
}

async fn search(
    extract::Extension(index): extract::Extension<Indexer>,
    extract::Query(query): extract::Query<Query>,
) -> response::Json<Vec<Document>> {
    response::Json(index.lock().await.search(query))
}
