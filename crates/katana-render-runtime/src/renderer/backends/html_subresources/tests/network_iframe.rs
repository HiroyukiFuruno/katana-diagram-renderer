use super::support::{TestResult, assert_frame_contains, to_string, viewport};
use crate::renderer::backends::html_subresources::HtmlSubresourcePolicy;
use crate::renderer::backends::{HtmlBrowserSource, HtmlRuntime};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};

const FRAME_HTML: &str = r#"<style>
html, body { margin: 0; }
#next { width: 160px; height: 100px; background: #ef4444; }
</style>
<button id=next>Waiting</button>
<script>
document.getElementById('next').addEventListener('click', function () {
  this.textContent = 'Loaded';
  this.style.backgroundColor = '#35a853';
});
</script>"#;

const WRAPPER_HTML: &str = r#"<style>
html, body, iframe { width: 100%; height: 100%; margin: 0; border: 0; }
</style>
<iframe id=deck src=frame.html></iframe>
<script>
document.getElementById('deck').addEventListener('load', function () {
  this.contentDocument.getElementById('next').click();
});
</script>"#;

#[test]
fn same_origin_network_iframe_joins_dom_css_javascript_and_load_event() -> TestResult {
    let (origin, server) = iframe_server()?;
    let source =
        HtmlBrowserSource::new(WRAPPER_HTML, format!("{origin}/index.html")).map_err(to_string)?;
    let frame = HtmlRuntime
        .open(source, viewport()?)
        .map_err(to_string)?
        .latest_frame()
        .cloned()
        .ok_or("frame is missing")?;
    server
        .join()
        .map_err(|_| "iframe server panicked".to_string())?
        .map_err(to_string)?;

    assert_frame_contains(&frame.pixels, [53, 168, 83]);
    Ok(())
}

#[test]
fn iframe_policy_allows_same_origin_network_and_rejects_cross_origin() -> TestResult {
    let source = HtmlBrowserSource::new("<p>main</p>", "https://docs.example/guide/index.html")
        .map_err(to_string)?;
    let policy = HtmlSubresourcePolicy::from_source(&source);

    assert_eq!(
        policy
            .resolve_iframe("../frame.html")
            .map_err(to_string)?
            .as_str(),
        "https://docs.example/frame.html"
    );
    assert!(
        policy
            .resolve_iframe("https://other.example/frame.html")
            .is_err()
    );
    assert!(policy.resolve_iframe("data:text/html,frame").is_err());
    assert!(policy.resolve_iframe("file:///tmp/frame.html").is_err());
    Ok(())
}

fn iframe_server() -> TestResult<(String, std::thread::JoinHandle<std::io::Result<()>>)> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(to_string)?;
    let address = listener.local_addr().map_err(to_string)?;
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let mut request = [0_u8; 1024];
        let length = stream.read(&mut request)?;
        let request = std::str::from_utf8(&request[..length]).map_err(std::io::Error::other)?;
        if request.split_ascii_whitespace().nth(1) != Some("/frame.html") {
            return Err(std::io::Error::other("unexpected iframe request"));
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            FRAME_HTML.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(FRAME_HTML.as_bytes())?;
        stream.flush()?;
        stream.shutdown(Shutdown::Write)
    });
    Ok((format!("http://{address}"), server))
}
