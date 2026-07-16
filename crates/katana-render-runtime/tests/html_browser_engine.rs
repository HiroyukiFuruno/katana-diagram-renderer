use katana_render_runtime::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserCommand, HtmlBrowserError, HtmlBrowserFrame,
    HtmlBrowserInput, HtmlBrowserNavigation, HtmlBrowserNavigationEvent, HtmlBrowserProcessConfig,
    HtmlBrowserRequest, HtmlBrowserResponse, HtmlBrowserSession, HtmlBrowserSource,
    HtmlBrowserViewport,
};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Mutex, MutexGuard, mpsc},
    thread,
    time::{Duration, Instant},
};
#[cfg(unix)]
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

type TestResult<T = ()> = Result<T, String>;
type ChildResponseReader = (
    thread::JoinHandle<()>,
    mpsc::Receiver<std::io::Result<String>>,
);
const CHILD_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);
static HTML_BROWSER_ENGINE_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static CHROMIUM_SESSION_LOCK: Mutex<()> = Mutex::new(());
#[cfg(unix)]
static FAKE_CHROMIUM_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn chromium_child_evaluates_inline_css_and_javascript_into_rgba_pixels() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#pixel { margin: 0; width: 100%; height: 100%; }</style><div id="pixel"></div><script>document.querySelector('#pixel').style.background = 'rgb(17, 34, 51)';</script>"#,
        "https://example.test/document.html",
        16,
        16,
    )?;

    let frame = latest_frame(&session)?;
    assert_eq!(frame.pixels.len(), 16 * 16 * 4);
    assert_frame_contains_rgb(frame, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_resolves_local_css_javascript_and_image_from_source_origin() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let origin = html_browser_fixture_origin()?;
    let mut session = start_session(
        r#"<!doctype html><link rel="stylesheet" href="resources/page.css"><img id="asset" src="resources/accent.svg"><div id="pixel"></div><script src="resources/action.js"></script>"#,
        origin,
        32,
        32,
    )?;

    assert_initial_local_resources(&session)?;
    dispatch_click(&mut session, 16.0, 16.0)?;
    assert_frame_contains_rgb(latest_frame(&session)?, [119, 136, 153])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_returns_link_navigation_to_the_source_host() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let origin = html_browser_fixture_origin()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,a{margin:0;width:100%;height:100%;display:block}</style><a href="next.html">next</a>"#,
        origin,
        32,
        32,
    )?;

    dispatch_click(&mut session, 16.0, 16.0)?;
    let navigation = take_navigation(&mut session)?;
    assert!(navigation.url.as_str().ends_with("/next.html"));
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_forwards_text_to_a_focused_html_form_control() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body{margin:0}input{width:32px;height:16px}#marker{position:absolute;top:16px;width:32px;height:16px;background:rgb(1,2,3)}</style><input id="field" autofocus><div id="marker"></div><script>const field=document.querySelector('#field');field.addEventListener('input',()=>{if(field.value==='ok')document.querySelector('#marker').style.background='rgb(17,34,51)'})</script>"#,
        "https://example.test/form.html",
        32,
        32,
    )?;

    dispatch_click(&mut session, 8.0, 8.0)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "ok".to_string(),
        })
        .map_err(|error| error.to_string())?;
    assert_frame_contains_rgb(latest_frame(&session)?, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_applies_surface_focus_before_text_input() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut blurred = start_session(
        focus_form_html(),
        "https://example.test/blurred.html",
        32,
        32,
    )?;
    assert_text_ignored_while_blurred(&mut blurred)?;
    blurred.close().map_err(|error| error.to_string())?;

    let mut focused = start_session(
        focus_form_html(),
        "https://example.test/focused.html",
        32,
        32,
    )?;
    assert_text_delivered_while_focused(&mut focused)?;
    focused.close().map_err(|error| error.to_string())
}

