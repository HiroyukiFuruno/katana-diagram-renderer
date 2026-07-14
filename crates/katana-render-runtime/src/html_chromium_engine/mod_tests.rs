use super::*;
use crate::{HtmlBrowserRequest, HtmlBrowserSource, HtmlBrowserViewport};
use std::cell::RefCell;
use std::io;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn handle_line_rejects_invalid_json() {
    assert!(matches!(
        handle_line("not-json"),
        Err((code, _message)) if code == "invalid_message"
    ));
}

#[test]
fn handle_line_rejects_unsupported_protocol_version() -> TestResult {
    let line = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION + 1,
        command: HtmlBrowserCommand::Close,
    })
    .map_err(|error| error.to_string())?;

    assert_eq!(
        handle_line(&line),
        Err((
            "protocol_version".to_string(),
            "unsupported protocol version".to_string()
        ))
    );
    Ok(())
}

#[test]
fn handle_line_accepts_close_without_loaded_page() -> TestResult {
    let line = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        command: HtmlBrowserCommand::Close,
    })
    .map_err(|error| error.to_string())?;

    assert_eq!(
        handle_line(&line),
        Ok((
            HtmlBrowserResponse::Closed {
                protocol_version: HTML_BROWSER_PROTOCOL_VERSION
            },
            true
        ))
    );
    Ok(())
}

#[test]
fn loop_response_converts_errors_to_ipc_error_responses() {
    assert_eq!(
        loop_response(Err(("code".to_string(), "message".to_string()))),
        (
            HtmlBrowserResponse::Error {
                protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
                code: "code".to_string(),
                message: "message".to_string()
            },
            false
        )
    );
}

#[test]
fn chromium_error_maps_to_ipc_error_code() {
    assert_eq!(
        chromium_error("boom".to_string()),
        ("chromium".to_string(), "boom".to_string())
    );
}

#[test]
fn string_error_preserves_display_message() {
    let error = io::Error::other("write failed");
    assert_eq!(io_error(error), "write failed");
    let error = serde_json::from_str::<HtmlBrowserRequest>("{").map_err(json_error);
    assert!(matches!(error, Err(message) if message.contains("EOF")));
    let error =
        serde_json::from_str::<HtmlBrowserRequest>("not-json").map_err(invalid_message_error);
    assert!(matches!(error, Err((code, _message)) if code == "invalid_message"));
}

#[test]
fn resize_requires_loaded_page() -> TestResult {
    let viewport = HtmlBrowserViewport::new(2, 2, 1.0).map_err(|error| error.to_string())?;
    let resize = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        command: HtmlBrowserCommand::Resize { viewport },
    })
    .map_err(|error| error.to_string())?;

    assert_eq!(
        handle_line(&resize),
        Err((
            "not_loaded".to_string(),
            "load must precede input or resize".to_string()
        ))
    );
    Ok(())
}

#[test]
fn input_requires_loaded_page() -> TestResult {
    let input = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        command: HtmlBrowserCommand::Input {
            input: crate::HtmlBrowserInput::KeyUp {
                key: "Enter".to_string(),
            },
        },
    })
    .map_err(|error| error.to_string())?;

    assert_eq!(
        handle_line(&input),
        Err((
            "not_loaded".to_string(),
            "load must precede input or resize".to_string()
        ))
    );
    Ok(())
}

#[test]
fn frame_refresh_requires_loaded_page() -> TestResult {
    let frame = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        command: HtmlBrowserCommand::Frame,
    })
    .map_err(|error| error.to_string())?;

    assert_eq!(
        handle_line(&frame),
        Err((
            "not_loaded".to_string(),
            "load must precede input or resize".to_string()
        ))
    );
    Ok(())
}

#[test]
fn load_rejects_invalid_request_before_starting_chromium() -> TestResult {
    let line = serde_json::to_string(&HtmlBrowserRequest {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        command: HtmlBrowserCommand::Load {
            source: HtmlBrowserSource {
                raw_html: "<p>ok</p>".to_string(),
                origin: crate::HtmlBrowserOrigin::parse("https://example.test/")
                    .map_err(|error| error.to_string())?,
            },
            viewport: HtmlBrowserViewport {
                width: 0,
                height: 2,
                device_scale_factor: 1.0,
            },
        },
    })
    .map_err(|error| error.to_string())?;

    assert!(matches!(
        handle_line(&line),
        Err((code, _message)) if code == "invalid_request"
    ));
    Ok(())
}

#[test]
fn load_rejects_deserialized_invalid_source_before_starting_chromium() -> TestResult {
    let source = serde_json::from_str::<HtmlBrowserSource>(
        r#"{"raw_html":"<p>ok</p>","origin":"not a url"}"#,
    )
    .map_err(|error| error.to_string())?;
    let slot = RefCell::new(None);

    assert_invalid_request(handle_command(
        &slot,
        HtmlBrowserCommand::Load {
            source,
            viewport: must_viewport()?,
        },
    ));
    Ok(())
}

#[test]
fn handle_command_rejects_invalid_resize_and_input_before_session_lookup() {
    let slot = RefCell::new(None);

    assert_invalid_request(handle_command(&slot, invalid_resize_command()));
    assert_invalid_request(handle_command(&slot, invalid_input_command()));
}

fn assert_invalid_request(result: Result<(HtmlBrowserResponse, bool), (String, String)>) {
    assert!(matches!(
        result,
        Err((code, _message)) if code == "invalid_request"
    ));
}

fn invalid_resize_command() -> HtmlBrowserCommand {
    HtmlBrowserCommand::Resize {
        viewport: HtmlBrowserViewport {
            width: 0,
            height: 2,
            device_scale_factor: 1.0,
        },
    }
}

fn invalid_input_command() -> HtmlBrowserCommand {
    HtmlBrowserCommand::Input {
        input: crate::HtmlBrowserInput::PointerMove {
            x: f32::NAN,
            y: 0.0,
        },
    }
}

fn must_viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(2, 2, 1.0).map_err(|error| error.to_string())
}
