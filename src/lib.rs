pub mod ai;
pub mod config;
pub mod discover;
pub mod ingest;
pub mod models;
pub mod store;
pub mod task;
pub mod workspace;

#[cfg(test)]
#[test]
fn stores_chunks_and_searches_full_text() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("metadata.sqlite");
    let store = store::MetadataStore::open(&db).unwrap();

    let doc = store::DocumentRecord::new_for_test("doc-1", "sample.txt", "text/plain");
    store.upsert_document(&doc).unwrap();
    store
        .insert_chunk("chunk-1", "doc-1", "text", "客户准入规则", None, None)
        .unwrap();

    let results = store.search_text("准入", 5).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].chunk_id, "chunk-1");
}
