use super::chromium_args::chromium_arguments;
use super::chromium_startup::{
    CHROMIUM_STARTUP_TIMEOUT, append_chromium_output, chromium_launch_error,
    chromium_profile_directory, wait_for_debug_ws_url,
};
use super::trace;
use crate::{HtmlBrowserViewport, system::ProcessService};
use headless_chrome::Browser;
use std::{
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Stdio},
    time::Duration,
};

const BROWSER_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const BROWSER_DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) struct ChromiumProcess {
    child: Child,
    profile_directory: PathBuf,
}

impl ChromiumProcess {
    fn launch(
        chrome_binary: &Path,
        viewport: HtmlBrowserViewport,
    ) -> Result<(Self, String), String> {
        Self::launch_with_timeout(chrome_binary, viewport, CHROMIUM_STARTUP_TIMEOUT)
    }

    pub(super) fn launch_with_timeout(
        chrome_binary: &Path,
        viewport: HtmlBrowserViewport,
        startup_timeout: std::time::Duration,
    ) -> Result<(Self, String), String> {
        trace::stage("chromium-process:launch:start");
        let profile_directory = chromium_profile_directory();
        let mut command = ProcessService::create_command(chrome_binary);
        command
            .args(chromium_arguments(&profile_directory, viewport))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        trace::stage("chromium-process:spawn");
        spawn_chromium(&mut command, &profile_directory).and_then(|child| {
            trace::stage("chromium-process:connect-started");
            Self::connect_to_started_chromium(
                Self {
                    child,
                    profile_directory,
                },
                startup_timeout,
            )
        })
    }

    fn connect_to_started_chromium(
        mut chromium: Self,
        startup_timeout: std::time::Duration,
    ) -> Result<(Self, String), String> {
        let stderr = chromium.take_stderr()?;
        trace::stage("chromium-process:wait-devtools");
        let debug_ws_url = wait_for_debug_ws_url(&mut chromium.child, stderr, startup_timeout)?;
        trace::stage("chromium-process:devtools-ready");
        Ok((chromium, debug_ws_url))
    }

    fn take_stderr(&mut self) -> Result<ChildStderr, String> {
        self.child
            .stderr
            .take()
            .ok_or("Chromium stderr was not piped".to_string())
    }
}

fn spawn_chromium(
    command: &mut std::process::Command,
    profile_directory: &Path,
) -> Result<Child, String> {
    command.spawn().map_err(|error| {
        let _ = std::fs::remove_dir_all(profile_directory);
        error.to_string()
    })
}

impl Drop for ChromiumProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile_directory);
    }
}

pub(super) fn chromium_is_running(
    responses: &std::sync::mpsc::Receiver<String>,
    child: &mut Child,
    output: &mut String,
) -> Result<Option<String>, String> {
    chromium_is_running_after_status(child.try_wait(), responses, output)
}

fn chromium_is_running_after_status(
    status: Result<Option<std::process::ExitStatus>, std::io::Error>,
    responses: &std::sync::mpsc::Receiver<String>,
    output: &mut String,
) -> Result<Option<String>, String> {
    let status = chromium_is_running_status(status)?;
    let Some(status) = status else {
        return Ok(None);
    };
    match responses.recv() {
        Ok(line) => {
            append_chromium_output(output, &line);
            Ok(Some(line))
        }
        Err(_) => Err(chromium_launch_error(
            &format!("Chromium exited with {status}"),
            output,
        )),
    }
}

