use thiserror::Error;

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
    #[error("the in-process HTML runtime is unavailable")]
    RuntimeNotStarted,
    #[error("in-process HTML runtime failed: {error}")]
    RuntimeFailure { error: String },
}