#[test]
fn browser_session_exposes_initial_and_action_frame_updates_once() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#marker{margin:0;width:100%;height:100%}#marker{background:rgb(1,2,3)}#action{position:absolute;inset:0;appearance:none;border:0;background:transparent;color:transparent;padding:0}</style><div id="marker"></div><button id="action">go</button><script>document.querySelector('#action').addEventListener('click',()=>{document.querySelector('#marker').style.background='rgb(17,34,51)'})</script>"#,
        "https://example.test/frame-update.html",
        16,
        16,
    )?;

    assert_frame_contains_rgb(take_frame_update(&mut session)?, [1, 2, 3])?;
    assert!(session.take_frame_update().is_none());
    dispatch_click(&mut session, 8.0, 8.0)?;
    assert_frame_contains_rgb(take_frame_update(&mut session)?, [17, 34, 51])?;
    assert!(session.take_frame_update().is_none());
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_refreshes_microtask_and_css_animation_frame_updates() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#marker{margin:0;width:100%;height:100%}#marker{background:rgb(1,2,3)}#action{position:absolute;inset:0;appearance:none;border:0;background:transparent;color:transparent;padding:0}.run #marker{animation:turn 40ms linear forwards}@keyframes turn{from{background:rgb(1,2,3)}to{background:rgb(17,34,51)}}</style><div id="marker"></div><button id="action">go</button><script>document.querySelector('#action').addEventListener('click',()=>{Promise.resolve().then(()=>document.body.classList.add('run'))})</script>"#,
        "https://example.test/animation.html",
        16,
        16,
    )?;

    dispatch_click(&mut session, 8.0, 8.0)?;
    wait_for_frame_rgb(&mut session, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_honors_prevent_default_without_kdv_navigation_semantics() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#link,#surface{margin:0;width:100%;height:100%}#link,#surface{display:block}#link{color:transparent}#surface{background:rgb(1,2,3)}</style><a id="link" href="next.html"><span id="surface"></span></a><script>document.querySelector('#link').addEventListener('click',event=>{event.preventDefault();document.querySelector('#surface').style.background='rgb(17,34,51)'})</script>"#,
        "https://example.test/prevent-default.html",
        16,
        16,
    )?;

    assert_frame_contains_rgb(latest_frame(&session)?, [1, 2, 3])?;
    dispatch_click(&mut session, 8.0, 8.0)?;
    assert!(session.take_navigation().is_none());
    wait_for_frame_rgb(&mut session, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_updates_the_viewport_after_scroll_and_resize() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body{margin:0}#top,#bottom{height:32px;width:100%}#top{background:rgb(17,34,51)}#bottom{background:rgb(68,85,102)}</style><div id="top"></div><div id="bottom"></div>"#,
        "https://example.test/scroll.html",
        32,
        32,
    )?;

    scroll_down_one_viewport(&mut session)?;
    assert_frame_contains_rgb(latest_frame(&session)?, [68, 85, 102])?;
    session
        .resize(viewport(24, 16)?)
        .map_err(|error| error.to_string())?;
    assert_resized_frame(latest_frame(&session)?)?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_navigates_an_existing_browser_session() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}#pixel{background:rgb(1,2,3)}</style><div id="pixel"></div>"#,
        "https://example.test/first.html",
        16,
        16,
    )?;
    let source = HtmlBrowserSource::new(
        r#"<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}#pixel{background:rgb(17,34,51)}</style><div id="pixel"></div>"#,
        "https://example.test/second.html",
    )
    .map_err(|error| error.to_string())?;

    session
        .navigate(HtmlBrowserNavigation::new(source).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    assert_frame_contains_rgb(latest_frame(&session)?, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_handles_pointer_move_keydown_and_ignored_pointer_up() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}#pixel{background:rgb(1,2,3)}</style><div id="pixel" tabindex="0"></div><script>document.addEventListener('keydown', event => { if (event.key === 'a') document.querySelector('#pixel').style.background='rgb(17,34,51)' })</script>"#,
        "https://example.test/input.html",
        16,
        16,
    )?;

    session
        .dispatch_input(HtmlBrowserInput::PointerMove { x: 4.0, y: 4.0 })
        .map_err(|error| error.to_string())?;
    dispatch_ignored_right_click(&mut session, 4.0, 4.0)?;
    session
        .dispatch_input(HtmlBrowserInput::KeyDown {
            key: "a".to_string(),
        })
        .map_err(|error| error.to_string())?;
    session
        .dispatch_input(HtmlBrowserInput::KeyUp {
            key: "a".to_string(),
        })
        .map_err(|error| error.to_string())?;
    assert_frame_contains_rgb(latest_frame(&session)?, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_evaluates_timer_driven_javascript_before_the_initial_frame() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}</style><div id="pixel"></div><script>setTimeout(() => { document.querySelector('#pixel').style.background='rgb(17,34,51)' }, 0)</script>"#,
        "https://example.test/timer.html",
        16,
        16,
    )?;

    assert_frame_contains_rgb(latest_frame(&session)?, [17, 34, 51])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_honors_no_sandbox_override_without_disabling_rendering() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let source = HtmlBrowserSource::new(
        "<!doctype html><style>html,body{margin:0;background:rgb(17,34,51)}</style>",
        "https://example.test/no-sandbox.html",
    )
    .map_err(|error| error.to_string())?;
    let response = child_response_for_source_load_with_chromium_override_and_no_sandbox(
        source,
        Some(test_chromium_binary()?),
    )?;

    let HtmlBrowserResponse::Frame { frame, .. } = response else {
        return Err(format!("unexpected browser child response: {response:?}"));
    };
    assert_frame_contains_rgb(&frame, [17, 34, 51])
}

