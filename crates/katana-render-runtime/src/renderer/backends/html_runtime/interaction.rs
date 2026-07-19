use super::execution::ExecutionBudget;
use super::script::{
    HtmlTryCatchScope, evaluate_value, exception_message, perform_microtask_checkpoint,
};
use super::types::HtmlRuntimeError;

const EVENT_SOURCE: &str =
    "({ defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } })";

pub(super) fn element_reference<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    node_id: u64,
) -> Result<v8::Local<'scope, v8::Object>, HtmlRuntimeError> {
    let source = format!("__krrElement('{node_id}')");
    let value = evaluate_value(scope, "krr-html-dom-element-reference", &source)?;
    let element = v8::Local::<v8::Object>::try_from(value).map_err(element_reference_error)?;
    Ok(element)
}

pub(super) fn event<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    target: v8::Local<'scope, v8::Object>,
    event_type: &str,
) -> Result<v8::Local<'scope, v8::Object>, HtmlRuntimeError> {
    let value = evaluate_value(scope, "krr-html-dom-event", EVENT_SOURCE)?;
    let event = v8::Local::<v8::Object>::try_from(value).map_err(event_error)?;
    let key = v8::String::new(scope, "target").ok_or_else(event_key_error)?;
    event
        .set(scope, key.into(), target.into())
        .ok_or_else(event_assignment_error)?;
    let key = v8::String::new(scope, "currentTarget").ok_or_else(event_key_error)?;
    event
        .set(scope, key.into(), target.into())
        .ok_or_else(event_assignment_error)?;
    let key = v8::String::new(scope, "type").ok_or_else(event_key_error)?;
    let value = v8::String::new(scope, event_type).ok_or_else(event_type_error)?;
    event
        .set(scope, key.into(), value.into())
        .ok_or_else(event_assignment_error)?;
    Ok(event)
}

pub(super) fn run_inline_handler(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    handler: &str,
    target: v8::Local<'_, v8::Object>,
    event: v8::Local<'_, v8::Object>,
) -> Result<(), HtmlRuntimeError> {
    let source = format!("(function(event) {{\n{handler}\n}})");
    let value = evaluate_value(scope, "krr-html-inline-handler", &source)?;
    let function = v8::Local::<v8::Function>::try_from(value).map_err(inline_handler_type_error)?;
    let budget = ExecutionBudget::start(scope);
    let result = function
        .call(scope, target.into(), &[event.into()])
        .ok_or_else(|| HtmlRuntimeError::JavaScriptException(exception_message(scope)));
    budget.finish()?;
    result?;
    perform_microtask_checkpoint(scope)?;
    Ok(())
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

fn element_reference_error<T>(_error: T) -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML node reference was not an object".to_string())
}

fn event_error<T>(_error: T) -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML event allocation failed".to_string())
}

fn event_key_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML event key allocation failed".to_string())
}

fn event_assignment_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML event assignment failed".to_string())
}

fn event_type_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML event type allocation failed".to_string())
}

fn inline_handler_type_error<T>(_error: T) -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("inline onclick handler was not a function".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_error_helpers_preserve_contract_messages() {
        assert!(matches!(
            element_reference_error(()),
            HtmlRuntimeError::DomBridge(message)
                if message == "HTML node reference was not an object"
        ));
        assert!(matches!(
            event_error(()),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event allocation failed"
        ));
        assert!(matches!(
            inline_handler_type_error(()),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "inline onclick handler was not a function"
        ));
    }

    #[test]
    fn interaction_v8_data_error_helpers_preserve_contract_messages() {
        let error = v8::DataError::BadType {
            actual: "actual",
            expected: "expected",
        };

        assert!(matches!(
            element_reference_error(error),
            HtmlRuntimeError::DomBridge(message)
                if message == "HTML node reference was not an object"
        ));
        assert!(matches!(
            event_error(error),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event allocation failed"
        ));
        assert!(matches!(
            inline_handler_type_error(error),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "inline onclick handler was not a function"
        ));
    }

    #[test]
    fn event_property_error_helpers_preserve_contract_messages() {
        assert!(matches!(
            event_key_error(),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event key allocation failed"
        ));
        assert!(matches!(
            event_assignment_error(),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event assignment failed"
        ));
        assert!(matches!(
            event_type_error(),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event type allocation failed"
        ));
    }
}
