use super::*;
use std::{
    io::{self, Cursor, Read},
    sync::mpsc,
    time::Duration,
};

#[cfg(unix)]
use crate::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserCommand, HtmlBrowserProcessConfig,
    HtmlBrowserResponse,
};
#[cfg(unix)]
use std::path::PathBuf;

#[cfg(unix)]
#[test]
fn spawn_passes_explicit_chromium_binary_to_child_environment() -> Result<(), String> {
    let script = format!(
        "IFS= read -r _request\nif [ \"$KRR_CHROME_BIN\" = '/tmp/krr-test-chrome' ]; then printf '%s\\n' '{{\"result\":\"closed\",\"protocol_version\":{HTML_BROWSER_PROTOCOL_VERSION}}}'; else printf '%s\\n' '{{\"result\":\"error\",\"protocol_version\":{HTML_BROWSER_PROTOCOL_VERSION},\"code\":\"env\",\"message\":\"missing explicit chromium binary\"}}'; fi"
    );
    let mut config = HtmlBrowserProcessConfig::new(PathBuf::from("/bin/sh"))
        .with_chromium_binary(PathBuf::from("/tmp/krr-test-chrome"));
    config.args = vec!["-c".to_string(), script];

    let mut process = HtmlBrowserProcess::spawn(&config).map_err(|error| error.to_string())?;
    let response = process
        .request(HtmlBrowserCommand::Close)
        .map_err(|error| error.to_string())?;
    process.wait_for_exit().map_err(|error| error.to_string())?;

    assert_eq!(
        response,
        HtmlBrowserResponse::Closed {
            protocol_version: HTML_BROWSER_PROTOCOL_VERSION
        }
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn spawn_reports_missing_program() {
    let config = HtmlBrowserProcessConfig::new(PathBuf::from("/tmp/krr-missing-browser-child"));

    assert!(matches!(
        HtmlBrowserProcess::spawn(&config),
        Err(HtmlBrowserError::ProcessStart { .. })
    ));
}

#[cfg(unix)]
#[test]
fn request_reports_invalid_child_json() -> Result<(), String> {
    let mut process = HtmlBrowserProcess::spawn(&shell_config(
        "IFS= read -r _request\nprintf '%s\\n' 'not-json'",
    ))
    .map_err(|error| error.to_string())?;
    let response = process.request(HtmlBrowserCommand::Close);
    process.wait_for_exit().map_err(|error| error.to_string())?;

    assert!(matches!(
        response,
        Err(HtmlBrowserError::InvalidProcessMessage { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn request_reports_child_exit_without_response() -> Result<(), String> {
    let mut process = HtmlBrowserProcess::spawn(&shell_config("IFS= read -r _request\nexit 7"))
        .map_err(|error| error.to_string())?;

    assert!(matches!(
        process.request(HtmlBrowserCommand::Close),
        Err(HtmlBrowserError::ProcessCrashed { .. })
    ));
    Ok(())
}

#[cfg(unix)]
#[test]
fn request_times_out_and_terminates_silent_child() -> Result<(), String> {
    let mut process = HtmlBrowserProcess::spawn(
        &shell_config("IFS= read -r _request\nsleep 1")
            .with_request_timeout(Duration::from_millis(100)),
    )
    .map_err(|error| error.to_string())?;

    assert!(matches!(
        process.request(HtmlBrowserCommand::Close),
        Err(HtmlBrowserError::ProcessTimeout { timeout_ms: 100 })
    ));
    process.wait_for_exit().map_err(|error| error.to_string())
}

#[cfg(unix)]
#[test]
fn terminate_succeeds_after_child_already_exited() -> Result<(), String> {
    let mut process =
        HtmlBrowserProcess::spawn(&shell_config("exit 0")).map_err(|error| error.to_string())?;
    process.wait_for_exit().map_err(|error| error.to_string())?;

    process.terminate().map_err(|error| error.to_string())
}

#[test]
fn decode_response_line_reports_reader_errors() {
    assert!(matches!(
        HtmlBrowserProcess::decode_response_line(Err("read failed".to_string())),
        Err(HtmlBrowserError::ProcessRead { error }) if error == "read failed"
    ));
}

#[test]
fn read_responses_maps_io_errors() -> Result<(), String> {
    let responses = HtmlBrowserProcess::read_responses(FailingRead);
    let line = responses.recv().map_err(|error| error.to_string())?;

    assert!(matches!(line, Err(error) if error == "reader failed"));
    Ok(())
}

#[test]
fn read_responses_stops_when_receiver_is_dropped() -> Result<(), String> {
    let (release, wait_for_release) = mpsc::channel();
    let responses = HtmlBrowserProcess::read_responses(DelayedRead {
        release: wait_for_release,
        cursor: Cursor::new(b"{\"result\":\"closed\",\"protocol_version\":1}\n".to_vec()),
    });
    drop(responses);
    release.send(()).map_err(|error| error.to_string())?;
    std::thread::sleep(Duration::from_millis(25));
    Ok(())
}

#[test]
fn process_start_and_write_errors_preserve_messages() {
    assert_eq!(
        process_start_error(io::Error::other("start")),
        HtmlBrowserError::ProcessStart {
            error: "start".to_string()
        }
    );
    assert_eq!(
        process_write_error(io::Error::other("write")),
        HtmlBrowserError::ProcessWrite {
            error: "write".to_string()
        }
    );
    assert_eq!(
        missing_stdin_error(),
        HtmlBrowserError::ProcessStart {
            error: "browser stdin was not piped".to_string()
        }
    );
    assert_eq!(
        missing_stdout_error(),
        HtmlBrowserError::ProcessStart {
            error: "browser stdout was not piped".to_string()
        }
    );
}

#[test]
fn protocol_errors_preserve_messages() {
    assert_eq!(
        protocol_encode_error("encode"),
        HtmlBrowserError::ProtocolEncode {
            error: "encode".to_string()
        }
    );
    assert_eq!(
        invalid_process_message_error("invalid"),
        HtmlBrowserError::InvalidProcessMessage {
            error: "invalid".to_string()
        }
    );
}

#[test]
fn process_terminate_error_preserves_message() {
    assert_eq!(
        process_terminate_error(io::Error::other("terminate")),
        HtmlBrowserError::ProcessTerminate {
            error: "terminate".to_string()
        }
    );
}

#[test]
fn child_crashed_from_try_wait_error_becomes_process_read_error() {
    assert_eq!(
        child_crashed_from_status(Err(io::Error::other("try-wait"))),
        HtmlBrowserError::ProcessRead {
            error: "try-wait".to_string()
        }
    );
}

#[test]
fn child_crashed_without_exit_status_reports_closed_stdout() {
    assert_eq!(
        child_crashed_from_status(Ok(None)),
        HtmlBrowserError::ProcessCrashed {
            status: "stdout closed while process was still running".to_string()
        }
    );
}

#[cfg(unix)]
#[test]
fn child_crashed_with_exit_status_reports_status() {
    use std::os::unix::process::ExitStatusExt;

    assert_eq!(
        child_crashed_from_status(Ok(Some(ExitStatus::from_raw(7 << 8)))),
        HtmlBrowserError::ProcessCrashed {
            status: "exit status: 7".to_string()
        }
    );
}

#[cfg(unix)]
fn shell_config(script: &str) -> HtmlBrowserProcessConfig {
    let mut config = HtmlBrowserProcessConfig::new(PathBuf::from("/bin/sh"));
    config.args = vec!["-c".to_string(), script.to_string()];
    config
}

struct FailingRead;

impl Read for FailingRead {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("reader failed"))
    }
}

struct DelayedRead {
    release: mpsc::Receiver<()>,
    cursor: Cursor<Vec<u8>>,
}

impl Read for DelayedRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let _ = self.release.recv();
        self.cursor.read(buffer)
    }
}