#[test]
fn chromium_child_blocks_local_resources_outside_the_document_directory() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let origin = html_browser_fixture_origin()?;
    let mut session = start_session(
        r#"<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}#pixel{background:rgb(1,2,3)}</style><link rel="stylesheet" href="../outside.css"><div id="pixel"></div>"#,
        origin,
        16,
        16,
    )?;

    let frame = latest_frame(&session)?;
    assert_frame_contains_rgb(frame, [1, 2, 3])?;
    assert_frame_excludes_rgb(frame, [119, 136, 153])?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn chromium_child_reports_protocol_errors_without_launching_chromium() -> TestResult {
    let invalid_message = child_response_for_line("not-json")?;
    assert!(matches!(
        invalid_message,
        HtmlBrowserResponse::Error { code, .. } if code == "invalid_message"
    ));
    let unsupported_protocol = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION + 1,
        command: HtmlBrowserCommand::Close,
    })
    .map_err(|error| error.to_string())?;

    let response = child_response_for_line(&unsupported_protocol)?;

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, message, .. }
            if code == "protocol_version" && message.contains("unsupported")
    ));
    Ok(())
}

#[test]
fn chromium_child_reports_stdin_read_errors_without_launching_chromium() -> TestResult {
    let response = child_response_for_input(&[0xff, b'\n'])?;

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, message, .. }
            if code == "stdin_read" && message.contains("stream did not contain valid UTF-8")
    ));
    Ok(())
}

#[test]
fn chromium_child_uses_packaged_chromium_when_no_override_is_set() -> TestResult {
    let _browser_guard = chromium_session_guard()?;
    let response = child_response_for_load_with_chromium_override(None)?;

    assert!(match response {
        HtmlBrowserResponse::Frame { .. } => true,
        HtmlBrowserResponse::Error { code, .. } => code == "chromium",
        _ => false,
    });
    Ok(())
}

#[test]
fn chromium_child_reports_missing_explicit_chromium_override() -> TestResult {
    let missing =
        std::env::temp_dir().join(format!("krr-child-missing-chromium-{}", std::process::id()));
    let response = child_response_for_load_with_chromium_override(Some(missing))?;

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, message, .. }
            if code == "chromium"
                && message.contains("KRR_CHROME_BIN executable was not found")
    ));
    Ok(())
}

#[test]
fn chromium_child_reports_chromium_launch_errors() -> TestResult {
    let fake_chromium =
        std::env::temp_dir().join(format!("krr-child-fake-chromium-{}", std::process::id()));
    std::fs::write(&fake_chromium, b"not a chromium executable")
        .map_err(|error| error.to_string())?;

    let response = child_response_for_load_with_chromium_override(Some(fake_chromium.clone()))?;
    let _ = std::fs::remove_file(&fake_chromium);

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, .. } if code == "chromium"
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn chromium_child_reports_devtools_endpoint_connection_failure() -> TestResult {
    let fake_chromium = fake_chromium_script(
        "echo 'DevTools listening on ws://127.0.0.1:0/devtools/browser/test' >&2\nsleep 1",
    )?;
    let response = child_response_for_load_with_chromium_override(Some(fake_chromium.clone()))?;
    let _ = std::fs::remove_file(&fake_chromium);

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, .. } if code == "chromium"
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn chromium_child_reports_chromium_exit_before_devtools_endpoint() -> TestResult {
    let fake_chromium = fake_chromium_script("exit 7")?;
    let response = child_response_for_load_with_chromium_override(Some(fake_chromium.clone()))?;
    let _ = std::fs::remove_file(&fake_chromium);

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, .. } if code == "chromium"
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn chromium_child_reports_chromium_startup_timeout() -> TestResult {
    let fake_chromium = fake_chromium_script("sleep 35")?;
    let response = child_response_for_load_with_chromium_override(Some(fake_chromium.clone()))?;
    let _ = std::fs::remove_file(&fake_chromium);

    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, message, .. }
            if code == "chromium" && message.contains("did not expose a DevTools endpoint")
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn chromium_child_opens_local_document_when_source_directory_is_readonly() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let _browser_guard = chromium_session_guard()?;
    let directory = std::env::temp_dir().join(format!(
        "krr-child-readonly-document-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let origin_path = directory.join("index.html");
    std::fs::write(&origin_path, b"origin").map_err(|error| error.to_string())?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))
        .map_err(|error| error.to_string())?;
    let origin = Url::from_file_path(&origin_path)
        .map_err(|()| "readonly document path is not a file URL".to_string())?;
    let source = HtmlBrowserSource::new("<p>readonly</p>", origin.to_string())
        .map_err(|error| error.to_string())?;

    let response = child_response_for_source_load_with_chromium_override(
        source,
        Some(test_chromium_binary()?),
    );
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::remove_file(&origin_path);
    let _ = std::fs::remove_dir(&directory);
    let response = response?;

    assert!(matches!(response, HtmlBrowserResponse::Frame { .. }));
    Ok(())
}

