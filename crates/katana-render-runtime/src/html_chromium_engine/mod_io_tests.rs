use super::*;
use std::io::{self, Write};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn run_with_io_reports_stdin_read_errors_and_stops() -> TestResult {
    let mut output = Vec::new();
    let mut reader = io::Cursor::new([b"not-json\n".as_slice(), &[0xff, b'\n']].concat());

    run_with_io(&mut reader, &mut output);

    let responses = responses_from_output(&output)?;
    assert!(matches!(
        responses.first(),
        Some(HtmlBrowserResponse::Error { code, .. }) if code == "invalid_message"
    ));
    let response = responses
        .get(1)
        .ok_or_else(|| "stdin read error response was not emitted".to_string())?;
    assert!(matches!(
        response,
        HtmlBrowserResponse::Error { code, message, .. }
            if code == "stdin_read" && message.contains("stream did not contain valid UTF-8")
    ));
    Ok(())
}

#[test]
fn run_with_io_accepts_empty_input() {
    let mut reader = io::Cursor::new(Vec::<u8>::new());
    let mut output = Vec::new();

    run_with_io(&mut reader, &mut output);

    assert!(output.is_empty());
}

#[test]
fn try_write_response_reports_write_errors() -> TestResult {
    let mut writer = WriteFailingWriter;
    let response = closed_response();

    let error = match try_write_response(&mut writer, &response) {
        Ok(()) => return Err("write should fail".to_string()),
        Err(error) => error,
    };

    assert!(error.contains("write failed"));
    Ok(())
}

#[test]
fn try_write_response_reports_flush_errors() -> TestResult {
    let mut writer = FlushFailingWriter;
    let response = closed_response();

    let error = match try_write_response(&mut writer, &response) {
        Ok(()) => return Err("flush should fail".to_string()),
        Err(error) => error,
    };

    assert!(error.contains("flush failed"));
    Ok(())
}

fn responses_from_output(output: &[u8]) -> TestResult<Vec<HtmlBrowserResponse>> {
    let line = std::str::from_utf8(output).map_err(|error| error.to_string())?;
    line.lines()
        .map(|line| serde_json::from_str(line).map_err(|error| error.to_string()))
        .collect()
}

fn closed_response() -> HtmlBrowserResponse {
    HtmlBrowserResponse::Closed {
        protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
    }
}

struct WriteFailingWriter;

impl Write for WriteFailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("write failed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FlushFailingWriter;

impl Write for FlushFailingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("flush failed"))
    }
}
