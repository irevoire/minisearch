# Minisearch

A real simple search engine just for funsies.

## Basic setup

Run the project with `cargo run --release`.

```
# add documents
echo '{ "id": 1, "text": "Hello World" }' | http ':3000/document'
echo '{ "id": 2, "text": "Hello Bob" }' | http ':3000/document'

# search
http ':3000/search?q=Hello'
http ':3000/search?q=Bob'
```
