use super::script::{HtmlTryCatchScope, evaluate_value};
use super::types::HtmlRuntimeError;

pub(super) fn event<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    node_id: u64,
    event_type: &str,
    event_key: Option<&str>,
) -> Result<v8::Local<'scope, v8::Object>, HtmlRuntimeError> {
    let key = event_key
        .map(|key| serde_json::to_string(key).map_err(event_serialization_error))
        .transpose()?
        .unwrap_or_else(|| "null".to_string());
    let (bubbles, cancelable) = event_flags(event_type);
    let source = format!(
        "__krrDispatchHostEvent('{node_id}', {}, {key}, {bubbles}, {cancelable})",
        serde_json::to_string(event_type).map_err(event_serialization_error)?
    );
    let value = evaluate_value(scope, "krr-html-dom-event", &source)?;
    let event = v8::Local::<v8::Object>::try_from(value).map_err(event_error)?;
    Ok(event)
}

fn event_flags(event_type: &str) -> (bool, bool) {
    match event_type {
        "focus" | "blur" | "toggle" => (false, false),
        "load" | "DOMContentLoaded" | "readystatechange" => (false, false),
        "click" | "keydown" | "keyup" => (true, true),
        _ => (true, false),
    }
}

fn event_serialization_error(error: serde_json::Error) -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge(format!("HTML event serialization failed: {error}"))
}

pub(super) fn event_default_prevented(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    event: v8::Local<'_, v8::Object>,
) -> Result<bool, HtmlRuntimeError> {
    let key = v8::String::new(scope, "defaultPrevented").ok_or(HtmlRuntimeError::DomBridge(
        "click event default key allocation failed".to_string(),
    ))?;
    event
        .get(scope, key.into())
        .map(|value| value.is_true())
        .ok_or(HtmlRuntimeError::JavaScriptException(
            "click event defaultPrevented lookup failed".to_string(),
        ))
}

fn event_error<T>(_error: T) -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML event allocation failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_error_helpers_preserve_contract_messages() {
        assert!(matches!(
            event_error(()),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event allocation failed"
        ));
    }

    #[test]
    fn interaction_v8_data_error_helpers_preserve_contract_messages() {
        let error = v8::DataError::BadType {
            actual: "actual",
            expected: "expected",
        };

        assert!(matches!(
            event_error(error),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event allocation failed"
        ));
    }

    #[test]
    fn interaction_serialization_error_preserves_contract_message() {
        let error = event_serialization_error(serde_json::Error::io(std::io::Error::other(
            "serialization failed",
        )));
        assert!(matches!(
            error,
            HtmlRuntimeError::DomBridge(message) if message == "HTML event serialization failed: serialization failed"
        ));
    }

    #[test]
    fn event_flags_follow_browser_bubbling_and_cancelability() {
        assert_eq!(event_flags("focus"), (false, false));
        assert_eq!(event_flags("click"), (true, true));
        assert_eq!(event_flags("input"), (true, false));
    }
}
