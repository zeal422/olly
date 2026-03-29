mod common;

use std::time::Duration;

use olly::ollama::{chat::ChatMessage, error::OllamaError};

fn timeout_client(timeout_ms: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .expect("build reqwest client with timeout")
}

#[tokio::test]
async fn chat_once_timeout_maps_to_network_error() {
    let server = common::mock_ollama::MockOllamaServer::spawn_delayed_once(
        200,
        r#"{"message":{"role":"assistant","content":"slow"},"done":true}"#,
        Duration::from_millis(200),
    );
    let client = timeout_client(40);
    let messages = vec![ChatMessage::new("user", "hello")];

    let err =
        olly::ollama::chat::chat_once(&client, &server.base_url, "llama3.2:latest", &messages)
            .await
            .expect_err("chat_once should timeout");
    assert!(matches!(err, OllamaError::Network(_)));
}

#[tokio::test]
async fn chat_stream_timeout_maps_to_network_error() {
    let body = format!("{}\n", common::mock_ollama::stream_line("slow", true));
    let server = common::mock_ollama::MockOllamaServer::spawn_delayed_once(
        200,
        &body,
        Duration::from_millis(220),
    );
    let client = timeout_client(40);
    let messages = vec![ChatMessage::new("user", "hello")];

    let err = olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |_| {},
    )
    .await
    .expect_err("chat_stream should timeout");
    assert!(matches!(err, OllamaError::Network(_)));
}

#[tokio::test]
async fn chat_stream_disconnect_after_first_chunk_is_controlled() {
    let chunks = [
        format!("{}\n", common::mock_ollama::stream_line("alpha", false)),
        format!("{}\n", common::mock_ollama::stream_line("beta", true)),
    ];
    let chunk_refs = chunks.iter().map(String::as_str).collect::<Vec<_>>();
    let server = common::mock_ollama::MockOllamaServer::spawn_chunked_disconnect(&chunk_refs, 1);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];
    let mut received = Vec::new();

    let result = olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push(chunk.content.clone());
        },
    )
    .await;

    assert_eq!(received, vec![String::from("alpha")]);
    if let Err(err) = result {
        assert!(matches!(err, OllamaError::Network(_)));
    }
}

#[tokio::test]
async fn chat_stream_disconnect_before_any_chunk_is_controlled() {
    let server = common::mock_ollama::MockOllamaServer::spawn_chunked_disconnect(&[], 0);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];
    let mut received = Vec::new();

    let result = olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push(chunk.content.clone());
        },
    )
    .await;

    assert!(received.is_empty());
    if let Err(err) = result {
        assert!(matches!(err, OllamaError::Network(_)));
    }
}

#[tokio::test]
async fn chat_stream_malformed_chunk_after_valid_chunk_returns_json_error() {
    let valid = common::mock_ollama::stream_line("ok", false);
    let body = format!("{valid}\n{{bad-json\n");
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, &body);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];
    let mut received = Vec::new();

    let err = olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push(chunk.content.clone());
        },
    )
    .await
    .expect_err("malformed chunk should fail");

    assert!(matches!(err, OllamaError::Json(_)));
    assert_eq!(received, vec![String::from("ok")]);
}

#[tokio::test]
async fn list_models_timeout_maps_to_network_error() {
    let server = common::mock_ollama::MockOllamaServer::spawn_delayed_once(
        200,
        r#"{"models":[{"name":"qwen2.5-coder:7b"}]}"#,
        Duration::from_millis(200),
    );
    let client = timeout_client(40);

    let err = olly::ollama::models::list_models(&client, &server.base_url)
        .await
        .expect_err("list_models should timeout");
    assert!(matches!(err, OllamaError::Network(_)));
}

#[tokio::test]
async fn pull_model_timeout_maps_to_network_error() {
    let server = common::mock_ollama::MockOllamaServer::spawn_delayed_once(
        200,
        r#"{"status":"ok"}"#,
        Duration::from_millis(200),
    );
    let client = timeout_client(40);

    let err = olly::ollama::models::pull_model(&client, &server.base_url, "qwen2.5-coder:7b")
        .await
        .expect_err("pull_model should timeout");
    assert!(matches!(err, OllamaError::Network(_)));
}

#[tokio::test]
async fn delete_model_timeout_maps_to_network_error() {
    let server = common::mock_ollama::MockOllamaServer::spawn_delayed_once(
        200,
        r#"{"status":"ok"}"#,
        Duration::from_millis(200),
    );
    let client = timeout_client(40);

    let err = olly::ollama::models::delete_model(&client, &server.base_url, "qwen2.5-coder:7b")
        .await
        .expect_err("delete_model should timeout");
    assert!(matches!(err, OllamaError::Network(_)));
}

#[tokio::test]
async fn slow_server_within_timeout_budget_succeeds() {
    let server = common::mock_ollama::MockOllamaServer::spawn_delayed_once(
        200,
        r#"{"message":{"role":"assistant","content":"ok"},"done":true}"#,
        Duration::from_millis(80),
    );
    let client = timeout_client(500);
    let messages = vec![ChatMessage::new("user", "hello")];

    let response =
        olly::ollama::chat::chat_once(&client, &server.base_url, "llama3.2:latest", &messages)
            .await
            .expect("chat_once should succeed");
    assert_eq!(response, "ok");
}

#[tokio::test]
async fn done_chunk_stops_callback_sequence_without_duplicates() {
    let chunks = [
        format!("{}\n", common::mock_ollama::stream_line("done", true)),
        format!("{}\n", common::mock_ollama::stream_line("ignored", true)),
    ];
    let chunk_refs = chunks.iter().map(String::as_str).collect::<Vec<_>>();
    let server = common::mock_ollama::MockOllamaServer::spawn_chunked_disconnect(&chunk_refs, 2);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];
    let mut received = Vec::new();

    olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| received.push((chunk.content.clone(), chunk.done)),
    )
    .await
    .expect("stream should return after first done chunk");

    assert_eq!(received, vec![(String::from("done"), true)]);
}
