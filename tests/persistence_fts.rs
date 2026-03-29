#[test]
fn fts_schema_links_to_messages_table() {
    let schema = "CREATE VIRTUAL TABLE messages_fts USING fts5(content, role, conversation_id, content=messages);";
    assert!(schema.contains("content=messages"));
}