#[test]
fn packaged_browser_config_reports_missing_adjacent_helper() -> TestResult {
    let _guard = HTML_BROWSER_ENGINE_ENV_LOCK
        .lock()
        .map_err(|error| error.to_string())?;
    unsafe { std::env::remove_var("KRR_HTML_BROWSER_ENGINE") };

    let result = HtmlBrowserProcessConfig::packaged();
    let expected_engine_name = if cfg!(target_os = "windows") {
        "krr-html-chromium-engine.exe"
    } else {
        "krr-html-chromium-engine"
    };

    assert!(matches!(
        result,
        Err(HtmlBrowserError::EngineBinaryNotFound { path })
            if path.ends_with(expected_engine_name)
    ));
    Ok(())
}

#[test]
fn packaged_browser_config_honors_engine_override() -> TestResult {
    let _guard = HTML_BROWSER_ENGINE_ENV_LOCK
        .lock()
        .map_err(|error| error.to_string())?;
    let helper =
        std::env::temp_dir().join(format!("krr-html-browser-engine-{}", std::process::id()));
    unsafe { std::env::set_var("KRR_HTML_BROWSER_ENGINE", &helper) };
    let result = HtmlBrowserProcessConfig::packaged();
    unsafe { std::env::remove_var("KRR_HTML_BROWSER_ENGINE") };
    let config = result.map_err(|error| error.to_string())?;

    assert_eq!(config.program, helper);
    assert!(config.args.is_empty());
    assert_eq!(config.chromium_binary, None);
    Ok(())
}

fn start_session(
    raw_html: &str,
    origin: impl Into<String>,
    width: u32,
    height: u32,
) -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(raw_html, origin).map_err(|error| error.to_string())?;
    let config = browser_process_config()?;
    HtmlBrowserSession::start(source, viewport(width, height)?, &config)
        .map_err(|error| error.to_string())
}

fn chromium_session_guard() -> TestResult<MutexGuard<'static, ()>> {
    Ok(CHROMIUM_SESSION_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner()))
}

fn viewport(width: u32, height: u32) -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(width, height, 1.0).map_err(|error| error.to_string())
}

fn latest_frame(session: &HtmlBrowserSession) -> TestResult<&HtmlBrowserFrame> {
    session
        .latest_frame()
        .ok_or_else(|| "browser session did not return a frame".to_string())
}

fn take_frame_update(session: &mut HtmlBrowserSession) -> TestResult<&HtmlBrowserFrame> {
    session
        .take_frame_update()
        .ok_or_else(|| "browser session did not return a frame update".to_string())
}

fn take_navigation(session: &mut HtmlBrowserSession) -> TestResult<HtmlBrowserNavigationEvent> {
    session
        .take_navigation()
        .ok_or_else(|| "browser session did not return a navigation event".to_string())
}

fn wait_for_frame_rgb(session: &mut HtmlBrowserSession, rgb: [u8; 3]) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        session.refresh_frame().map_err(|error| error.to_string())?;
        if frame_contains_rgb(latest_frame(session)?, rgb) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return assert_frame_contains_rgb(latest_frame(session)?, rgb);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn assert_text_ignored_while_blurred(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::Focus { focused: false })
        .map_err(|error| error.to_string())?;
    dispatch_text_ok(session)?;
    assert_frame_excludes_rgb(latest_frame(session)?, [17, 34, 51])
}

