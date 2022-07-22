use std::time::Instant;
use std::{borrow::Cow, sync::Arc};

use axum::{extract, response, routing::get, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::{DocId, Index as RawIndex};

type Index<I> = Arc<RwLock<I>>;

pub async fn run<I: RawIndex + 'static>(index: I) {
    let index = Arc::new(RwLock::new(index));
    // our router
    let app = Router::new()
        .route("/", get(root))
        .route("/documents/:docid", get(get_document::<I>))
        .route(
            "/documents",
            get(get_documents::<I>)
                .post(add_documents::<I>)
                .delete(delete_documents::<I>),
        )
        .route("/search", get(search::<I>))
        .layer(extract::Extension(index));

    log::info!("Server started on `http://localhost:3000/`");

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

async fn get_document<I: RawIndex>(
    extract::Extension(index): extract::Extension<Index<I>>,
    extract::Path(docid): extract::Path<DocId>,
) -> response::Json<Option<Document>> {
    response::Json(index.read().await.get_document(docid).map(Cow::into_owned))
}

async fn get_documents<I: RawIndex>(
    extract::Extension(index): extract::Extension<Index<I>>,
) -> response::Json<Vec<Document>> {
    response::Json(
        index
            .read()
            .await
            .get_documents()
            .into_iter()
            .map(Cow::into_owned)
            .collect(),
    )
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Multiple(Vec<T>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document(serde_json::Map<String, serde_json::Value>);

impl Document {
    pub fn docid(&self) -> DocId {
        for (field, value) in self.0.iter() {
            if field.contains("id") {
                return value
                    .as_u64()
                    .map(|value| value as DocId)
                    .or_else(|| value.as_str().map(|s| s.parse::<DocId>().ok()).flatten())
                    .expect("Document id is supposed to be an integer")
                    as DocId;
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

async fn add_documents<I: RawIndex>(
    extract::Extension(index): extract::Extension<Index<I>>,
    extract::Json(documents): extract::Json<OneOrMany<Document>>,
) -> response::Json<Value> {
    let now = Instant::now();

    let mut index = index.write().await;
    match documents {
        OneOrMany::One(document) => index.add_documents(vec![document]),
        OneOrMany::Multiple(documents) => index.add_documents(documents),
    }

    response::Json(json!({ "elapsed": format!("{:?}", now.elapsed()) }))
}

async fn delete_documents<I: RawIndex>(
    extract::Extension(index): extract::Extension<Index<I>>,
    extract::Json(docids): extract::Json<OneOrMany<DocId>>,
) -> response::Json<Value> {
    let now = Instant::now();

    let mut index = index.write().await;
    match docids {
        OneOrMany::One(docid) => index.delete_documents(vec![docid]),
        OneOrMany::Multiple(docids) => index.delete_documents(docids),
    }

    response::Json(json!({ "elapsed": format!("{:?}", now.elapsed()) }))
}

#[derive(Deserialize, Clone, Debug)]
pub struct Query {
    pub q: String,
}

async fn search<I: RawIndex>(
    extract::Extension(index): extract::Extension<Index<I>>,
    extract::Query(query): extract::Query<Query>,
) -> response::Json<Value> {
    let now = Instant::now();

    let index = index.read().await;
    let results = index.search(&query);

    let response = json!({ "elapsed": format!("{:?}", now.elapsed()), "nb_hits": results.len(), "results": results.into_iter().take(3).collect::<Vec<_>>() });
    response::Json(response)
}
