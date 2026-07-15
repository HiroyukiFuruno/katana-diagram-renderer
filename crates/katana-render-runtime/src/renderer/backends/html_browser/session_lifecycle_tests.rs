use super::*;
use crate::HTML_BROWSER_PROTOCOL_VERSION;
use std::path::PathBuf;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn recover_process_requires_a_started_session() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;

    assert!(matches!(
        session.recover_process(),
        Err(HtmlBrowserError::EngineNotStarted)
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn process_crash_detaches_and_recovery_reloads_current_page() -> TestResult {
    let mut session = HtmlBrowserSession::start(
        test_source("https://example.test/a.html")?,
        viewport()?,
        &shell_config(&crash_after_first_response_script()),
    )
    .map_err(|error| error.to_string())?;
    assert!(session.take_frame_update().is_some());

    assert!(matches!(
        session.refresh_frame(),
        Err(HtmlBrowserError::ProcessCrashed { .. })
    ));
    assert!(!session.has_process());

    session
        .recover_process()
        .map_err(|error| error.to_string())?;
    assert!(session.has_process());
    assert_eq!(
        session.take_frame_update().map(|frame| frame.generation),
        Some(1)
    );
    session.close().map_err(|error| error.to_string())
}

#[test]
fn process_failure_classifier_keeps_protocol_errors_attached() {
    let process_errors = [
        HtmlBrowserError::InvalidProcessMessage {
            error: "json".to_string(),
        },
        HtmlBrowserError::ProcessWrite {
            error: "write".to_string(),
        },
        HtmlBrowserError::ProcessRead {
            error: "read".to_string(),
        },
        HtmlBrowserError::ProcessTimeout { timeout_ms: 1 },
        HtmlBrowserError::ProcessCrashed {
            status: "exit".to_string(),
        },
    ];
    for error in process_errors {
        assert!(HtmlBrowserSession::drops_process_after_error(&error));
    }
    assert!(!HtmlBrowserSession::drops_process_after_error(
        &HtmlBrowserError::SessionClosed
    ));
}

fn test_source(origin: &str) -> TestResult<HtmlBrowserSource> {
    HtmlBrowserSource::new("<p>ok</p>", origin).map_err(|error| error.to_string())
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(2, 2, 1.0).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn shell_config(script: &str) -> HtmlBrowserProcessConfig {
    let mut config = HtmlBrowserProcessConfig::new(PathBuf::from("/bin/sh"));
    config.args = vec!["-c".to_string(), script.to_string()];
    config
}

#[cfg(unix)]
fn crash_after_first_response_script() -> String {
    format!(
        r#"count=0
while IFS= read -r _request; do
  count=$((count + 1))
  case "$count" in
    1)
      printf '%s\n' '{{"result":"frame","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION},"frame":{{"generation":1,"origin":"https://example.test/a.html","viewport":{{"width":2,"height":2,"device_scale_factor":1.0}},"pixel_format":"Rgba8","pixels":[0,0,0,255,0,0,0,255,0,0,0,255,0,0,0,255]}}}}'
      ;;
    *)
      exit 9
      ;;
  esac
done"#
    )
}
