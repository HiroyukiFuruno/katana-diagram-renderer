use crate::{HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserError, HtmlBrowserResponse};
use std::io::{self, Write};

pub(super) fn loop_response(
    response: Result<(HtmlBrowserResponse, bool), (String, String)>,
) -> (HtmlBrowserResponse, bool) {
    match response {
        Ok((response, close)) => (response, close),
        Err((code, message)) => (error_response(code, message), false),
    }
}

pub(super) fn error_response(code: String, message: String) -> HtmlBrowserResponse {
    HtmlBrowserResponse::Error {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        code,
        message,
    }
}

pub(super) fn request_error(error: HtmlBrowserError) -> (String, String) {
    ("invalid_request".into(), error.to_string())
}

pub(super) fn chromium_error(error: String) -> (String, String) {
    ("chromium".into(), error)
}

pub(super) fn not_loaded() -> (String, String) {
    (
        "not_loaded".into(),
        "load must precede input or resize".into(),
    )
}

pub(super) fn try_write_response(
    writer: &mut dyn Write,
    response: &HtmlBrowserResponse,
) -> Result<(), String> {
    let line = serde_json::to_string(response).map_err(json_error)?;
    writeln!(writer, "{line}").map_err(io_error)?;
    writer.flush().map_err(io_error)
}

pub(super) fn invalid_message_error(error: serde_json::Error) -> (String, String) {
    ("invalid_message".into(), error.to_string())
}

pub(super) fn json_error(error: serde_json::Error) -> String {
    error.to_string()
}

pub(super) fn io_error(error: io::Error) -> String {
    error.to_string()
}
