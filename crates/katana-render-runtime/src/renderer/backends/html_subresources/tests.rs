use super::{HtmlSubresourceLoader, HtmlSubresourcePolicy};
use crate::renderer::backends::{HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use url::Url;

type TestResult<T = ()> = Result<T, String>;

static NEXT_FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

#[test]
fn local_resources_feed_css_v8_and_image_layout() -> TestResult {
    let fixture = LocalFixture::new()?;
    let source = HtmlBrowserSource::new(fixture.html(), fixture.origin()?).map_err(to_string)?;
    let mut document =
        crate::renderer::backends::html_document::HtmlDocument::parse(&source.raw_html);
    let resources = HtmlSubresourceLoader::new(&source)
        .load(&mut document)
        .map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;

    assert!(resources.stylesheets.contains_key("style.css"));
    assert!(
        resources
            .scripts
            .iter()
            .any(|script| script.contains("scripted"))
    );
    assert!(document.render().contains("data:image/svg+xml;base64,"));
    assert_frame_contains(&frame.pixels, [16, 185, 129]);
    assert_frame_contains(&frame.pixels, [239, 68, 68]);
    assert_frame_contains(&frame.pixels, [49, 130, 206]);
    Ok(())
}

#[test]
fn same_origin_http_resources_are_loaded_without_main_document_fetch() -> TestResult {
    let (origin, server) = local_resource_server()?;
    let source = HtmlBrowserSource::new(http_document(), format!("{origin}/index.html"))
        .map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;
    let joined = server
        .join()
        .map_err(|_| "resource server panicked".to_string())?;

    assert!(joined.is_ok());
    assert_frame_contains(&frame.pixels, [16, 185, 129]);
    assert_frame_contains(&frame.pixels, [239, 68, 68]);
    Ok(())
}

#[test]
fn policy_rejects_absolute_escape_cross_origin_and_iframe_resources() -> TestResult {
    let fixture = LocalFixture::new()?;
    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);

    assert_direct_reference_policy(&policy);
    assert_navigation_policy(&policy);
    assert_rejects_non_local_file_navigation(&policy)?;
    assert_policy_error(fixture.source("<iframe src=https://other.example></iframe>")?)?;
    assert_policy_error(fixture.source("<link rel=stylesheet href=../outside.css>")?)?;
    assert_policy_error(fixture.source("<script src=../outside.js></script>")?)?;
    assert_policy_error(fixture.source("<img src=../outside.svg>")?)?;
    Ok(())
}

fn assert_rejects_non_local_file_navigation(policy: &HtmlSubresourcePolicy) -> TestResult {
    let url = Url::parse("file://example.test/outside.html").map_err(to_string)?;

    assert!(!policy.allows_local_navigation(&url));
    Ok(())
}

fn assert_direct_reference_policy(policy: &HtmlSubresourcePolicy) {
    assert!(policy.resolve_subresource("../outside.css").is_err());
    assert!(policy.resolve_subresource("/absolute.css").is_err());
    assert!(policy.resolve_subresource("http://[").is_err());
    assert!(
        policy
            .resolve_subresource("https://other.example/style.css")
            .is_err()
    );
    assert!(
        policy
            .resolve_subresource("data:text/css,body%7B%7D")
            .is_ok()
    );
}

fn assert_navigation_policy(policy: &HtmlSubresourcePolicy) {
    assert!(
        policy
            .resolve_navigation("https://other.example/next.html")
            .is_err()
    );
    assert!(policy.resolve_navigation("next.html").is_ok());
    assert!(policy.resolve_navigation("guide/next.html").is_ok());
    assert!(policy.resolve_navigation("../outside.html").is_err());
}

#[cfg(unix)]
#[test]
fn policy_rejects_symlink_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let outside = fixture
        .root
        .parent()
        .ok_or("fixture parent is missing")?
        .join("outside.css");
    std::fs::write(&outside, "#target { color: red; }").map_err(to_string)?;
    std::os::unix::fs::symlink(&outside, fixture.root.join("escape.css")).map_err(to_string)?;

    assert_policy_error(fixture.source("<link rel=stylesheet href=escape.css>")?)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn policy_rejects_navigation_through_a_symlink_escape() -> TestResult {
    let fixture = LocalFixture::new()?;
    let outside = fixture.root.with_extension("outside");
    std::fs::create_dir_all(&outside).map_err(to_string)?;
    std::os::unix::fs::symlink(&outside, fixture.root.join("escape")).map_err(to_string)?;

    let policy = HtmlSubresourcePolicy::from_source(&fixture.source("<p>ok</p>")?);
    let rejected = policy.resolve_navigation("escape/next.html").is_err();
    std::fs::remove_dir_all(&outside).map_err(to_string)?;
    assert!(rejected);
    Ok(())
}

fn assert_policy_error(source: HtmlBrowserSource) -> TestResult {
    let error = HtmlRuntime.open(source, viewport()?);
    assert!(matches!(
        error,
        Err(error) if error.to_string().contains("HTML subresource error")
    ));
    Ok(())
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(320, 240, 1.0).map_err(to_string)
}

fn assert_frame_contains(pixels: &[u8], expected: [u8; 3]) {
    assert!(
        pixels.chunks_exact(4).any(|pixel| pixel[..3] == expected),
        "frame does not contain {expected:?}"
    );
}

fn local_resource_server() -> TestResult<(String, std::thread::JoinHandle<std::io::Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    let server = std::thread::spawn(move || serve_resources(listener));
    Ok((format!("http://{address}"), server))
}

fn serve_resources(listener: TcpListener) -> std::io::Result<()> {
    for _ in 0..2 {
        let (mut stream, _) = listener.accept()?;
        let path = request_path(&mut stream)?;
        let body = match path.as_str() {
            "/style.css" => "#styled { background: #10b981; width: 80px; height: 30px; }",
            "/app.js" => "document.getElementById('scripted').style.backgroundColor = '#ef4444';",
            _ => return Err(std::io::Error::other("unexpected subresource request")),
        };
        write_response(&mut stream, body.as_bytes())?;
    }
    Ok(())
}

fn request_path(stream: &mut std::net::TcpStream) -> std::io::Result<String> {
    let mut request = [0_u8; 1024];
    let length = stream.read(&mut request)?;
    let request = std::str::from_utf8(&request[..length]).map_err(std::io::Error::other)?;
    request
        .split_ascii_whitespace()
        .nth(1)
        .map(str::to_string)
        .ok_or_else(|| std::io::Error::other("request path is missing"))
}

fn write_response(stream: &mut std::net::TcpStream, body: &[u8]) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)
}

fn http_document() -> &'static str {
    "<link rel=stylesheet href=style.css><div id=styled>Styled</div><div id=scripted style='width:80px;height:30px'>Scripted</div><script src=app.js></script>"
}

struct LocalFixture {
    root: PathBuf,
}

impl LocalFixture {
    fn new() -> TestResult<Self> {
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

    fn origin(&self) -> TestResult<String> {
        Url::from_file_path(self.root.join("index.html"))
            .map(|url| url.to_string())
            .map_err(|_| "fixture path is not a file URL".to_string())
    }

    fn source(&self, raw_html: &str) -> TestResult<HtmlBrowserSource> {
        HtmlBrowserSource::new(raw_html, self.origin()?).map_err(to_string)
    }

    fn html(&self) -> &'static str {
        "<link rel=stylesheet href=style.css><div id=styled>Styled</div><div id=scripted style='width:80px;height:30px'>Scripted</div><script src=app.js></script><img src=pixel.svg style='width:40px;height:40px'>"
    }
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

fn to_string(error: impl ToString) -> String {
    error.to_string()
}
