use axum::{extract, response, routing::get, Router};
use serde::{Deserialize, Serialize};

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
    "Call `/document` or `/search`"
}

async fn get_document(
    extract::Extension(index): extract::Extension<Indexer>,
    extract::Path(docid): extract::Path<usize>,
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

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub id: usize,
    pub text: String,
}

async fn add_documents(
    extract::Extension(index): extract::Extension<Indexer>,
    extract::Json(documents): extract::Json<Documents>,
) {
    let mut index = index.lock().await;
    match documents {
        Documents::One(document) => index.add_document(document),
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
