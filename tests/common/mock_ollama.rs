use std::{
    io::{Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
    time::Duration,
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

    pub fn spawn_chunked(chunks: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("read local addr");
        let chunk_list = chunks
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();

        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept client");
            let mut request_buffer = [0_u8; 2048];
            let _ = stream.read(&mut request_buffer);

            let headers = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
            stream
                .write_all(headers.as_bytes())
                .expect("write chunked headers");

            for chunk in chunk_list {
                let bytes = chunk.as_bytes();
                let prefix = format!("{:X}\r\n", bytes.len());
                stream
                    .write_all(prefix.as_bytes())
                    .expect("write chunk length");
                stream.write_all(bytes).expect("write chunk body");
                stream.write_all(b"\r\n").expect("write chunk terminator");
                stream.flush().expect("flush chunk");
                thread::sleep(Duration::from_millis(5));
            }

            stream.write_all(b"0\r\n\r\n").expect("write final chunk");
            stream.flush().expect("flush final chunk");
        });

        Self {
            base_url: format!("http://{}", addr),
            worker: Some(worker),
        }
    }

    pub fn spawn_delayed_once(status_code: u16, body: &str, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("read local addr");
        let body = body.to_string();

        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept client");
            let mut request_buffer = [0_u8; 2048];
            let _ = stream.read(&mut request_buffer);
            thread::sleep(delay);

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
                .expect("write delayed mock response");
            stream.flush().expect("flush delayed response");
        });

        Self {
            base_url: format!("http://{}", addr),
            worker: Some(worker),
        }
    }

    pub fn spawn_chunked_disconnect(chunks: &[&str], chunks_to_send: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
        let addr = listener.local_addr().expect("read local addr");
        let chunk_list = chunks
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();

        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept client");
            let mut request_buffer = [0_u8; 2048];
            let _ = stream.read(&mut request_buffer);

            let headers = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
            stream
                .write_all(headers.as_bytes())
                .expect("write chunked headers");

            for chunk in chunk_list.into_iter().take(chunks_to_send) {
                let bytes = chunk.as_bytes();
                let prefix = format!("{:X}\r\n", bytes.len());
                stream
                    .write_all(prefix.as_bytes())
                    .expect("write chunk length");
                stream.write_all(bytes).expect("write chunk body");
                stream.write_all(b"\r\n").expect("write chunk terminator");
                stream.flush().expect("flush chunk");
                thread::sleep(Duration::from_millis(5));
            }

            // Intentionally close early without the final 0-sized chunk to simulate disconnect.
            stream.flush().expect("flush abrupt close");
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

pub fn stream_line(content: &str, done: bool) -> String {
    format!(
        r#"{{"model":"demo","message":{{"role":"assistant","content":"{content}"}},"done":{done}}}"#
    )
}
