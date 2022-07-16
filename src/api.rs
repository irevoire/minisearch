use axum::{extract, response, routing::get, Router};
use serde::{Deserialize, Serialize};

use crate::Indexer;

pub async fn run(index: Indexer) {
    // our router
    let app = Router::new()
        .route("/", get(root))
        .route("/document/:docid", get(get_document))
        .route("/document", get(get_document_help).post(add_document))
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

async fn get_document_help() -> &'static str {
    "Call `/document/:docid` or `/search`"
}

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub id: usize,
    pub text: String,
}

async fn add_document(
    extract::Extension(index): extract::Extension<Indexer>,
    extract::Json(document): extract::Json<Document>,
) {
    index.lock().await.add(document);
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
