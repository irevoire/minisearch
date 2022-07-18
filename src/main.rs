use minisearch::indexes;

#[tokio::main]
async fn main() {
    let index = indexes::Naive::default();
    println!("Starting http server on `http://localhost:3000`");

    minisearch::run(index).await;
}
