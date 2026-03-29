mod common;

use olly::ollama::{chat::ChatMessage, error::OllamaError};

#[tokio::test]
async fn list_models_maps_http_errors() {
    let server = common::mock_ollama::MockOllamaServer::spawn_once(404, r#"{"error":"not found"}"#);
    let client = reqwest::Client::new();

    let err = olly::ollama::models::list_models(&client, &server.base_url)
        .await
        .expect_err("expected api error");

    assert!(matches!(err, OllamaError::Api(_)));
}

#[tokio::test]
async fn chat_once_maps_http_errors() {
    let server = common::mock_ollama::MockOllamaServer::spawn_once(500, r#"{"error":"boom"}"#);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];

    let err =
        olly::ollama::chat::chat_once(&client, &server.base_url, "llama3.2:latest", &messages)
            .await
            .expect_err("expected api error");

    assert!(matches!(err, OllamaError::Api(_)));
}

#[tokio::test]
async fn pull_model_maps_http_errors() {
    let server = common::mock_ollama::MockOllamaServer::spawn_once(500, r#"{"error":"boom"}"#);
    let client = reqwest::Client::new();

    let err = olly::ollama::models::pull_model(&client, &server.base_url, "llama3.2:latest")
        .await
        .expect_err("expected api error");

    assert!(matches!(err, OllamaError::Api(_)));
}

#[tokio::test]
async fn delete_model_maps_http_errors() {
    let server = common::mock_ollama::MockOllamaServer::spawn_once(500, r#"{"error":"boom"}"#);
    let client = reqwest::Client::new();

    let err = olly::ollama::models::delete_model(&client, &server.base_url, "llama3.2:latest")
        .await
        .expect_err("expected api error");

    assert!(matches!(err, OllamaError::Api(_)));
}
