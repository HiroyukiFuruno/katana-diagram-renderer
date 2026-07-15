use super::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserCommand, HtmlBrowserError, HtmlBrowserProcessConfig,
    HtmlBrowserRequest, HtmlBrowserResponse,
};
use crate::system::ProcessService;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

const CHROMIUM_BINARY_ENV: &str = "KRR_CHROME_BIN";

#[derive(Debug)]
pub struct HtmlBrowserProcess {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<Result<String, String>>,
    request_timeout: Duration,
}

impl HtmlBrowserProcess {
    pub fn spawn(config: &HtmlBrowserProcessConfig) -> Result<Self, HtmlBrowserError> {
        let mut child = Self::spawn_child(config)?;
        let stdin = child.stdin.take().ok_or_else(missing_stdin_error)?;
        let stdout = child.stdout.take().ok_or_else(missing_stdout_error)?;
        Ok(Self {
            child,
            stdin,
            responses: Self::read_responses(stdout),
            request_timeout: config.request_timeout(),
        })
    }

    fn spawn_child(config: &HtmlBrowserProcessConfig) -> Result<Child, HtmlBrowserError> {
        let mut command = ProcessService::create_command(&config.program);
        command.args(&config.args);
        if let Some(chromium_binary) = &config.chromium_binary {
            command.env(CHROMIUM_BINARY_ENV, chromium_binary);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(process_start_error)
    }

    fn read_responses(
        stdout: impl std::io::Read + Send + 'static,
    ) -> Receiver<Result<String, String>> {
        let (sender, responses) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender
                    .send(line.map_err(|error| error.to_string()))
                    .is_err()
                {
                    break;
                }
            }
        });
        responses
    }

    pub fn request(
        &mut self,
        command: HtmlBrowserCommand,
    ) -> Result<HtmlBrowserResponse, HtmlBrowserError> {
        let encoded = Self::encode_request(command)?;
        self.stdin
            .write_all(encoded.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(process_write_error)?;
        match self.responses.recv_timeout(self.request_timeout) {
            Ok(line) => Self::decode_response_line(line),
            Err(mpsc::RecvTimeoutError::Timeout) => self.timeout(),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(self.child_crashed()),
        }
    }

    fn encode_request(command: HtmlBrowserCommand) -> Result<String, HtmlBrowserError> {
        serde_json::to_string(&HtmlBrowserRequest {
            protocol_version: HTML_BROWSER_PROTOCOL_VERSION,
            command,
        })
        .map_err(protocol_encode_error)
    }

    fn decode_response_line(
        line: Result<String, String>,
    ) -> Result<HtmlBrowserResponse, HtmlBrowserError> {
        match line {
            Ok(line) => serde_json::from_str(&line).map_err(invalid_process_message_error),
            Err(error) => Err(process_read_error(error)),
        }
    }
}

fn process_start_error(error: std::io::Error) -> HtmlBrowserError {
    HtmlBrowserError::ProcessStart {
        error: error.to_string(),
    }
}

fn process_write_error(error: std::io::Error) -> HtmlBrowserError {
    HtmlBrowserError::ProcessWrite {
        error: error.to_string(),
    }
}

fn process_read_error(error: String) -> HtmlBrowserError {
    HtmlBrowserError::ProcessRead { error }
}

fn protocol_encode_error(error: impl ToString) -> HtmlBrowserError {
    HtmlBrowserError::ProtocolEncode {
        error: error.to_string(),
    }
}

fn invalid_process_message_error(error: impl ToString) -> HtmlBrowserError {
    HtmlBrowserError::InvalidProcessMessage {
        error: error.to_string(),
    }
}

fn missing_stdin_error() -> HtmlBrowserError {
    HtmlBrowserError::ProcessStart {
        error: "browser stdin was not piped".into(),
    }
}

fn missing_stdout_error() -> HtmlBrowserError {
    HtmlBrowserError::ProcessStart {
        error: "browser stdout was not piped".into(),
    }
}

#[path = "process_lifecycle.rs"]
mod process_lifecycle;
#[cfg(test)]
use process_lifecycle::{child_crashed_from_status, process_terminate_error};
#[cfg(all(test, unix))]
use std::process::ExitStatus;

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
