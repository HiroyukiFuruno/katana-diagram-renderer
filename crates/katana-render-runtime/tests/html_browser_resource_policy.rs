use katana_render_runtime::{
    HtmlBrowserFrame, HtmlBrowserProcessConfig, HtmlBrowserSession, HtmlBrowserSource,
    HtmlBrowserViewport,
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

type TestResult<T = ()> = Result<T, String>;
const HTTP_RESOURCE_SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
static CHROMIUM_SESSION_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn chromium_child_allows_http_same_origin_and_blocks_redirected_iframe_cross_origin_resources()
-> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let blocked = blocked_resource_server()?;
    let blocked_script = format!("{}/blocked.js", blocked.origin());
    let blocked_frame = format!("{}/frame.html", blocked.origin());
    let allowed = allowed_resource_server(blocked_script)?;
    let raw_html = browser_page_with_cross_origin_targets(&blocked_frame);

    assert_browser_policy_frame(&allowed, &raw_html)
}

fn blocked_resource_server() -> TestResult<TestHttpServer> {
    TestHttpServer::start(|path| match path {
        "/blocked.js" => HttpResponse::javascript(
            "document.querySelector('#pixel').style.background='rgb(119,136,153)'",
        ),
        "/frame.html" => HttpResponse::html(
            "<!doctype html><style>html,body{margin:0;background:rgb(119,136,153)}</style>",
        ),
        _ => HttpResponse::not_found(),
    })
}

fn allowed_resource_server(blocked_script: String) -> TestResult<TestHttpServer> {
    TestHttpServer::start(move |path| match path {
        "/allowed.css" => HttpResponse::css(
            "html,body,#pixel{margin:0;width:100%;height:100%;background:rgb(17,34,51)}",
        ),
        "/redirect.js" => HttpResponse::redirect(&blocked_script),
        _ => HttpResponse::not_found(),
    })
}

fn browser_page_with_cross_origin_targets(blocked_frame: &str) -> String {
    format!(
        r#"<!doctype html>
<link rel="stylesheet" href="/allowed.css">
<div id="pixel"></div>
<script src="/redirect.js"></script>
<iframe style="position:absolute;left:0;top:0;width:8px;height:8px;border:0" src="{blocked_frame}"></iframe>
"#
    )
}

fn assert_browser_policy_frame(allowed: &TestHttpServer, raw_html: &str) -> TestResult {
    let mut session = start_session(raw_html, format!("{}/index.html", allowed.origin()))?;
    wait_for_frame_rgb(&mut session, [17, 34, 51])?;
    let frame = latest_frame(&session)?;
    assert_frame_excludes_rgb(frame, [119, 136, 153])?;
    session.close().map_err(|error| error.to_string())
}

fn start_session(raw_html: &str, origin: impl Into<String>) -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(raw_html, origin).map_err(|error| error.to_string())?;
    let config = browser_process_config()?;
    HtmlBrowserSession::start(source, viewport()?, &config).map_err(|error| error.to_string())
}

fn chromium_session_guard() -> TestResult<MutexGuard<'static, ()>> {
    CHROMIUM_SESSION_LOCK
        .lock()
        .map_err(|error| error.to_string())
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(32, 32, 1.0).map_err(|error| error.to_string())
}

fn latest_frame(session: &HtmlBrowserSession) -> TestResult<&HtmlBrowserFrame> {
    session
        .latest_frame()
        .ok_or_else(|| "browser session did not return a frame".to_string())
}

