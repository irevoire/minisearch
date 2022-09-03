use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::{tokenize, DocId, Index};

lazy_static::lazy_static!(
    static ref CONNECTION: Mutex<Connection> = Mutex::new(Connection::open("sqlite.db").expect("Couldn’t init sqlite database"));
);

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
            .unwrap()
            .unwrap();

        Some(std::borrow::Cow::Owned(res))
    }

    fn add_documents(&mut self, documents: Vec<crate::Document>) {
        let connection = CONNECTION.lock().unwrap();
        let mut doc_stmt = connection
            .prepare(
                r#"
            INSERT INTO documents (doc_id, document) VALUES (?, ?)
            ON CONFLICT(doc_id) DO UPDATE SET document = excluded.document;
            "#,
            )
            .unwrap();
        let mut search_stmt = connection
            .prepare(
                r#"
            INSERT INTO document_search (doc_id, word) VALUES (?, ?);
            "#,
            )
            .unwrap();
        let mut del_search_stmt = connection
            .prepare(
                r#"
            DELETE FROM document_search WHERE doc_id = ?;
            "#,
            )
            .unwrap();
        for document in &documents {
            let doc_bytes = serde_json::to_vec(document).expect("Error while serializing document");
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

    fn delete_documents(&mut self, documents: Vec<crate::DocId>) {
        let connection = CONNECTION.lock().unwrap();
        let ids = documents
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
            .join(",");
        connection
            .prepare("DELETE FROM document_search WHERE doc_id IN (?);")
            .unwrap()
            .execute(params![ids])
            .unwrap();
        connection
            .prepare("DELETE FORM documents WHERE doc_id IN (?);")
            .unwrap()
            .execute(params![ids])
            .unwrap();
    }

    fn search(&self, query: &crate::Query) -> Vec<DocId> {
        let words = tokenize(query.q.as_deref().unwrap_or(""))
            .collect::<Vec<String>>()
            .join(",");

        let res = CONNECTION
            .lock()
            .unwrap()
            .prepare(
                r#"
            SELECT DISTINCT document_search.doc_id FROM document_search
            WHERE document_search.word IN (?);
            "#,
            )
            .unwrap()
            .query_map(params![words], |row| row.get::<_, u32>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();

        res
    }

    fn clear_database() {
        let connection = CONNECTION.lock().unwrap();
        match connection.execute("DELETE FROM document_search;", []) {
            Ok(nb_rows) => println!("document_search has been purged. {} rows deleted", nb_rows),
            Err(err) => println!("document_search couldn’t be deleted {}", err),
        }
        match connection.execute("DELETE FROM documents;", []) {
            Ok(nb_rows) => println!("documents has been purged. {} rows deleted", nb_rows),
            Err(err) => println!("documents couldn’t be deleted {}", err),
        }
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
        Self {}
    }
}
