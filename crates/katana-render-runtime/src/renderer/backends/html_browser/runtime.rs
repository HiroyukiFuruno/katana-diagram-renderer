use super::{
    HtmlBrowserError, HtmlBrowserProcessConfig, HtmlBrowserSession, HtmlBrowserSource,
    HtmlBrowserViewport,
};

/// Public HTML runtime entry point for browser-equivalent interactive surfaces.
#[derive(Debug, Clone, Copy)]
pub struct HtmlRuntime;

pub type HtmlRuntimeSession = HtmlBrowserSession;

impl HtmlRuntime {
    pub fn open(
        &self,
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        config: &HtmlBrowserProcessConfig,
    ) -> Result<HtmlRuntimeSession, HtmlBrowserError> {
        HtmlBrowserSession::start(source, viewport, config)
    }

    pub fn open_packaged(
        &self,
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<HtmlRuntimeSession, HtmlBrowserError> {
        HtmlBrowserSession::start(source, viewport, &HtmlBrowserProcessConfig::packaged()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::HTML_BROWSER_PROTOCOL_VERSION;
    #[cfg(unix)]
    use std::path::PathBuf;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    #[cfg(unix)]
    #[test]
    fn open_starts_browser_session_with_explicit_process_config() -> TestResult {
        let script = single_frame_script();
        let mut session = HtmlRuntime.open(test_source()?, viewport()?, &shell_config(&script))?;

        assert!(session.has_process());
        assert_eq!(
            session.latest_frame().map(|frame| frame.generation),
            Some(1)
        );
        session.close()?;
        Ok(())
    }

    #[test]
    fn open_packaged_reports_missing_helper_before_session_start() -> TestResult {
        let result = HtmlRuntime.open_packaged(test_source()?, viewport()?);

        assert!(matches!(
            result,
            Err(HtmlBrowserError::EngineBinaryNotFound { .. })
        ));
        Ok(())
    }

    #[test]
    fn html_runtime_traits_are_value_like() {
        let runtime = HtmlRuntime;
        let copied = runtime;
        let cloned = <HtmlRuntime as Clone>::clone(&copied);

        assert_eq!(format!("{runtime:?}"), "HtmlRuntime");
        assert_eq!(format!("{cloned:?}"), format!("{copied:?}"));
    }

    #[test]
    fn test_source_helper_propagates_invalid_origin() {
        assert!(matches!(
            test_source_with_origin("not a url"),
            Err(error)
                if error
                    .downcast_ref::<HtmlBrowserError>()
                    .is_some_and(|error| matches!(error, HtmlBrowserError::InvalidOrigin { .. }))
        ));
    }

    fn test_source() -> TestResult<HtmlBrowserSource> {
        test_source_with_origin("https://example.test/index.html")
    }

    fn test_source_with_origin(origin: &str) -> TestResult<HtmlBrowserSource> {
        Ok(HtmlBrowserSource::new("<p>ok</p>", origin)?)
    }

    fn viewport() -> TestResult<HtmlBrowserViewport> {
        Ok(HtmlBrowserViewport::new(2, 2, 1.0)?)
    }

    #[cfg(unix)]
    fn shell_config(script: &str) -> HtmlBrowserProcessConfig {
        let mut config = HtmlBrowserProcessConfig::new(PathBuf::from("/bin/sh"));
        config.args = vec!["-c".to_string(), script.to_string()];
        config
    }

    #[cfg(unix)]
    fn single_frame_script() -> String {
        let response = frame_response_json();
        format!(
            r#"IFS= read -r _request
printf '%s\n' '{response}'
IFS= read -r _request
printf '%s\n' '{{"result":"closed","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION}}}'
"#
        )
    }

    #[cfg(unix)]
    fn frame_response_json() -> String {
        serde_json::json!({
            "result": "frame",
            "protocol_version": HTML_BROWSER_PROTOCOL_VERSION,
            "frame": {
                "generation": 1,
                "origin": "https://example.test/index.html",
                "viewport": {
                    "width": 2,
                    "height": 2,
                    "device_scale_factor": 1.0
                },
                "pixel_format": "Rgba8",
                "pixels": [0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255]
            }
        })
        .to_string()
    }
}
