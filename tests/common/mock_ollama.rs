#[derive(Debug, Clone)]
pub struct MockOllamaServer {
    pub base_url: String,
}

impl MockOllamaServer {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
}

pub fn example_chat_stream_chunk() -> &'static str {
    r#"{"model":"demo","message":{"role":"assistant","content":"hello"},"done":false}"#
}
