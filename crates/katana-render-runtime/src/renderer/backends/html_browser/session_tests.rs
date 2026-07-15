use super::*;
use crate::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserEngineErrorCode, HtmlBrowserPixelFormat,
    HtmlBrowserResponse,
};
#[cfg(unix)]
use std::path::PathBuf;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn new_session_exposes_active_state_without_a_process() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;

    assert_eq!(session.source().raw_html, "<p>ok</p>");
    assert_eq!(session.viewport().width, 2);
    assert_eq!(session.state(), HtmlBrowserSessionState::Active);
    assert!(!session.has_process());
    assert!(session.latest_frame().is_none());
    assert!(session.take_frame_update().is_none());
    Ok(())
}

#[test]
fn accept_frame_exposes_each_frame_update_once() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;

    assert!(matches!(
        session.accept_frame(test_frame("https://example.test/b.html", 1)?),
        Err(HtmlBrowserError::FrameOriginMismatch { .. })
    ));
    session
        .accept_frame(test_frame("https://example.test/a.html", 2)?)
        .map_err(|error| error.to_string())?;
    assert_eq!(
        session.take_frame_update().map(|frame| frame.generation),
        Some(2)
    );
    assert!(session.take_frame_update().is_none());
    assert!(matches!(
        session.accept_frame(test_frame("https://example.test/a.html", 2)?),
        Err(HtmlBrowserError::StaleFrameGeneration {
            latest: 2,
            received: 2
        })
    ));
    Ok(())
}

#[test]
fn accept_response_records_navigation() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;
    let navigation = HtmlBrowserNavigationEvent::new("https://example.test/b.html")
        .map_err(|error| error.to_string())?;

    session
        .accept_response(HtmlBrowserResponse::Navigation {
            protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
            navigation,
        })
        .map_err(|error| error.to_string())?;
    assert_eq!(
        session
            .take_navigation()
            .map(|event| event.url.as_str().to_string()),
        Some("https://example.test/b.html".to_string())
    );
    Ok(())
}

#[test]
fn accept_response_rejects_closed_as_unexpected() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;

    assert!(matches!(
        session.accept_response(HtmlBrowserResponse::Closed {
            protocol_version: HTML_BROWSER_PROTOCOL_VERSION
        }),
        Err(HtmlBrowserError::UnexpectedProcessResponse { .. })
    ));
    Ok(())
}

#[test]
fn accept_response_rejects_engine_error() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;

    assert!(matches!(
        session.accept_response(HtmlBrowserResponse::Error {
            protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
            code: "chromium".to_string(),
            message: "boom".to_string()
        }),
        Err(HtmlBrowserError::EngineRejected {
            code: HtmlBrowserEngineErrorCode::Chromium,
            ..
        })
    ));
    Ok(())
}

#[test]
fn accept_response_errors_do_not_republish_stale_frame_updates() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;
    session
        .accept_frame(test_frame("https://example.test/a.html", 1)?)
        .map_err(|error| error.to_string())?;
    assert!(session.take_frame_update().is_some());

    let result = session.accept_response(HtmlBrowserResponse::Error {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        code: "invalid_request".to_string(),
        message: "bad input".to_string(),
    });

    assert!(matches!(
        result,
        Err(HtmlBrowserError::EngineRejected {
            code: HtmlBrowserEngineErrorCode::InvalidRequest,
            ..
        })
    ));
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(1)
    );
    assert!(session.take_frame_update().is_none());
    Ok(())
}

#[cfg(unix)]
#[test]
fn session_process_roundtrip_supports_navigation_resize_input_and_close() -> TestResult {
    let mut session = HtmlBrowserSession::start(
        test_source("https://example.test/a.html")?,
        viewport()?,
        &shell_config(&roundtrip_script()),
    )
    .map_err(|error| error.to_string())?;

    session
        .navigate(
            HtmlBrowserNavigation::new(test_source("https://example.test/b.html")?)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    session
        .resize(viewport()?)
        .map_err(|error| error.to_string())?;
    session.refresh_frame().map_err(|error| error.to_string())?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "ok".to_string(),
        })
        .map_err(|error| error.to_string())?;
    session.close().map_err(|error| error.to_string())?;
    session.close().map_err(|error| error.to_string())?;

    assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    assert!(!session.has_process());
    Ok(())
}

#[cfg(unix)]
#[test]
fn close_terminates_process_when_close_request_fails() -> TestResult {
    let mut session = HtmlBrowserSession::start(
        test_source("https://example.test/a.html")?,
        viewport()?,
        &shell_config(&close_failure_script()),
    )
    .map_err(|error| error.to_string())?;

    session.close().map_err(|error| error.to_string())?;
    assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    assert!(!session.has_process());
    Ok(())
}

fn test_source(origin: &str) -> TestResult<HtmlBrowserSource> {
    HtmlBrowserSource::new("<p>ok</p>", origin).map_err(|error| error.to_string())
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(2, 2, 1.0).map_err(|error| error.to_string())
}

fn test_frame(origin: &str, generation: u64) -> TestResult<HtmlBrowserFrame> {
    HtmlBrowserFrame::new(
        generation,
        HtmlBrowserSource::new("", origin)
            .map_err(|error| error.to_string())?
            .origin,
        viewport()?,
        HtmlBrowserPixelFormat::Rgba8,
        vec![0; 16],
    )
    .map_err(|error| error.to_string())
}

#[cfg(unix)]
fn shell_config(script: &str) -> HtmlBrowserProcessConfig {
    let mut config = HtmlBrowserProcessConfig::new(PathBuf::from("/bin/sh"));
    config.args = vec!["-c".to_string(), script.to_string()];
    config
}

#[cfg(unix)]
fn roundtrip_script() -> String {
    format!(
        r#"count=0
while IFS= read -r request; do
  case "$request" in
    *'"command":"close"'*)
      printf '%s\n' '{{"result":"closed","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION}}}'
      exit 0
      ;;
  esac
  count=$((count + 1))
  case "$count" in
    1) origin='https://example.test/a.html' ;;
    *) origin='https://example.test/b.html' ;;
  esac
  printf '%s\n' '{{"result":"frame","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION},"frame":{{"generation":'"$count"',"origin":"'"$origin"'","viewport":{{"width":2,"height":2,"device_scale_factor":1.0}},"pixel_format":"Rgba8","pixels":[0,0,0,255,0,0,0,255,0,0,0,255,0,0,0,255]}}}}'
done"#
    )
}

#[cfg(unix)]
fn close_failure_script() -> String {
    format!(
        r#"count=0
while IFS= read -r request; do
  count=$((count + 1))
  case "$count" in
    1)
      printf '%s\n' '{{"result":"frame","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION},"frame":{{"generation":1,"origin":"https://example.test/a.html","viewport":{{"width":2,"height":2,"device_scale_factor":1.0}},"pixel_format":"Rgba8","pixels":[0,0,0,255,0,0,0,255,0,0,0,255,0,0,0,255]}}}}'
      ;;
    *)
      printf '%s\n' 'not-json'
      sleep 1
      ;;
  esac
done"#
    )
}
