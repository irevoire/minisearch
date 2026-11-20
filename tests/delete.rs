use minisearch::{indexes, Document, Index, Query};

fn document(json: &str) -> Document {
    serde_json::from_str(json).unwrap()
}

fn search(index: &impl Index, q: &str) -> Vec<u32> {
    index.search(&Query {
        q: Some(q.to_string()),
        limit: 20,
    })
}

/// Deleting several documents sharing a word in the same batch must remove all of them.
fn delete_batch_with_shared_word<I: Index>() {
    I::clear_database();
    let mut index = I::default();
    index.add_documents(vec![
        document(r#"{ "id": 1, "text": "hello world" }"#),
        document(r#"{ "id": 2, "text": "hello bob" }"#),
        document(r#"{ "id": 3, "text": "hello dog" }"#),
    ]);

    index.delete_documents(vec![1, 2]);

    assert_eq!(search(&index, "hello"), vec![3]);
    assert_eq!(search(&index, "world"), Vec::<u32>::new());
    assert_eq!(search(&index, "bob"), Vec::<u32>::new());
    assert_eq!(search(&index, "world bob dog"), vec![3]);
    assert!(index.get_document(1).is_none());
    assert!(index.get_document(3).is_some());
    I::clear_database();
}

/// Replacing a document must drop its old words even if another document of the batch uses them.
fn replace_in_batch_with_shared_word<I: Index>() {
    I::clear_database();
    let mut index = I::default();
    index.add_documents(vec![document(r#"{ "id": 1, "text": "hello world" }"#)]);

    index.add_documents(vec![
        document(r#"{ "id": 2, "text": "hello bob" }"#),
        document(r#"{ "id": 1, "text": "bye world" }"#),
    ]);

    assert_eq!(search(&index, "hello"), vec![2]);
    assert_eq!(search(&index, "bye"), vec![1]);
    I::clear_database();
}

// Each backend always uses the same database directory, so its scenarios must run in the same test
// to not race with each other.
#[test]
fn heed() {
    delete_batch_with_shared_word::<indexes::Heed>();
    replace_in_batch_with_shared_word::<indexes::Heed>();
}

#[test]
fn sled() {
    delete_batch_with_shared_word::<indexes::Sled>();
    replace_in_batch_with_shared_word::<indexes::Sled>();
}

#[test]
fn sqlite() {
    delete_batch_with_shared_word::<indexes::SQLite>();
    replace_in_batch_with_shared_word::<indexes::SQLite>();
}

#[test]
fn fiole() {
    delete_batch_with_shared_word::<indexes::Fiole>();
    replace_in_batch_with_shared_word::<indexes::Fiole>();
}
