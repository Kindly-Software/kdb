//! kindly-video-server: Static file server for kindly.software
//!
//! Uses pure std::net for serving pre-built WASM bundle.
//! SPA routing with index.html fallback.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const PORT: u16 = 8081;
const DIST_DIR: &str = "/home/samuel/Primitives/kindly-video/dist";

fn main() {
    println!("[kindly-video] Starting server on port {}", PORT);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT))
        .expect("Failed to bind to port");

    println!("[kindly-video] Serving {} on http://0.0.0.0:{}", DIST_DIR, PORT);

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            handle_connection(stream);
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 4096];
    let bytes_read = stream.read(&mut buffer).unwrap_or(0);

    if bytes_read == 0 {
        return;
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let path = parse_request_path(&request);

    serve_file(&mut stream, &path);
}

fn parse_request_path(request: &str) -> String {
    request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .map(|p| if p == "/" { "/index.html" } else { p })
        .unwrap_or("/index.html")
        .to_string()
}

fn serve_file(stream: &mut TcpStream, path: &str) {
    let file_path = format!("{}{}", DIST_DIR, path);

    match fs::read(&file_path) {
        Ok(contents) => {
            let mime = detect_mime(path);
            let response = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {}\r\n\
                 Content-Length: {}\r\n\
                 Cache-Control: public, max-age=31536000\r\n\
                 \r\n",
                mime,
                contents.len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&contents);
        }
        Err(_) => {
            // SPA fallback: serve index.html for all routes
            if let Ok(index) = fs::read(format!("{}/index.html", DIST_DIR)) {
                let response = format!(
                    "HTTP/1.1 200 OK\r\n\
                     Content-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\n\
                     \r\n",
                    index.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&index);
            } else {
                let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                let _ = stream.write_all(response.as_bytes());
            }
        }
    }
}

fn detect_mime(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}
