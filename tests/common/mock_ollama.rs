use std::{
    io::{Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
};

#[derive(Debug)]
pub struct MockOllamaServer {
    pub base_url: String,
    worker: Option<JoinHandle<()>>,
}

impl MockOllamaServer {
    pub fn spawn_once(status_code: u16, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("read local addr");
        let body = body.to_string();

        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept client");
            let mut request_buffer = [0_u8; 2048];
            let _ = stream.read(&mut request_buffer);

            let status_text = match status_code {
                200 => "OK",
                404 => "Not Found",
                500 => "Internal Server Error",
                _ => "Mock",
            };
            let response = format!(
                "HTTP/1.1 {status_code} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write mock response");
        });

        Self {
            base_url: format!("http://{}", addr),
            worker: Some(worker),
        }
    }
}

impl Drop for MockOllamaServer {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn example_chat_stream_chunk() -> &'static str {
    r#"{"model":"demo","message":{"role":"assistant","content":"hello"},"done":false}"#
}
