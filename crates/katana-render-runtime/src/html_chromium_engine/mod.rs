mod document;
mod input;
mod main_document;
mod page;
mod page_slot;
mod policy;
mod runtime;
mod source;

use crate::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserCommand, HtmlBrowserError, HtmlBrowserRequest,
    HtmlBrowserResponse,
};
use page::ChromiumPage;
use source::BrowserSource;
use std::io::{self, BufRead, Write};

pub struct HtmlChromiumEngine;

impl HtmlChromiumEngine {
    pub fn run() {
        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        run_with_io(&mut reader, &mut writer);
    }
}

fn run_with_io(reader: &mut dyn BufRead, writer: &mut dyn Write) {
    for line in reader.lines() {
        let (response, close) = match line {
            Ok(line) => loop_response(handle_line(&line)),
            Err(error) => (
                error_response("stdin_read".to_string(), error.to_string()),
                true,
            ),
        };
        let _ = try_write_response(writer, &response);
        if close {
            break;
        }
    }
    page_slot::clear();
}

fn loop_response(
    response: Result<(HtmlBrowserResponse, bool), (String, String)>,
) -> (HtmlBrowserResponse, bool) {
    match response {
        Ok((response, close)) => (response, close),
        Err((code, message)) => (error_response(code, message), false),
    }
}

fn error_response(code: String, message: String) -> HtmlBrowserResponse {
    HtmlBrowserResponse::Error {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
        code,
        message,
    }
}

fn handle_line(line: &str) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    let request: HtmlBrowserRequest = serde_json::from_str(line).map_err(invalid_message_error)?;
    if request.protocol_version != HTML_BROWSER_PROTOCOL_VERSION {
        return Err((
            "protocol_version".into(),
            "unsupported protocol version".into(),
        ));
    }
    page_slot::with_page(|slot| handle_command(slot, request.command))
}

fn handle_command(
    slot: &std::cell::RefCell<Option<ChromiumPage>>,
    command: HtmlBrowserCommand,
) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    match command {
        HtmlBrowserCommand::Load { source, viewport } => load(slot, source, viewport),
        HtmlBrowserCommand::Frame => refresh_frame(slot),
        HtmlBrowserCommand::Resize { viewport } => resize(slot, viewport),
        HtmlBrowserCommand::Input { input: event } => dispatch_input(slot, event),
        HtmlBrowserCommand::Close => {
            *slot.borrow_mut() = None;
            Ok((
                HtmlBrowserResponse::Closed {
                    protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
                },
                true,
            ))
        }
    }
}

fn load(
    slot: &std::cell::RefCell<Option<ChromiumPage>>,
    source: crate::HtmlBrowserSource,
    viewport: crate::HtmlBrowserViewport,
) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    let source = BrowserSource::validate(source).map_err(request_error)?;
    viewport.validate().map_err(request_error)?;
    let mut borrowed = slot.borrow_mut();
    match borrowed.as_mut() {
        Some(page) => page.navigate(source, viewport).map_err(chromium_error)?,
        None => *borrowed = Some(ChromiumPage::new(source, viewport).map_err(chromium_error)?),
    }
    let page = borrowed.as_mut().ok_or_else(not_loaded)?;
    frame_response(page)
}

fn refresh_frame(
    slot: &std::cell::RefCell<Option<ChromiumPage>>,
) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    let mut borrowed = slot.borrow_mut();
    let page = borrowed.as_mut().ok_or_else(not_loaded)?;
    frame_response(page)
}

fn resize(
    slot: &std::cell::RefCell<Option<ChromiumPage>>,
    viewport: crate::HtmlBrowserViewport,
) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    viewport.validate().map_err(request_error)?;
    let mut borrowed = slot.borrow_mut();
    let page = borrowed.as_mut().ok_or_else(not_loaded)?;
    page.resize(viewport).map_err(chromium_error)?;
    frame_response(page)
}

fn dispatch_input(
    slot: &std::cell::RefCell<Option<ChromiumPage>>,
    input: crate::HtmlBrowserInput,
) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    input.validate().map_err(request_error)?;
    let mut borrowed = slot.borrow_mut();
    let page = borrowed.as_mut().ok_or_else(not_loaded)?;
    page.input(input).map_err(chromium_error)?;
    if let Some(navigation) = page.take_navigation().map_err(chromium_error)? {
        return Ok((
            HtmlBrowserResponse::Navigation {
                protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
                navigation,
            },
            false,
        ));
    }
    frame_response(page)
}

fn frame_response(
    page: &mut ChromiumPage,
) -> Result<(HtmlBrowserResponse, bool), (String, String)> {
    let frame = page.screenshot().map_err(chromium_error)?;
    Ok((
        HtmlBrowserResponse::Frame {
            protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
            frame,
        },
        false,
    ))
}

fn request_error(error: HtmlBrowserError) -> (String, String) {
    ("invalid_request".into(), error.to_string())
}
fn chromium_error(error: String) -> (String, String) {
    ("chromium".into(), error)
}
fn not_loaded() -> (String, String) {
    (
        "not_loaded".into(),
        "load must precede input or resize".into(),
    )
}

fn try_write_response(
    writer: &mut dyn Write,
    response: &HtmlBrowserResponse,
) -> Result<(), String> {
    let line = serde_json::to_string(response).map_err(json_error)?;
    writeln!(writer, "{line}").map_err(io_error)?;
    writer.flush().map_err(io_error)
}

fn invalid_message_error(error: serde_json::Error) -> (String, String) {
    ("invalid_message".into(), error.to_string())
}

fn json_error(error: serde_json::Error) -> String {
    error.to_string()
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
#[path = "mod_io_tests.rs"]
mod io_tests;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
