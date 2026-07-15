use super::HtmlBrowserError;
use std::{path::PathBuf, time::Duration};

const HTML_BROWSER_ENGINE_ENV: &str = "KRR_HTML_BROWSER_ENGINE";
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 15_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HtmlBrowserProcessConfig {
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chromium_binary: Option<PathBuf>,
    #[serde(
        default = "default_request_timeout_ms",
        skip_serializing_if = "is_default_request_timeout_ms"
    )]
    pub request_timeout_ms: u64,
}

impl HtmlBrowserProcessConfig {
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            args: Vec::new(),
            chromium_binary: None,
            request_timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
        }
    }

    pub fn with_chromium_binary(mut self, binary: PathBuf) -> Self {
        self.chromium_binary = Some(binary);
        self
    }

    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout_ms = timeout.as_millis().try_into().unwrap_or(u64::MAX);
        self
    }

    pub(crate) fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }

    pub fn packaged() -> Result<Self, HtmlBrowserError> {
        if let Some(program) = std::env::var_os(HTML_BROWSER_ENGINE_ENV) {
            return Ok(Self::new(program.into()));
        }
        current_executable().and_then(Self::packaged_adjacent_to)
    }

    fn packaged_adjacent_to(executable: PathBuf) -> Result<Self, HtmlBrowserError> {
        let directory = executable.parent().ok_or(HtmlBrowserError::EnginePath {
            error: "current executable has no parent directory".into(),
        })?;
        let program = directory.join(packaged_engine_name());
        if !program.is_file() {
            return Err(HtmlBrowserError::EngineBinaryNotFound {
                path: program.display().to_string(),
            });
        }
        Ok(Self::new(program))
    }
}

fn current_executable() -> Result<PathBuf, HtmlBrowserError> {
    std::env::current_exe().map_err(engine_path_error)
}

fn engine_path_error(error: std::io::Error) -> HtmlBrowserError {
    HtmlBrowserError::EnginePath {
        error: error.to_string(),
    }
}

fn default_request_timeout_ms() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_MS
}

fn is_default_request_timeout_ms(timeout_ms: &u64) -> bool {
    *timeout_ms == DEFAULT_REQUEST_TIMEOUT_MS
}

#[cfg(target_os = "windows")]
fn packaged_engine_name() -> &'static str {
    "krr-html-chromium-engine.exe"
}

#[cfg(not(target_os = "windows"))]
fn packaged_engine_name() -> &'static str {
    "krr-html-chromium-engine"
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
