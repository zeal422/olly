mod common;

#[test]
fn fts_index_updates_with_insert_and_search() {
    let db = common::temp_db::TempDb::new("fts");
    let manager =
        olly::workspace::manager::WorkspaceManager::open(db.path()).expect("open manager");
    let session = manager
        .bootstrap_default("Default Workspace", "llama3.2:latest")
        .expect("bootstrap");

    manager
        .save_message(
            session.conversation_id,
            "assistant",
            "Rust streaming responses are smooth in Olly",
        )
        .expect("save message");

    let hits = manager.search_messages("streaming").expect("search fts");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].conversation_id, session.conversation_id);
    assert_eq!(hits[0].role, "assistant");
    assert!(hits[0].content.contains("streaming"));
}