fn assert_text_delivered_while_focused(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::Focus { focused: true })
        .map_err(|error| error.to_string())?;
    dispatch_text_ok(session)?;
    assert_frame_contains_rgb(latest_frame(session)?, [17, 34, 51])
}

fn dispatch_text_ok(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "ok".to_string(),
        })
        .map_err(|error| error.to_string())
}

fn focus_form_html() -> &'static str {
    r#"<!doctype html><style>html,body{margin:0}input{width:32px;height:16px}#marker{position:absolute;top:16px;width:32px;height:16px;background:rgb(1,2,3)}</style><input id="field" autofocus><div id="marker"></div><script>const field=document.querySelector('#field');field.addEventListener('input',()=>{if(field.value==='ok')document.querySelector('#marker').style.background='rgb(17,34,51)'})</script>"#
}

fn dispatch_click(session: &mut HtmlBrowserSession, x: f32, y: f32) -> TestResult {
    for input in [
        HtmlBrowserInput::PointerDown { x, y, button: 0 },
        HtmlBrowserInput::PointerUp { x, y, button: 0 },
    ] {
        session
            .dispatch_input(input)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn dispatch_ignored_right_click(session: &mut HtmlBrowserSession, x: f32, y: f32) -> TestResult {
    for input in [
        HtmlBrowserInput::PointerDown { x, y, button: 1 },
        HtmlBrowserInput::PointerUp { x, y, button: 1 },
    ] {
        session
            .dispatch_input(input)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn scroll_down_one_viewport(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 32.0,
        })
        .map_err(|error| error.to_string())
}

fn assert_initial_local_resources(session: &HtmlBrowserSession) -> TestResult {
    let frame = latest_frame(session)?;
    assert_frame_contains_rgb(frame, [17, 34, 51])?;
    assert_frame_contains_rgb(frame, [68, 85, 102])
}

fn assert_resized_frame(frame: &HtmlBrowserFrame) -> TestResult {
    if frame.viewport.width != 24 || frame.viewport.height != 16 {
        return Err(format!(
            "unexpected frame viewport {}x{}",
            frame.viewport.width, frame.viewport.height
        ));
    }
    if frame.pixels.len() != 24 * 16 * 4 {
        return Err(format!(
            "unexpected frame byte length {}",
            frame.pixels.len()
        ));
    }
    Ok(())
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

fn html_browser_fixture_origin() -> TestResult<String> {
    let origin_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/html_browser/index.html");
    Url::from_file_path(origin_path)
        .map(|url| url.to_string())
        .map_err(|()| "HTML browser fixture path is not a valid file URL".to_string())
}

fn browser_process_config() -> TestResult<HtmlBrowserProcessConfig> {
    Ok(
        HtmlBrowserProcessConfig::new(env!("CARGO_BIN_EXE_krr-html-chromium-engine").into())
            .with_chromium_binary(test_chromium_binary()?)
            .with_request_timeout(Duration::from_secs(45)),
    )
}

#[cfg(target_os = "macos")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    let [adjacent_bundle, build_bundle] = bundled_chromium_binaries()?;
    chromium_candidate([
        adjacent_bundle,
        build_bundle,
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from(
            "/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        ),
    ])
}

#[cfg(target_os = "linux")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    let [adjacent_bundle, build_bundle] = bundled_chromium_binaries()?;
    chromium_candidate([
        adjacent_bundle,
        build_bundle,
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
    ])
}

#[cfg(target_os = "windows")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    let mut candidates = Vec::from(bundled_chromium_binaries()?);
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

#[cfg(unix)]
fn fake_chromium_script(body: &str) -> TestResult<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let sequence = FAKE_CHROMIUM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "krr-child-fake-chromium-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).map_err(|error| error.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    Ok(path)
}

fn bundled_chromium_binaries() -> TestResult<[PathBuf; 2]> {
    let helper = PathBuf::from(env!("CARGO_BIN_EXE_krr-html-chromium-engine"));
    let directory = helper
        .parent()
        .ok_or_else(|| "browser helper test binary has no parent directory".to_string())?;
    let relative_path = bundled_chromium_relative_path();
    let build_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/debug");
    Ok([
        directory.join(relative_path),
        build_directory.join(relative_path),
    ])
}

fn child_response_for_line(line: &str) -> TestResult<HtmlBrowserResponse> {
    child_response_for_input(format!("{line}\n").as_bytes())
}

fn child_response_for_input(input: &[u8]) -> TestResult<HtmlBrowserResponse> {
    child_response_for_input_with_chromium_override(input, None, false)
}

fn child_response_for_load_with_chromium_override(
    chromium_binary: Option<PathBuf>,
) -> TestResult<HtmlBrowserResponse> {
    let source = HtmlBrowserSource::new(
        "<!doctype html><style>html,body{margin:0;background:rgb(1,2,3)}</style>",
        "https://example.test/packaged.html",
    )
    .map_err(|error| error.to_string())?;
    child_response_for_source_load_with_chromium_override(source, chromium_binary)
}

fn child_response_for_source_load_with_chromium_override(
    source: HtmlBrowserSource,
    chromium_binary: Option<PathBuf>,
) -> TestResult<HtmlBrowserResponse> {
    child_response_for_source_load_with_options(source, chromium_binary, false)
}

fn child_response_for_source_load_with_chromium_override_and_no_sandbox(
    source: HtmlBrowserSource,
    chromium_binary: Option<PathBuf>,
) -> TestResult<HtmlBrowserResponse> {
    child_response_for_source_load_with_options(source, chromium_binary, true)
}

fn child_response_for_source_load_with_options(
    source: HtmlBrowserSource,
    chromium_binary: Option<PathBuf>,
    no_sandbox: bool,
) -> TestResult<HtmlBrowserResponse> {
    let request = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        command: HtmlBrowserCommand::Load {
            source,
            viewport: viewport(2, 2)?,
        },
    })
    .map_err(|error| error.to_string())?;
    child_response_for_input_with_chromium_override(
        format!("{request}\n").as_bytes(),
        chromium_binary,
        no_sandbox,
    )
}