fn wait_for_frame_rgb(session: &mut HtmlBrowserSession, rgb: [u8; 3]) -> TestResult {
    let deadline = Instant::now() + HTTP_RESOURCE_SETTLE_TIMEOUT;
    loop {
        session.refresh_frame().map_err(|error| error.to_string())?;
        if frame_contains_rgb(latest_frame(session)?, rgb) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return assert_frame_contains_rgb(latest_frame(session)?, rgb);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn assert_frame_contains_rgb(frame: &HtmlBrowserFrame, rgb: [u8; 3]) -> TestResult {
    if frame_contains_rgb(frame, rgb) {
        Ok(())
    } else {
        Err(format!(
            "frame did not contain rgb({},{},{})",
            rgb[0], rgb[1], rgb[2]
        ))
    }
}

fn assert_frame_excludes_rgb(frame: &HtmlBrowserFrame, rgb: [u8; 3]) -> TestResult {
    if frame_contains_rgb(frame, rgb) {
        Err(format!(
            "frame unexpectedly contained rgb({},{},{})",
            rgb[0], rgb[1], rgb[2]
        ))
    } else {
        Ok(())
    }
}

fn frame_contains_rgb(frame: &HtmlBrowserFrame, rgb: [u8; 3]) -> bool {
    frame
        .pixels
        .chunks_exact(4)
        .any(|pixel| pixel[0] == rgb[0] && pixel[1] == rgb[1] && pixel[2] == rgb[2])
}

struct TestHttpServer {
    origin: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestHttpServer {
    fn start(handler: impl Fn(&str) -> HttpResponse + Send + Sync + 'static) -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let origin = format!(
            "http://{}",
            listener.local_addr().map_err(|error| error.to_string())?
        );
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handler = Arc::new(handler);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => handle_connection(stream, handler.as_ref()),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            origin,
            stop,
            handle: Some(handle),
        })
    }

    fn origin(&self) -> &str {
        &self.origin
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct HttpResponse {
    status: &'static str,
    content_type: &'static str,
    body: String,
    location: Option<String>,
}

impl HttpResponse {
    fn css(body: impl Into<String>) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/css",
            body: body.into(),
            location: None,
        }
    }

    fn html(body: impl Into<String>) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/html",
            body: body.into(),
            location: None,
        }
    }

    fn javascript(body: impl Into<String>) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/javascript",
            body: body.into(),
            location: None,
        }
    }

    fn redirect(location: &str) -> Self {
        Self {
            status: "302 Found",
            content_type: "text/plain",
            body: String::new(),
            location: Some(location.to_owned()),
        }
    }

    fn not_found() -> Self {
        Self {
            status: "404 Not Found",
            content_type: "text/plain",
            body: "not found".to_string(),
            location: None,
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    handler: &(dyn Fn(&str) -> HttpResponse + Send + Sync),
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buffer = [0; 2048];
    let Ok(count) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..count]);
    let path = request_path(&request);
    let response = handler(&path);
    let body = response.body.as_bytes();
    let location = response
        .location
        .as_ref()
        .map(|value| format!("Location: {value}\r\n"))
        .unwrap_or_default();
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.content_type,
        location,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn request_path(request: &str) -> String {
    let first_line = request.lines().next().unwrap_or_default();
    let target = first_line.split_whitespace().nth(1).unwrap_or("/");
    target.split('?').next().unwrap_or(target).to_string()
}

fn browser_process_config() -> TestResult<HtmlBrowserProcessConfig> {
    Ok(
        HtmlBrowserProcessConfig::new(env!("CARGO_BIN_EXE_krr-html-chromium-engine").into())
            .with_chromium_binary(test_chromium_binary()?),
    )
}

#[cfg(target_os = "macos")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    chromium_candidate([
        bundled_chromium_binary()?,
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from(
            "/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        ),
    ])
}

#[cfg(target_os = "linux")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    chromium_candidate([
        bundled_chromium_binary()?,
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
    ])
}

#[cfg(target_os = "windows")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    let mut candidates = vec![bundled_chromium_binary()?];
    for base in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
        if let Some(root) = std::env::var_os(base) {
            candidates.push(PathBuf::from(root).join("Google/Chrome/Application/chrome.exe"));
        }
    }
    chromium_candidate(candidates)
}

fn chromium_candidate(candidates: impl IntoIterator<Item = PathBuf>) -> TestResult<PathBuf> {
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| "test Chromium binary was not found in known install locations".to_string())
}

fn bundled_chromium_binary() -> TestResult<PathBuf> {
    let helper = PathBuf::from(env!("CARGO_BIN_EXE_krr-html-chromium-engine"));
    let directory = helper
        .parent()
        .ok_or_else(|| "browser helper test binary has no parent directory".to_string())?;
    Ok(directory.join(bundled_chromium_relative_path()))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn bundled_chromium_relative_path() -> &'static str {
    "chromium/mac-arm64/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn bundled_chromium_relative_path() -> &'static str {
    "chromium/mac-x64/chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bundled_chromium_relative_path() -> &'static str {
    "chromium/linux64/chrome-linux64/chrome"
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn bundled_chromium_relative_path() -> &'static str {
    "chromium/win64/chrome-win64/chrome.exe"
}

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
fn bundled_chromium_relative_path() -> &'static str {
    "chromium/unsupported/chrome"
}
