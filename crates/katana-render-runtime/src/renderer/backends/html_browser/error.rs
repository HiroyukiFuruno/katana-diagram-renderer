use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlBrowserEngineErrorCode {
    InvalidMessage,
    ProtocolVersion,
    InvalidRequest,
    Chromium,
    NotLoaded,
    StdinRead,
    Unknown(String),
}

impl HtmlBrowserEngineErrorCode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidMessage => "invalid_message",
            Self::ProtocolVersion => "protocol_version",
            Self::InvalidRequest => "invalid_request",
            Self::Chromium => "chromium",
            Self::NotLoaded => "not_loaded",
            Self::StdinRead => "stdin_read",
            Self::Unknown(code) => code,
        }
    }
}

impl From<String> for HtmlBrowserEngineErrorCode {
    fn from(code: String) -> Self {
        match code.as_str() {
            "invalid_message" => Self::InvalidMessage,
            "protocol_version" => Self::ProtocolVersion,
            "invalid_request" => Self::InvalidRequest,
            "chromium" => Self::Chromium,
            "not_loaded" => Self::NotLoaded,
            "stdin_read" => Self::StdinRead,
            _ => Self::Unknown(code),
        }
    }
}

impl std::fmt::Display for HtmlBrowserEngineErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HtmlBrowserError {
    #[error("browser source exceeds {max_bytes} bytes: {actual_bytes}")]
    SourceTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("browser origin is not a valid absolute URL: {origin}")]
    InvalidOrigin { origin: String },
    #[error("browser origin uses an unsupported scheme: {origin}")]
    UnsupportedOriginScheme { origin: String },
    #[error("browser viewport dimensions must be non-zero")]
    InvalidViewport,
    #[error("browser viewport device scale factor must be finite and positive")]
    InvalidDeviceScaleFactor,
    #[error("browser input coordinates must be finite")]
    InvalidInputCoordinates,
    #[error("browser frame dimensions overflow the address space")]
    FrameDimensionsOverflow,
    #[error("browser frame buffer size is {actual_bytes}, expected {expected_bytes}")]
    InvalidFrameBufferSize {
        actual_bytes: usize,
        expected_bytes: usize,
    },
    #[error(
        "browser frame origin does not match document origin: expected {expected}, got {actual}"
    )]
    FrameOriginMismatch { expected: String, actual: String },
    #[error("browser frame generation {received} is not newer than {latest}")]
    StaleFrameGeneration { latest: u64, received: u64 },
    #[error("browser session is closed")]
    SessionClosed,
    #[error("browser engine process has not been started")]
    EngineNotStarted,
    #[error("browser IPC protocol version mismatch: expected {expected}, got {actual}")]
    ProtocolVersionMismatch { expected: u32, actual: u32 },
    #[error("failed to encode browser IPC message: {error}")]
    ProtocolEncode { error: String },
    #[error("browser process returned invalid JSON: {error}")]
    InvalidProcessMessage { error: String },
    #[error("browser process returned an unexpected response: {response}")]
    UnexpectedProcessResponse { response: String },
    #[error("browser process rejected the request ({code}): {message}")]
    EngineRejected {
        code: HtmlBrowserEngineErrorCode,
        message: String,
    },
    #[error("failed to start browser process: {error}")]
    ProcessStart { error: String },
    #[error("failed to resolve the packaged browser engine: {error}")]
    EnginePath { error: String },
    #[error("packaged browser engine was not found at {path}")]
    EngineBinaryNotFound { path: String },
    #[error("failed to write to browser process: {error}")]
    ProcessWrite { error: String },
    #[error("failed to read browser process output: {error}")]
    ProcessRead { error: String },
    #[error("browser process did not respond within {timeout_ms}ms")]
    ProcessTimeout { timeout_ms: u64 },
    #[error("browser process crashed: {status}")]
    ProcessCrashed { status: String },
    #[error("failed to terminate browser process: {error}")]
    ProcessTerminate { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_engine_error_codes_map_wire_codes_and_display_names() {
        let cases = [
            (
                "invalid_message",
                HtmlBrowserEngineErrorCode::InvalidMessage,
            ),
            (
                "protocol_version",
                HtmlBrowserEngineErrorCode::ProtocolVersion,
            ),
            (
                "invalid_request",
                HtmlBrowserEngineErrorCode::InvalidRequest,
            ),
            ("chromium", HtmlBrowserEngineErrorCode::Chromium),
            ("not_loaded", HtmlBrowserEngineErrorCode::NotLoaded),
            ("stdin_read", HtmlBrowserEngineErrorCode::StdinRead),
        ];
        for (wire, expected) in cases {
            let code = HtmlBrowserEngineErrorCode::from(wire.to_string());
            assert_eq!(code, expected);
            assert_eq!(code.as_str(), wire);
            assert_eq!(code.to_string(), wire);
        }
    }

    #[test]
    fn unknown_engine_error_code_preserves_wire_code() {
        let unknown = HtmlBrowserEngineErrorCode::from("future_code".to_string());
        assert_eq!(
            unknown,
            HtmlBrowserEngineErrorCode::Unknown("future_code".to_string())
        );
        assert_eq!(unknown.as_str(), "future_code");
        assert_eq!(unknown.to_string(), "future_code");
    }
}
