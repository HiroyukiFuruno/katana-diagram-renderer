use super::HtmlBrowserError;
use std::path::PathBuf;

const HTML_BROWSER_ENGINE_ENV: &str = "KRR_HTML_BROWSER_ENGINE";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HtmlBrowserProcessConfig {
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chromium_binary: Option<PathBuf>,
}

impl HtmlBrowserProcessConfig {
    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            args: Vec::new(),
            chromium_binary: None,
        }
    }

    pub fn with_chromium_binary(mut self, binary: PathBuf) -> Self {
        self.chromium_binary = Some(binary);
        self
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

#[cfg(target_os = "windows")]
fn packaged_engine_name() -> &'static str {
    "krr-html-chromium-engine.exe"
}

#[cfg(not(target_os = "windows"))]
fn packaged_engine_name() -> &'static str {
    "krr-html-chromium-engine"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn builder_records_explicit_chromium_binary() {
        let config = HtmlBrowserProcessConfig::new(PathBuf::from("helper"))
            .with_chromium_binary(PathBuf::from("chrome-for-testing"));

        assert_eq!(config.program, PathBuf::from("helper"));
        assert_eq!(config.args, Vec::<String>::new());
        assert_eq!(
            config.chromium_binary,
            Some(PathBuf::from("chrome-for-testing"))
        );
    }

    #[test]
    fn serialization_omits_default_chromium_binary() {
        let config = HtmlBrowserProcessConfig::new(PathBuf::from("helper"));
        let value = must(serde_json::to_value(config));

        assert_eq!(value.get("chromium_binary"), None);
    }

    #[test]
    fn serialization_includes_explicit_chromium_binary() {
        let config = HtmlBrowserProcessConfig::new(PathBuf::from("helper"))
            .with_chromium_binary(PathBuf::from("chrome-for-testing"));
        let value = must(serde_json::to_value(config));

        assert_eq!(
            value.get("chromium_binary").and_then(Value::as_str),
            Some("chrome-for-testing")
        );
    }

    #[test]
    fn packaged_uses_environment_override() {
        unsafe { std::env::set_var(HTML_BROWSER_ENGINE_ENV, "/tmp/krr-test-helper") };
        let result = HtmlBrowserProcessConfig::packaged();
        unsafe { std::env::remove_var(HTML_BROWSER_ENGINE_ENV) };
        let config = must(result);
        assert_eq!(config.program, PathBuf::from("/tmp/krr-test-helper"));
        assert_eq!(config.args, Vec::<String>::new());
        assert_eq!(config.chromium_binary, None);

        assert!(matches!(
            HtmlBrowserProcessConfig::packaged(),
            Err(HtmlBrowserError::EngineBinaryNotFound { .. })
        ));
    }

    #[test]
    fn packaged_adjacent_to_reports_parentless_executable() {
        let result = HtmlBrowserProcessConfig::packaged_adjacent_to(PathBuf::new());

        assert_eq!(
            result,
            Err(HtmlBrowserError::EnginePath {
                error: "current executable has no parent directory".to_string()
            })
        );
    }

    #[test]
    fn packaged_adjacent_to_reports_missing_helper() {
        let missing =
            std::env::temp_dir().join(format!("krr-missing-browser-helper-{}", std::process::id()));
        let helper = missing.join(packaged_engine_name());

        assert_eq!(
            HtmlBrowserProcessConfig::packaged_adjacent_to(missing.join("test-runner")),
            Err(HtmlBrowserError::EngineBinaryNotFound {
                path: helper.display().to_string()
            })
        );
    }

    #[test]
    fn packaged_adjacent_to_uses_existing_helper() {
        let directory = std::env::temp_dir().join(format!(
            "krr-existing-browser-helper-{}",
            std::process::id()
        ));
        must(fs::create_dir_all(&directory));
        let helper = directory.join(packaged_engine_name());
        must(fs::write(&helper, b"helper"));

        let config = must(HtmlBrowserProcessConfig::packaged_adjacent_to(
            directory.join("test-runner"),
        ));
        let _ = fs::remove_file(&helper);
        let _ = fs::remove_dir(&directory);

        assert_eq!(config.program, helper);
        assert_eq!(config.args, Vec::<String>::new());
        assert_eq!(config.chromium_binary, None);
    }

    #[test]
    fn engine_path_error_preserves_message() {
        assert_eq!(
            engine_path_error(std::io::Error::other("boom")),
            HtmlBrowserError::EnginePath {
                error: "boom".to_string()
            }
        );
    }

    #[test]
    #[should_panic(
        expected = "unexpected test error: browser viewport dimensions must be non-zero"
    )]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err(HtmlBrowserError::InvalidViewport));
    }

    #[test]
    fn must_error_branch_covers_test_value_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: HtmlBrowserProcessConfig = must::<HtmlBrowserProcessConfig, HtmlBrowserError>(Err(
                    HtmlBrowserError::InvalidViewport,
                ));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: Value = must::<Value, serde_json::Error>(Err(serde_json::Error::io(
                    std::io::Error::other("boom"),
                )));
            })
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(|| {
                let _: () = must::<(), std::io::Error>(Err(std::io::Error::other("boom")));
            })
            .is_err()
        );
    }

    fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }
}
