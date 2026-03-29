mod common;

use olly::ollama::{chat::ChatMessage, error::OllamaError};

#[test]
fn parse_line_with_unknown_extra_fields_succeeds() {
    let line = r#"{"model":"demo","message":{"role":"assistant","content":"hello","extra":1},"done":false,"mystery":"field"}"#;
    let parsed = olly::ollama::chat::parse_stream_line(line)
        .expect("parse line")
        .expect("chunk should exist");
    assert_eq!(parsed.content, "hello");
    assert!(!parsed.done);
}

#[test]
fn parse_line_with_tool_calls_missing_function_degrades_gracefully() {
    let line = r#"{"model":"demo","message":{"role":"assistant","content":"","tool_calls":[{}]},"done":false}"#;
    let parsed = olly::ollama::chat::parse_stream_line(line)
        .expect("parse line")
        .expect("chunk should exist");
    assert_eq!(parsed.tool_calls.len(), 1);
    assert!(parsed.tool_calls[0].function.is_none());
}

#[test]
fn parse_line_with_unicode_content_roundtrips() {
    let line = r#"{"model":"demo","message":{"role":"assistant","content":"Salut, lume! 你好"},"done":false}"#;
    let parsed = olly::ollama::chat::parse_stream_line(line)
        .expect("parse line")
        .expect("chunk should exist");
    assert_eq!(parsed.content, "Salut, lume! 你好");
}

#[test]
fn parse_line_with_very_long_content_stays_stable() {
    let long = "x".repeat(32_768);
    let line = format!(
        r#"{{"model":"demo","message":{{"role":"assistant","content":"{}"}},"done":false}}"#,
        long
    );
    let parsed = olly::ollama::chat::parse_stream_line(&line)
        .expect("parse line")
        .expect("chunk should exist");
    assert_eq!(parsed.content.len(), 32_768);
    assert!(parsed.content.chars().all(|ch| ch == 'x'));
}

#[test]
fn parse_done_true_with_empty_message_content() {
    let line = r#"{"model":"demo","message":{"role":"assistant","content":""},"done":true}"#;
    let parsed = olly::ollama::chat::parse_stream_line(line)
        .expect("parse line")
        .expect("chunk should exist");
    assert!(parsed.done);
    assert_eq!(parsed.content, "");
}

#[tokio::test]
async fn mixed_newline_styles_are_handled_in_stream() {
    let l1 = common::mock_ollama::stream_line("alpha", false);
    let l2 = common::mock_ollama::stream_line("beta", true);
    let body = format!("{l1}\r\n{l2}\n");
    let server = common::mock_ollama::MockOllamaServer::spawn_once(200, &body);
    let client = reqwest::Client::new();
    let messages = vec![ChatMessage::new("user", "hello")];
    let mut chunks = Vec::new();

    olly::ollama::chat::chat_stream(
        &client,
        &server.base_url,
        "llama3.2:latest",
        &messages,
        |chunk| chunks.push((chunk.content.clone(), chunk.done)),
    )
    .await
    .expect("stream should succeed");

    assert_eq!(
        chunks,
        vec![(String::from("alpha"), false), (String::from("beta"), true)]
    );
}

#[tokio::test]
async fn interleaved_blank_lines_are_ignored_safely() {
    let l1 = common::mock_ollama::stream_line("one", false);
    let l2 = common::mock_ollama::stream_line("two", true);
    let body = format!("\n\r\n{l1}\n\n\n{l2}\n\n");
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

    assert_eq!(received, vec![String::from("one"), String::from("two")]);
}

#[test]
fn bounded_randomized_malformed_lines_never_panic() {
    // Deterministic xorshift sequence for reproducible malformed payload generation.
    let mut seed = 0xA17E_C0DE_u64;
    for _ in 0..128 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let pick = (seed % 6) as u8;
        let line = match pick {
            0 => String::from("{"),
            1 => String::from("[]"),
            2 => String::from(r#"{"done":"not-bool"}"#),
            3 => format!(
                r#"{{"message":{{"content":"{}"}}}}"#,
                "x".repeat((seed % 32) as usize)
            ),
            4 => String::from(r#"{"message":null,"done":false}"#),
            _ => String::from(r#"{"model":"demo","message":{"role":"assistant"},"done":false}"#),
        };
        let result = std::panic::catch_unwind(|| olly::ollama::chat::parse_stream_line(&line));
        assert!(result.is_ok(), "parse_stream_line panicked on line: {line}");
        let parsed = result.expect("catch_unwind result");
        if let Err(err) = parsed {
            assert!(
                matches!(err, OllamaError::Json(_)),
                "unexpected error type for malformed input"
            );
        }
    }
}
