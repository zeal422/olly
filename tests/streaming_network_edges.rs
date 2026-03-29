mod common;

use olly::ollama::{chat::ChatMessage, error::OllamaError};

#[test]
fn parse_stream_line_malformed_json_returns_json_error() {
    let err =
        olly::ollama::chat::parse_stream_line("{not-json").expect_err("malformed json should fail");
    assert!(matches!(err, OllamaError::Json(_)));
}

#[tokio::test]
async fn chat_stream_non_success_status_returns_api_error() {
    let server = common::mock_ollama::MockOllamaServer::spawn_once(500, r#"{"error":"boom"}"#);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];

    let err = olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |_| {},
    )
    .await
    .expect_err("non-success response should fail");
    assert!(matches!(err, OllamaError::Api(_)));
}

#[tokio::test]
async fn chat_stream_ignores_blank_lines_between_records() {
    let line1 = common::mock_ollama::stream_line("alpha", false);
    let line2 = common::mock_ollama::stream_line("beta", true);
    let body = format!("\n\n{line1}\n\n{line2}\n\n");
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, &body);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];
    let mut received = Vec::new();

    olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push(chunk.content.clone());
        },
    )
    .await
    .expect("stream should succeed");

    assert_eq!(received, vec![String::from("alpha"), String::from("beta")]);
}

#[test]
fn parse_stream_line_missing_content_defaults_to_empty() {
    let line = r#"{"model":"demo","message":{"role":"assistant"},"done":false}"#;
    let parsed = olly::ollama::chat::parse_stream_line(line)
        .expect("line should parse")
        .expect("chunk should exist");
    assert_eq!(parsed.content, "");
    assert!(!parsed.done);
}

#[tokio::test]
async fn chat_once_without_message_returns_empty_string() {
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, r#"{"done":true}"#);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];

    let response =
        olly::ollama::chat::chat_once(&client, &server.base_url, "llama3.2:latest", &messages)
            .await
            .expect("chat once should succeed");
    assert_eq!(response, "");
}
