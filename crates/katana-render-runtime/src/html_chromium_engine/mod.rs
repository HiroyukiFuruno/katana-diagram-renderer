mod chromium_process;
mod chromium_startup;
mod document;
mod input;
mod ipc;
mod main_document;
mod page;
mod page_slot;
mod policy;
mod rendering_sync;
mod runtime;
mod source;

use crate::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserCommand, HtmlBrowserRequest, HtmlBrowserResponse,
};
use ipc::{
    chromium_error, error_response, invalid_message_error, loop_response, not_loaded,
    request_error, try_write_response,
};
#[cfg(test)]
use ipc::{io_error, json_error};
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

#[cfg(test)]
#[path = "mod_io_tests.rs"]
mod io_tests;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
