mod common;

use olly::ollama::chat::ChatMessage;

#[tokio::test]
async fn chat_stream_handles_multiple_lines_in_single_chunk() {
    let first = common::mock_ollama::stream_line("hello", false);
    let second = common::mock_ollama::stream_line("world", true);
    let body = format!("{first}\n{second}\n");
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, &body);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "demo")];
    let mut received = Vec::new();

    olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push((chunk.content.clone(), chunk.done));
        },
    )
    .await
    .expect("stream should succeed");

    assert_eq!(
        received,
        vec![
            (String::from("hello"), false),
            (String::from("world"), true)
        ]
    );
}

#[tokio::test]
async fn chat_stream_handles_line_split_across_chunks() {
    let chunks = vec![
        r#"{"model":"demo","message":{"role":"assistant","content":"hel"#,
        r#"lo"},"done":false}"#,
        "\n",
        r#"{"model":"demo","message":{"role":"assistant","content":"bye"},"done":true}"#,
        "\n",
    ];
    let server = common::mock_ollama::MockOllamaServer::spawn_chunked(&chunks);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "demo")];
    let mut received = Vec::new();

    olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push((chunk.content.clone(), chunk.done));
        },
    )
    .await
    .expect("stream should succeed");

    assert_eq!(
        received,
        vec![(String::from("hello"), false), (String::from("bye"), true)]
    );
}

#[tokio::test]
async fn chat_stream_stops_processing_after_done_true() {
    let first = common::mock_ollama::stream_line("first", false);
    let done = common::mock_ollama::stream_line("done", true);
    let ignored = common::mock_ollama::stream_line("ignored", false);
    let body = format!("{first}\n{done}\n{ignored}\n");
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, &body);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "demo")];
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

    assert_eq!(received, vec![String::from("first"), String::from("done")]);
}

#[tokio::test]
async fn chat_stream_processes_trailing_buffer_without_final_newline() {
    let body = common::mock_ollama::stream_line("tail", false);
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, &body);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "demo")];
    let mut received = Vec::new();

    olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| {
            received.push((chunk.content.clone(), chunk.done));
        },
    )
    .await
    .expect("stream should succeed");

    assert_eq!(received, vec![(String::from("tail"), false)]);
}
