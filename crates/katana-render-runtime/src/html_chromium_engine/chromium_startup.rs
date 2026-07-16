use super::chromium_process::chromium_is_running;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, ChildStderr},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) const CHROMIUM_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const CHROMIUM_STDERR_LIMIT: usize = 16 * 1024;
const DEVTOOLS_LISTENING_PREFIX: &str = "DevTools listening on ";
static CHROMIUM_PROFILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn chromium_profile_directory() -> PathBuf {
    let timestamp = chromium_timestamp(SystemTime::now());
    let sequence = CHROMIUM_PROFILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "krr-chromium-{}-{timestamp}-{sequence}",
        std::process::id()
    ))
}

fn chromium_timestamp(now: SystemTime) -> u128 {
    match now.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    }
}

pub(super) fn wait_for_debug_ws_url(
    child: &mut Child,
    stderr: ChildStderr,
    startup_timeout: Duration,
) -> Result<String, String> {
    let responses = read_chromium_stderr(stderr);
    let deadline = Instant::now() + startup_timeout;
    let mut output = String::new();
    loop {
        let response = chromium_startup_response(&responses, child, deadline, &mut output);
        if let Err(error) = &response {
            return Err(error.clone());
        }
        let Some(line) = response.ok().flatten() else {
            continue;
        };
        let Some(debug_ws_url) = debug_ws_url_from_line(&line) else {
            continue;
        };
        return Ok(debug_ws_url);
    }
}

fn chromium_startup_response(
    responses: &Receiver<String>,
    child: &mut Child,
    deadline: Instant,
    output: &mut String,
) -> Result<Option<String>, String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(chromium_startup_timeout_error(output));
    }
    match responses.recv_timeout(remaining.min(Duration::from_millis(100))) {
        Ok(line) => {
            append_chromium_output(output, &line);
            Ok(Some(line))
        }
        Err(mpsc::RecvTimeoutError::Timeout) => chromium_is_running(responses, child, output),
        Err(mpsc::RecvTimeoutError::Disconnected) => chromium_stderr_closed_error(child, output),
    }
}

fn chromium_stderr_closed_error(child: &mut Child, output: &str) -> Result<Option<String>, String> {
    chromium_stderr_closed_status(child.try_wait(), output)
}

fn chromium_stderr_closed_status(
    status: Result<Option<std::process::ExitStatus>, std::io::Error>,
    output: &str,
) -> Result<Option<String>, String> {
    let status = match status {
        Ok(status) => status,
        Err(error) => return Err(error.to_string()),
    };
    let summary = chromium_stderr_closed_summary(status);
    Err(chromium_launch_error(&summary, output))
}

fn read_chromium_stderr(stderr: ChildStderr) -> Receiver<String> {
    let (sender, responses) = mpsc::sync_channel(32);
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let _ = sender.send(line.trim_end().to_string());
                }
            }
        }
    });
    responses
}

pub(super) fn append_chromium_output(output: &mut String, line: &str) {
    if output.len() >= CHROMIUM_STDERR_LIMIT {
        return;
    }
    let line_limit = (CHROMIUM_STDERR_LIMIT - output.len()).saturating_sub(1);
    let end = line
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(line.len()))
        .take_while(|index| *index <= line_limit)
        .last()
        .unwrap_or(0);
    output.push_str(&line[..end]);
    output.push('\n');
}

fn debug_ws_url_from_line(line: &str) -> Option<String> {
    let (_, url) = line.split_once(DEVTOOLS_LISTENING_PREFIX)?;
    let url = url.trim();
    if url.starts_with("ws://") {
        Some(url.to_string())
    } else {
        None
    }
}

pub(super) fn chromium_launch_error(summary: &str, output: &str) -> String {
    if output.is_empty() {
        format!("{summary}; Chromium emitted no stderr output")
    } else {
        format!("{summary}: {output}")
    }
}

fn chromium_startup_timeout_error(output: &str) -> String {
    chromium_launch_error("Chromium did not expose a DevTools endpoint", output)
}

