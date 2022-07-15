use axum::{extract, response, routing::get, Router};
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    // our router
    let app = Router::new()
        .route("/", get(root))
        .route("/document/:docid", get(get_document))
        .route("/document", get(get_document_help).post(add_document))
        .route("/search", get(search));

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

async fn get_document(docid: extract::Path<usize>) -> response::Json<Document> {
    response::Json(Document {
        id: 42,
        text: "Hello".to_string(),
    })
}

async fn get_document_help() -> &'static str {
    "Call `/document/:docid` or `/search`"
}

#[derive(Serialize, Deserialize)]
pub struct Document {
    id: usize,
    text: String,
}

async fn add_document(doc: extract::Json<Document>) -> response::Json<Document> {
    response::Json(Document {
        id: 42,
        text: "Hello".to_string(),
    })
}

#[derive(Deserialize)]
pub struct Query {
    q: String,
}

async fn search(query: extract::Json<Query>) -> response::Json<Document> {
    response::Json(Document {
        id: 42,
        text: "Hello".to_string(),
    })
}
