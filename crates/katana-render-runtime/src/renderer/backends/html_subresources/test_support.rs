use crate::renderer::backends::{HtmlBrowserSource, HtmlBrowserViewport};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use url::Url;

pub(super) type TestResult<T = ()> = Result<T, String>;

static NEXT_FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

pub(super) fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)
}

pub(super) fn assert_frame_contains(pixels: &[u8], expected: [u8; 3]) {
    assert!(
        pixels.chunks_exact(4).any(|pixel| pixel[..3] == expected),
        "frame does not contain {expected:?}"
    );
}

pub(super) fn local_resource_server()
-> TestResult<(String, std::thread::JoinHandle<std::io::Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    let server = std::thread::spawn(move || serve_resources(listener));
    Ok((format!("http://{address}"), server))
}

fn serve_resources(listener: TcpListener) -> std::io::Result<()> {
    for _ in 0..2 {
        let (mut stream, _) = listener.accept()?;
        let path = request_path(&mut stream)?;
        let body = resource_body(&path)?;
        write_response(&mut stream, body.as_bytes())?;
    }
    Ok(())
}

fn resource_body(path: &str) -> std::io::Result<&'static str> {
    match path {
        "/style.css" => Ok("#styled { background: #10b981; width: 80px; height: 30px; }"),
        "/app.js" => Ok("document.getElementById('scripted').style.backgroundColor = '#ef4444';"),
        _ => Err(std::io::Error::other("unexpected subresource request")),
    }
}

fn request_path(stream: &mut std::net::TcpStream) -> std::io::Result<String> {
    let mut request = [0_u8; 1024];
    let length = stream.read(&mut request)?;
    let request = std::str::from_utf8(&request[..length]).map_err(std::io::Error::other)?;
    request
        .split_ascii_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or_else(missing_request_path)
}

fn missing_request_path() -> std::io::Error {
    std::io::Error::other("request path is missing")
}

fn write_response(stream: &mut std::net::TcpStream, body: &[u8]) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)
}

pub(super) struct LocalFixture {
    pub(super) root: PathBuf,
}

impl LocalFixture {
    pub(super) fn new() -> TestResult<Self> {
        let root = temp_root();
        std::fs::create_dir_all(&root).map_err(to_string)?;
        std::fs::write(
            root.join("style.css"),
            "#styled { background: #10b981; width: 80px; height: 30px; }",
        )
        .map_err(to_string)?;
        std::fs::write(
            root.join("app.js"),
            "document.getElementById('scripted').style.backgroundColor = '#ef4444';",
        )
        .map_err(to_string)?;
        std::fs::write(root.join("pixel.svg"), "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><rect width='8' height='8' fill='#3182ce'/></svg>")
            .map_err(to_string)?;
        std::fs::write(root.join("index.html"), "<!doctype html>").map_err(to_string)?;
        Ok(Self { root })
    }

    pub(super) fn origin(&self) -> TestResult<String> {
        Url::from_file_path(self.root.join("index.html"))
            .map(|url| url.to_string())
            .map_err(file_url_error)
    }

    pub(super) fn source(&self, raw_html: &str) -> TestResult<HtmlBrowserSource> {
        HtmlBrowserSource::new(raw_html, self.origin()?).map_err(to_string)
    }

    pub(super) fn html(&self) -> &'static str {
        "<link rel=stylesheet href=style.css><div id=styled>Styled</div><div id=scripted style='width:80px;height:30px'>Scripted</div><script src=app.js></script><img src=pixel.svg style='width:40px;height:40px'>"
    }
}

fn file_url_error(_: ()) -> String {
    "fixture path is not a file URL".to_string()
}

impl Drop for LocalFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn temp_root() -> PathBuf {
    let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("krr-html-resources-{}-{id}", std::process::id()))
}

pub(super) fn to_string(error: impl ToString) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{file_url_error, missing_request_path, resource_body, to_string};
    use crate::renderer::backends::html_browser::HtmlBrowserError;
    use url::Url;

    #[test]
    fn support_helpers_report_unexpected_resources_and_errors() {
        assert!(resource_body("/missing.css").is_err());
        assert_eq!(to_string("fixture failure"), "fixture failure");
        assert_eq!(to_string("owned failure".to_string()), "owned failure");
        assert_eq!(to_string(std::io::Error::other("io failure")), "io failure");
        assert_eq!(
            to_string(HtmlBrowserError::InvalidViewport),
            "browser viewport dimensions must be non-zero"
        );
        let mut parse_messages = Vec::new();
        if let Some(error) = Url::parse("http://[").err() {
            parse_messages.push(to_string(error));
        }
        assert_eq!(parse_messages, vec!["invalid IPv6 address"]);
        assert_eq!(
            missing_request_path().to_string(),
            "request path is missing"
        );
        assert_eq!(file_url_error(()), "fixture path is not a file URL");
    }
}