fn chromium_stderr_closed_summary(status: Option<std::process::ExitStatus>) -> String {
    match status {
        Some(status) => format!("Chromium exited with {status}"),
        None => "Chromium closed stderr before exposing a DevTools endpoint".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HtmlBrowserViewport;

    #[test]
    fn chromium_output_respects_byte_and_utf8_limits() {
        let mut full = "x".repeat(CHROMIUM_STDERR_LIMIT);
        append_chromium_output(&mut full, "unreachable");
        assert_eq!(full.len(), CHROMIUM_STDERR_LIMIT);

        let mut output = "x".repeat(CHROMIUM_STDERR_LIMIT - 1);
        append_chromium_output(&mut output, "あ");
        assert_eq!(
            output,
            format!("{}\n", "x".repeat(CHROMIUM_STDERR_LIMIT - 1))
        );
    }

    #[test]
    fn chromium_output_extracts_devtools_websocket_url() {
        assert_eq!(
            debug_ws_url_from_line("DevTools listening on ws://127.0.0.1/devtools/browser/id"),
            Some("ws://127.0.0.1/devtools/browser/id".to_string())
        );
        assert_eq!(
            debug_ws_url_from_line("DevTools listening on http://127.0.0.1"),
            None
        );
        assert_eq!(debug_ws_url_from_line("Chromium startup"), None);
    }

    #[test]
    fn chromium_launch_error_includes_or_omits_stderr_context() {
        assert_eq!(
            chromium_launch_error("failed", ""),
            "failed; Chromium emitted no stderr output"
        );
        assert_eq!(chromium_launch_error("failed", "detail"), "failed: detail");
    }

    #[cfg(unix)]
    #[test]
    fn chromium_stderr_close_summaries_distinguish_exited_and_active_processes() {
        let status = must(
            std::process::Command::new("sh")
                .args(["-c", "exit 7"])
                .status(),
        );

        assert!(chromium_stderr_closed_summary(Some(status)).contains("Chromium exited with"));
        assert_eq!(
            chromium_stderr_closed_summary(None),
            "Chromium closed stderr before exposing a DevTools endpoint"
        );
    }

    #[cfg(unix)]
    #[test]
    fn chromium_startup_errors_preserve_clock_and_stderr_close_failures() {
        let close_error =
            chromium_stderr_closed_status(Err(std::io::Error::other("close failed")), "detail");

        assert_eq!(chromium_timestamp(UNIX_EPOCH - Duration::from_nanos(1)), 0);
        assert_eq!(close_error, Err("close failed".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn chromium_startup_response_reports_disconnected_stderr() {
        let mut child = must(
            std::process::Command::new("sh")
                .args(["-c", "sleep 1"])
                .spawn(),
        );
        let (sender, responses) = std::sync::mpsc::channel();
        drop(sender);
        let mut output = "stderr detail\n".to_string();

        let error = must(
            chromium_startup_response(
                &responses,
                &mut child,
                Instant::now() + Duration::from_secs(1),
                &mut output,
            )
            .err()
            .ok_or("disconnected Chromium stderr was accepted"),
        );
        let _ = child.kill();
        let _ = child.wait();

        assert!(error.contains("Chromium closed stderr"));
        assert!(error.contains("stderr detail"));
    }

    #[test]
    fn chromium_profile_directory_uses_unique_temporary_paths() {
        let first = chromium_profile_directory();
        let second = chromium_profile_directory();

        assert!(first.starts_with(std::env::temp_dir()));
        assert_ne!(first, second);
    }

    #[test]
    fn must_reports_unexpected_test_errors() {
        let invalid_viewport = HtmlBrowserViewport::new(0, 2, 1.0);
        assert_panics(|| {
            let _: String = must(Err("boom"));
        });
        assert_panics(|| {
            let _: std::process::ExitStatus = must(Err(std::io::Error::other("boom")));
        });
        assert_panics(|| {
            let _ = must::<std::process::Child, std::io::Error>(Err(std::io::Error::other("boom")))
                .wait();
        });
        assert_panics(|| {
            let _: HtmlBrowserViewport = must(invalid_viewport);
        });
        assert_panics(|| {
            let _: () = must(Err("boom"));
        });
    }

    fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn assert_panics(action: impl FnOnce()) {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).is_err());
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }
}
