mod common;

#[test]
fn ndjson_chunk_placeholder() {
    let line = common::mock_ollama::example_chat_stream_chunk();
    assert!(line.contains("\"done\":false"));
    assert!(line.contains("\"content\":\"hello\""));
}
