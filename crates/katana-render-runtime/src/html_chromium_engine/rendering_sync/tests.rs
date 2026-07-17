use super::scripts::{
    EXPECTED_URL_PLACEHOLDER, LOADED_RENDERING_READY_SCRIPT, RENDERING_READY_SCRIPT,
};
use super::*;
use serde_json::Value;
use std::time::Duration;

#[test]
fn rendering_sync_waits_for_two_animation_frames_with_timeout_fallback() {
    assert_eq!(
        RENDERING_READY_SCRIPT
            .matches("requestAnimationFrame")
            .count(),
        2
    );
    assert!(RENDERING_READY_SCRIPT.contains("setTimeout(resolve, 0)"));
    assert!(RENDERING_READY_SCRIPT.contains("setTimeout(finish, 100)"));
    assert!(RENDERING_READY_SCRIPT.contains("getBoundingClientRect"));
}

#[test]
fn rendering_sync_capture_script_uses_only_paint_barrier() {
    assert!(!RENDERING_READY_SCRIPT.contains("DOCUMENT_READY_TIMEOUT_MS"));
    assert!(!RENDERING_READY_SCRIPT.contains("RESOURCE_READY_TIMEOUT_MS"));
    assert!(!RENDERING_READY_SCRIPT.contains("window.addEventListener('load'"));
}

#[test]
fn rendering_sync_waits_for_document_scripts_before_resource_paint() {
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("location.href !== expectedUrl"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("document.readyState === 'complete'"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("window.addEventListener('load'"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("DOCUMENT_READY_TIMEOUT_MS = 2000"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("SCRIPT_READY_TIMEOUT_MS = 2000"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("document.scripts"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("script.addEventListener('load'"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("script.readyState === 'complete'"));
}

#[test]
fn rendering_sync_waits_for_stylesheet_and_image_resources() {
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("waitForResourceEvent"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("RESOURCE_READY_TIMEOUT_MS = 2000"));
    assert!(
        LOADED_RENDERING_READY_SCRIPT.contains("setTimeout(finish, RESOURCE_READY_TIMEOUT_MS)")
    );
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("link[rel~=\"stylesheet\"]"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("cssRules"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("document.images"));
    assert!(LOADED_RENDERING_READY_SCRIPT.contains("image.decode"));
}

#[test]
fn loaded_rendering_script_injects_expected_url_as_json() {
    let script = loaded_rendering_ready_script("file:///tmp/document.html?name=\"quoted\"");

    assert!(!script.contains(EXPECTED_URL_PLACEHOLDER));
    assert!(script.contains(r#"file:///tmp/document.html?name=\"quoted\""#));
}

#[test]
fn loaded_rendering_ready_value_accepts_only_boolean_true() {
    assert!(is_ready_value(Some(Value::Bool(true))));
    assert!(!is_ready_value(Some(Value::Bool(false))));
    assert!(!is_ready_value(Some(Value::String("true".to_string()))));
    assert!(!is_ready_value(None));
}

#[test]
fn loaded_rendering_retry_accepts_ready_after_pending_document() {
    let mut attempts = 0;
    let mut evaluate = || {
        attempts += 1;
        Ok(attempts == 2)
    };

    assert_eq!(
        retry_loaded_rendering_sync(&mut evaluate, Duration::from_secs(1), Duration::ZERO,),
        Ok(())
    );

    assert_eq!(attempts, 2);
}

#[test]
fn loaded_rendering_retry_retries_transient_evaluation_errors() {
    let mut attempts = 0;
    let mut evaluate = || {
        attempts += 1;
        if attempts == 1 {
            Err("execution context was destroyed".to_string())
        } else {
            Ok(true)
        }
    };

    assert_eq!(
        retry_loaded_rendering_sync(&mut evaluate, Duration::from_secs(1), Duration::ZERO,),
        Ok(())
    );

    assert_eq!(attempts, 2);
}

#[test]
fn loaded_rendering_retry_reports_pending_document_timeout() {
    let mut evaluate = || Ok(false);

    assert_eq!(
        retry_loaded_rendering_sync(&mut evaluate, Duration::ZERO, Duration::ZERO),
        Err("browser document did not reach expected URL".to_string())
    );
}

#[test]
fn loaded_rendering_retry_reports_last_evaluation_error_on_timeout() {
    let mut evaluate = || Err("execution context was destroyed".to_string());

    assert_eq!(
        retry_loaded_rendering_sync(&mut evaluate, Duration::ZERO, Duration::ZERO,),
        Err("execution context was destroyed".to_string())
    );
}
