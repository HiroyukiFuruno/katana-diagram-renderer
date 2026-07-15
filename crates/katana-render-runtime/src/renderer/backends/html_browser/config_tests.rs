use super::*;
use serde_json::Value;
use std::fs;

#[test]
fn builder_records_explicit_chromium_binary() {
    let config = HtmlBrowserProcessConfig::new(PathBuf::from("helper"))
        .with_chromium_binary(PathBuf::from("chrome-for-testing"))
        .with_request_timeout(Duration::from_millis(45_000));

    assert_eq!(config.program, PathBuf::from("helper"));
    assert_eq!(config.args, Vec::<String>::new());
    assert_eq!(
        config.chromium_binary,
        Some(PathBuf::from("chrome-for-testing"))
    );
    assert_eq!(config.request_timeout(), Duration::from_millis(45_000));
}

#[test]
fn serialization_omits_default_chromium_binary_and_timeout() {
    let config = HtmlBrowserProcessConfig::new(PathBuf::from("helper"));
    let value = must(serde_json::to_value(config));

    assert_eq!(value.get("chromium_binary"), None);
    assert_eq!(value.get("request_timeout_ms"), None);
}

#[test]
fn serialization_includes_explicit_chromium_binary_and_timeout() {
    let config = HtmlBrowserProcessConfig::new(PathBuf::from("helper"))
        .with_chromium_binary(PathBuf::from("chrome-for-testing"))
        .with_request_timeout(Duration::from_millis(45_000));
    let value = must(serde_json::to_value(config));

    assert_eq!(
        value.get("chromium_binary").and_then(Value::as_str),
        Some("chrome-for-testing")
    );
    assert_eq!(
        value.get("request_timeout_ms").and_then(Value::as_u64),
        Some(45_000)
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
    assert_eq!(config.request_timeout_ms, DEFAULT_REQUEST_TIMEOUT_MS);

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
    assert_eq!(default_request_timeout_ms(), DEFAULT_REQUEST_TIMEOUT_MS);
    assert!(is_default_request_timeout_ms(&config.request_timeout_ms));
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
#[should_panic(expected = "unexpected test error: browser viewport dimensions must be non-zero")]
fn must_reports_unexpected_test_errors() {
    let _: () = must(Err(HtmlBrowserError::InvalidViewport));
}

#[test]
fn must_error_branch_covers_test_value_types() {
    assert!(
        std::panic::catch_unwind(|| {
            let _: HtmlBrowserProcessConfig = must::<HtmlBrowserProcessConfig, HtmlBrowserError>(
                Err(HtmlBrowserError::InvalidViewport),
            );
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
