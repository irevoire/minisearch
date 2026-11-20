use std::sync::{LazyLock, Mutex};

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};

use crate::{tokenize, DocId, Index};

const DB_NAME: &str = "sqlite.db";

static CONNECTION: LazyLock<Mutex<Connection>> = LazyLock::new(|| {
    Mutex::new(Connection::open(DB_NAME).expect("Couldn’t init sqlite database"))
});

/// Generate the `?, ?, ?` placeholders of an `IN` clause, one for each value to bind.
fn placeholders(n: usize) -> String {
    vec!["?"; n].join(", ")
}

pub struct SQLite {}

impl Index for SQLite {
    fn get_documents(&self) -> Vec<std::borrow::Cow<crate::Document>> {
        let connection = CONNECTION.lock().unwrap();
        let res = connection
            .prepare(
                r#"
        SELECT document FROM documents;
        "#,
            )
            .unwrap()
            .query_map([], |row| {
                Ok(serde_json::from_slice(
                    &row.get::<_, Vec<u8>>(0).expect("Error retrieving document"),
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .map(Result::unwrap)
            .map(|doc| std::borrow::Cow::Owned(doc))
            .collect();

        res
    }

    fn get_document(&self, id: crate::DocId) -> Option<std::borrow::Cow<crate::Document>> {
        let connection = CONNECTION.lock().unwrap();
        let res = connection
            .prepare("SELECT document FROM documents WHERE doc_id = ?1;")
            .unwrap()
            .query_row(params![id], |row| {
                Ok(serde_json::from_slice(
                    &row.get::<_, Vec<u8>>(0).expect("Error retrieving row"),
                ))
            })
            .optional()
            .unwrap()
            .map(|doc| std::borrow::Cow::Owned(doc.unwrap()));

        res
    }

    fn add_documents(&mut self, documents: Vec<crate::Document>) {
        let mut connection = CONNECTION.lock().unwrap();
        // commit the whole batch at once instead of committing every single insert
        let transaction = connection.transaction().unwrap();
        {
            let mut doc_stmt = transaction
                .prepare(
                    r#"
            INSERT INTO documents (doc_id, document) VALUES (?, ?)
            ON CONFLICT(doc_id) DO UPDATE SET document = excluded.document;
            "#,
                )
                .unwrap();
            let mut search_stmt = transaction
                .prepare(
                    r#"
            INSERT INTO document_search (doc_id, word) VALUES (?, ?);
            "#,
                )
                .unwrap();
            let mut del_search_stmt = transaction
                .prepare(
                    r#"
            DELETE FROM document_search WHERE doc_id = ?;
            "#,
                )
                .unwrap();
            for document in &documents {
                let doc_bytes =
                    serde_json::to_vec(document).expect("Error while serializing document");
                let doc_id = document.docid();
                del_search_stmt
                    .execute(params![doc_id])
                    .expect("Error while deleting previous search");
                doc_stmt
                    .execute(params![doc_id, doc_bytes])
                    .expect("Error while inserting document");
                document.fields().flat_map(tokenize).for_each(|word| {
                    search_stmt.execute(params![doc_id, word]).unwrap();
                });
            }
        }
        transaction.commit().unwrap();
    }

    fn delete_documents(&mut self, documents: Vec<crate::DocId>) {
        if documents.is_empty() {
            return;
        }
        let connection = CONNECTION.lock().unwrap();
        let placeholders = placeholders(documents.len());
        connection
            .prepare(&format!(
                "DELETE FROM document_search WHERE doc_id IN ({placeholders});"
            ))
            .unwrap()
            .execute(params_from_iter(&documents))
            .unwrap();
        connection
            .prepare(&format!(
                "DELETE FROM documents WHERE doc_id IN ({placeholders});"
            ))
            .unwrap()
            .execute(params_from_iter(&documents))
            .unwrap();
    }

    fn search(&self, query: &crate::Query) -> Vec<DocId> {
        let words = tokenize(query.q.as_deref().unwrap_or("")).collect::<Vec<String>>();
        if words.is_empty() {
            return Vec::new();
        }

        let res = CONNECTION
            .lock()
            .unwrap()
            .prepare(&format!(
                r#"
            SELECT DISTINCT document_search.doc_id FROM document_search
            WHERE document_search.word IN ({})
            ORDER BY document_search.doc_id;
            "#,
                placeholders(words.len())
            ))
            .unwrap()
            .query_map(params_from_iter(&words), |row| row.get::<_, u32>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();

        res
    }

    fn database_path() -> &'static str {
        DB_NAME
    }

    fn clear_database() {
        // the tables are recreated when the index is opened with `SQLite::default`
        let connection = CONNECTION.lock().unwrap();
        connection
            .execute_batch(
                r#"
            DROP TABLE IF EXISTS document_search;
            DROP TABLE IF EXISTS documents;
            "#,
            )
            .expect("Error while clearing the database");
    }
}

impl Default for SQLite {
    fn default() -> Self {
        let connection = CONNECTION.lock().unwrap();
        connection
            .prepare(
                r#"
                CREATE TABLE IF NOT EXISTS documents (
                    doc_id INT PRIMARY KEY,
                    document BLOB NOT NULL
        );
        "#,
            )
            .expect("Error while preparing init query")
            .raw_execute()
            .expect("Error while executing init query");
        connection
            .prepare(
                r#"
            CREATE TABLE IF NOT EXISTS document_search (
                doc_id INT NOT NULL,
                word TEXT NOT NULL,
                FOREIGN KEY(doc_id) REFERENCES documents(doc_id)
            );
        "#,
            )
            .expect("Error while preparing init query")
            .raw_execute()
            .expect("Error while executing init query");
        connection
            .execute_batch(
                r#"
            -- used to delete the words of a document without scanning the whole table
            CREATE INDEX IF NOT EXISTS document_search_doc_id ON document_search (doc_id);
            -- used by the search, it contains the doc_id so we never need to read the table
            CREATE INDEX IF NOT EXISTS document_search_word ON document_search (word, doc_id);
        "#,
            )
            .expect("Error while creating the indexes");
        Self {}
    }
}
