use super::{HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserEngineErrorCode, HtmlBrowserError};

pub(super) fn validate_protocol(version: u32) -> Result<(), HtmlBrowserError> {
    if version == HTML_BROWSER_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(HtmlBrowserError::ProtocolVersionMismatch {
            expected: HTML_BROWSER_PROTOCOL_VERSION,
            actual: version,
        })
    }
}

pub(super) fn engine_error(
    version: u32,
    code: String,
    message: String,
) -> Result<(), HtmlBrowserError> {
    validate_protocol(version)?;
    Err(HtmlBrowserError::EngineRejected {
        code: HtmlBrowserEngineErrorCode::from(code),
        message,
    })
}

pub(super) fn closed_response(version: u32) -> Result<(), HtmlBrowserError> {
    validate_protocol(version)?;
    Err(HtmlBrowserError::UnexpectedProcessResponse {
        response: "closed".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_helpers_reject_mismatches_and_engine_errors() {
        assert!(matches!(
            validate_protocol(HTML_BROWSER_PROTOCOL_VERSION + 1),
            Err(HtmlBrowserError::ProtocolVersionMismatch { .. })
        ));
        assert!(matches!(
            engine_error(
                HTML_BROWSER_PROTOCOL_VERSION,
                "chromium".to_string(),
                "boom".to_string()
            ),
            Err(HtmlBrowserError::EngineRejected { code, message })
                if code == HtmlBrowserEngineErrorCode::Chromium && message == "boom"
        ));
        assert!(matches!(
            closed_response(HTML_BROWSER_PROTOCOL_VERSION),
            Err(HtmlBrowserError::UnexpectedProcessResponse { response })
                if response == "closed"
        ));
    }
}