fn child_response_for_input_with_chromium_override(
    input: &[u8],
    chromium_binary: Option<PathBuf>,
    no_sandbox: bool,
) -> TestResult<HtmlBrowserResponse> {
    let mut child = spawn_browser_child(chromium_binary, no_sandbox)?;
    write_child_input(&mut child, input)?;
    let (reader, receiver) = spawn_child_response_reader(&mut child)?;
    let response = match child_response_line(&mut child, receiver) {
        Ok(response) => response,
        Err(error) => {
            let _ = reader.join();
            return Err(error);
        }
    };
    let status = wait_for_child_exit(&mut child, CHILD_RESPONSE_TIMEOUT);
    let _ = reader.join();
    let status = status?;
    if !status.success() {
        return Err(format!("browser child exited unsuccessfully: {status}"));
    }
    serde_json::from_str(response.trim_end()).map_err(|error| error.to_string())
}

fn spawn_browser_child(chromium_binary: Option<PathBuf>, no_sandbox: bool) -> TestResult<Child> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_krr-html-chromium-engine"));
    if let Some(path) = chromium_binary {
        command.env("KRR_CHROME_BIN", path);
    } else {
        command.env_remove("KRR_CHROME_BIN");
    }
    if no_sandbox {
        command.env("KRR_CHROMIUM_NO_SANDBOX", "1");
    } else {
        command.env_remove("KRR_CHROMIUM_NO_SANDBOX");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())
}

fn write_child_input(child: &mut Child, input: &[u8]) -> TestResult {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "browser child stdin was not piped".to_string())?;
    stdin.write_all(input).map_err(|error| error.to_string())?;
    Ok(())
}

fn spawn_child_response_reader(child: &mut Child) -> TestResult<ChildResponseReader> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "browser child stdout was not piped".to_string())?;
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut response = String::new();
        let result = BufReader::new(stdout)
            .read_line(&mut response)
            .map(|_| response);
        let _ = sender.send(result);
    });
    Ok((reader, receiver))
}

fn child_response_line(
    child: &mut Child,
    receiver: mpsc::Receiver<std::io::Result<String>>,
) -> TestResult<String> {
    match receiver.recv_timeout(CHILD_RESPONSE_TIMEOUT) {
        Ok(response) => response.map_err(|error| error.to_string()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            terminate_child(child);
            Err(format!(
                "browser child did not write a response within {}ms",
                CHILD_RESPONSE_TIMEOUT.as_millis()
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            terminate_child(child);
            Err("browser child response reader stopped".to_string())
        }
    }
}

fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> TestResult<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            terminate_child(child);
            return Err(format!(
                "browser child did not exit within {}ms",
                timeout.as_millis()
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
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
