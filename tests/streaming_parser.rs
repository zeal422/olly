mod common;

#[test]
fn parse_valid_stream_chunk() {
    let line = common::mock_ollama::example_chat_stream_chunk();
    let parsed = olly::ollama::chat::parse_stream_line(line).expect("parse line");
    let parsed = parsed.expect("chunk exists");
    assert_eq!(parsed.content, "hello");
    assert!(!parsed.done);
    assert!(parsed.tool_calls.is_empty());
}

#[test]
fn parse_empty_stream_chunk_returns_none() {
    let parsed = olly::ollama::chat::parse_stream_line("   ").expect("parse empty");
    assert!(parsed.is_none());
}