pub(super) fn chromium_is_running_status(
    status: Result<Option<std::process::ExitStatus>, std::io::Error>,
) -> Result<Option<std::process::ExitStatus>, String> {
    match status {
        Ok(status) => Ok(status),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn launch_chromium(
    chrome_binary: &Path,
    viewport: HtmlBrowserViewport,
) -> Result<(Browser, ChromiumProcess, String), String> {
    trace::stage("chromium:launch");
    let (chromium, debug_ws_url) = match ChromiumProcess::launch(chrome_binary, viewport) {
        Ok((chromium, debug_ws_url)) => (chromium, debug_ws_url),
        Err(error) => return Err(error),
    };
    trace::stage("chromium:connect-browser");
    let browser = connect_browser(debug_ws_url.clone())?;
    trace::stage("chromium:browser-connected");
    Ok((browser, chromium, debug_ws_url))
}

fn connect_browser(debug_ws_url: String) -> Result<Browser, String> {
    let browser =
        Browser::connect_with_timeout(debug_ws_url, BROWSER_IDLE_TIMEOUT).map_err(string_error)?;
    browser.set_default_timeout(BROWSER_DEFAULT_TIMEOUT);
    Ok(browser)
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[cfg(unix)]
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    #[cfg(unix)]
    static CHROMIUM_SCRIPT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    #[cfg(unix)]
    #[test]
    fn chromium_process_reports_startup_timeout_without_an_endpoint() {
        let chrome = chromium_script("sleep 1");
        let result = ChromiumProcess::launch_with_timeout(
            &chrome,
            must(HtmlBrowserViewport::new(2, 2, 1.0)),
            std::time::Duration::from_millis(1),
        );
        let _ = std::fs::remove_file(&chrome);
        let error = must(result.err().ok_or("Chromium launch unexpectedly succeeded"));

        assert_eq!(
            error,
            "Chromium did not expose a DevTools endpoint; Chromium emitted no stderr output"
        );
    }

    #[cfg(unix)]
    #[test]
    fn chromium_process_reports_missing_stderr_pipe() {
        let profile_directory = chromium_profile_directory();
        let child = must(
            std::process::Command::new("sh")
                .args(["-c", "sleep 1"])
                .stderr(Stdio::null())
                .spawn(),
        );
        let chromium = ChromiumProcess {
            child,
            profile_directory,
        };
        let error = must(
            ChromiumProcess::connect_to_started_chromium(
                chromium,
                std::time::Duration::from_millis(1),
            )
            .err()
            .ok_or("Chromium stderr pipe was accepted"),
        );

        assert_eq!(error, "Chromium stderr was not piped");
    }

    #[test]
    fn chromium_process_reports_refused_devtools_connection() {
        assert!(connect_browser("ws://127.0.0.1:0/devtools/browser/test".to_string()).is_err());
    }

    #[test]
    fn browser_connection_timeouts_cover_slow_ci_chromium() {
        assert!(CHROMIUM_STARTUP_TIMEOUT >= std::time::Duration::from_secs(30));
        assert!(BROWSER_IDLE_TIMEOUT > CHROMIUM_STARTUP_TIMEOUT);
        assert!(BROWSER_IDLE_TIMEOUT > BROWSER_DEFAULT_TIMEOUT);
        assert!(BROWSER_DEFAULT_TIMEOUT >= std::time::Duration::from_secs(120));
    }

    #[test]
    fn chromium_process_removes_profile_directory_when_spawn_fails() {
        let profile_directory = chromium_profile_directory();
        must(std::fs::create_dir(&profile_directory));
        let missing_chromium = profile_directory.join("missing-chromium");
        let mut command = std::process::Command::new(missing_chromium);
        let error = must(
            spawn_chromium(&mut command, &profile_directory)
                .err()
                .ok_or("Chromium spawn unexpectedly succeeded"),
        );

        assert!(!error.is_empty());
        assert!(!profile_directory.exists());
    }

    #[cfg(unix)]
    #[test]
    fn chromium_process_drains_stderr_after_the_child_exits() {
        let mut child = must(
            std::process::Command::new("sh")
                .args(["-c", "exit 7"])
                .spawn(),
        );
        let _ = must(child.wait());
        let (sender, responses) = mpsc::channel();
        must(sender.send("startup failed".to_string()));
        drop(sender);
        let mut output = String::new();

        assert_eq!(
            chromium_is_running(&responses, &mut child, &mut output),
            Ok(Some("startup failed".to_string()))
        );
        assert_eq!(output, "startup failed\n");
    }

    #[cfg(unix)]
    #[test]
    fn chromium_process_reports_child_exit_when_stderr_is_empty() {
        let mut child = must(
            std::process::Command::new("sh")
                .args(["-c", "exit 7"])
                .spawn(),
        );
        let _ = must(child.wait());
        let (sender, responses) = mpsc::channel();
        drop(sender);
        let mut output = "prior stderr\n".to_string();

        let error = must(
            chromium_is_running(&responses, &mut child, &mut output)
                .err()
                .ok_or("Chromium exit was accepted without stderr"),
        );

        assert!(error.contains("Chromium exited with"));
        assert!(error.contains("prior stderr"));
    }

    #[test]
    fn chromium_process_preserves_try_wait_failures() {
        let (_sender, responses) = mpsc::channel();
        let mut output = String::new();
        let error = chromium_is_running_after_status(
            Err(std::io::Error::other("wait failed")),
            &responses,
            &mut output,
        );

        assert_eq!(error, Err("wait failed".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn chromium_process_stops_when_devtools_endpoint_refuses_connections() {
        let chrome = chromium_script(
            "echo 'DevTools listening on ws://127.0.0.1:0/devtools/browser/test' >&2\nsleep 1",
        );
        let result = launch_chromium(&chrome, must(HtmlBrowserViewport::new(2, 2, 1.0)));
        let _ = std::fs::remove_file(&chrome);

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn chromium_process_stops_when_launch_chromium_exits_before_devtools_endpoint() {
        let chrome = chromium_script("exit 7");
        let result = launch_chromium(&chrome, must(HtmlBrowserViewport::new(2, 2, 1.0)));
        let _ = std::fs::remove_file(&chrome);
        let error = must(result.err().ok_or("Chromium launch unexpectedly succeeded"));

        assert!(!error.is_empty());
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
            let _: std::time::Duration = must(UNIX_EPOCH.duration_since(SystemTime::now()));
        });
        assert_panics(|| {
            let _: HtmlBrowserViewport = must(invalid_viewport);
        });
        assert_panics(|| {
            let _: () = must(Err(mpsc::SendError("boom".to_string())));
        });
        assert_panics(|| {
            let _: () = must(Err(std::io::Error::other("boom")));
        });
        assert_panics(|| {
            let _: () = must(Err("boom"));
        });
    }

    #[cfg(unix)]
    fn chromium_script(body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let sequence = CHROMIUM_SCRIPT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "krr-page-test-chromium-{}-{}-{}",
            std::process::id(),
            must(SystemTime::now().duration_since(UNIX_EPOCH)).as_nanos(),
            sequence
        ));
        must(std::fs::write(&path, format!("#!/bin/sh\n{body}\n")));
        must(std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(0o700),
        ));
        path
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
