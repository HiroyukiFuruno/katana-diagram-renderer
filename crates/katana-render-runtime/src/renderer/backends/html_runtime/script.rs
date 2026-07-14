use super::bridge::dom_callback;
use super::dom_state::HtmlDomBridgeState;
use super::execution::ExecutionBudget;
use super::types::HtmlRuntimeError;

pub(super) const DOM_BOOTSTRAP: &str = r#"
const __krrNativeDom = globalThis.__krr_dom;
const __krrElement = (nodeId) => {
  if (nodeId === null || nodeId === undefined || nodeId === '') return null;
  const element = Object.create(__krrElementPrototype);
  Object.defineProperty(element, '__krrNodeId', { value: String(nodeId) });
  return element;
};
const __krrElementPrototype = {
  get textContent() { return __krrNativeDom('textContent', this.__krrNodeId); },
  set textContent(value) { __krrNativeDom('setTextContent', this.__krrNodeId, String(value)); },
  get innerHTML() { return __krrNativeDom('innerHTML', this.__krrNodeId); },
  set innerHTML(value) { __krrNativeDom('setInnerHTML', this.__krrNodeId, String(value)); },
  get className() { return __krrNativeDom('getAttribute', this.__krrNodeId, 'class') || ''; },
  set className(value) { __krrNativeDom('setAttribute', this.__krrNodeId, 'class', String(value)); },
  get id() { return __krrNativeDom('getAttribute', this.__krrNodeId, 'id') || ''; },
  set id(value) { __krrNativeDom('setAttribute', this.__krrNodeId, 'id', String(value)); },
  get style() {
    const nodeId = this.__krrNodeId;
    return new Proxy({}, {
      get(_target, property) { return __krrNativeDom('styleGet', nodeId, String(property)) || ''; },
      set(_target, property, value) { __krrNativeDom('styleSet', nodeId, String(property), String(value)); return true; },
    });
  },
  getAttribute(name) { return __krrNativeDom('getAttribute', this.__krrNodeId, String(name)); },
  setAttribute(name, value) { __krrNativeDom('setAttribute', this.__krrNodeId, String(name), String(value)); },
  appendChild(child) { __krrNativeDom('appendChild', this.__krrNodeId, child.__krrNodeId); return child; },
  remove() { __krrNativeDom('remove', this.__krrNodeId); },
  addEventListener(type, listener) {
    if (type !== 'click' || typeof listener !== 'function') throw new TypeError('Only click event listeners are supported');
    __krrNativeDom('addEventListener', this.__krrNodeId, type, listener);
  },
};
globalThis.document = {
  getElementById(id) { return __krrElement(__krrNativeDom('getElementById', String(id))); },
  querySelector(selector) { return __krrElement(__krrNativeDom('querySelector', String(selector))); },
  createElement(tag) { return __krrElement(__krrNativeDom('createElement', String(tag))); },
};
globalThis.window = globalThis;
"#;

pub(super) type HtmlTryCatchScope<'pin, 'scope, 'object, 'isolate> =
    v8::PinnedRef<'pin, v8::TryCatch<'scope, 'object, v8::HandleScope<'isolate>>>;

pub(super) fn install_dom_bridge(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(), HtmlRuntimeError> {
    let context = scope.get_current_context();
    let global = context.global(scope);
    let name_error = HtmlRuntimeError::DomBridge("DOM function name allocation failed".to_string());
    let callback_error = HtmlRuntimeError::DomBridge("DOM callback allocation failed".to_string());
    let registration_error =
        HtmlRuntimeError::DomBridge("DOM callback registration failed".to_string());
    let name = v8::String::new(scope, "__krr_dom").ok_or(name_error)?;
    let callback = v8::Function::new(scope, dom_callback).ok_or(callback_error)?;
    global
        .set(scope, name.into(), callback.into())
        .ok_or(registration_error)?;
    evaluate(scope, "krr-html-dom-bootstrap", DOM_BOOTSTRAP)
}

pub(super) fn evaluate(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<(), HtmlRuntimeError> {
    evaluate_value(scope, name, code).map(|_| ())
}

pub(super) fn evaluate_value<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<v8::Local<'scope, v8::Value>, HtmlRuntimeError> {
    let budget = ExecutionBudget::start(scope);
    let result = evaluate_value_unbounded(scope, name, code);
    budget.finish()?;
    result
}

fn evaluate_value_unbounded<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<v8::Local<'scope, v8::Value>, HtmlRuntimeError> {
    let source = v8::String::new(scope, code).ok_or_else(source_allocation_error)?;
    let origin_name = v8::String::new(scope, name).ok_or_else(filename_allocation_error)?;
    let origin = v8::ScriptOrigin::new(
        scope,
        origin_name.into(),
        0,
        0,
        false,
        0,
        Some(origin_name.into()),
        false,
        false,
        false,
        None,
    );
    let script = v8::Script::compile(scope, source, Some(&origin))
        .ok_or_else(|| HtmlRuntimeError::JavaScriptException(exception_message(scope)))?;
    script
        .run(scope)
        .ok_or_else(|| HtmlRuntimeError::JavaScriptException(exception_message(scope)))
}

pub(super) fn perform_microtask_checkpoint(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(), HtmlRuntimeError> {
    let budget = ExecutionBudget::start(scope);
    scope.as_mut().perform_microtask_checkpoint();
    budget.finish()
}

pub(super) fn check_bridge_error(
    scope: &HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(), HtmlRuntimeError> {
    let state = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or_else(dom_state_unavailable_error)?;
    match state.error.borrow_mut().take() {
        Some(error) => Err(HtmlRuntimeError::DomBridge(error)),
        None => Ok(()),
    }
}

pub(super) fn dom_state_unavailable_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML DOM state is unavailable".to_string())
}

fn source_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("source allocation failed".to_string())
}

fn filename_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("filename allocation failed".to_string())
}

pub(super) fn exception_message(scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>) -> String {
    scope
        .exception()
        .map(|exception| exception.to_rust_string_lossy(scope))
        .unwrap_or_else(unknown_v8_exception_message)
}

fn unknown_v8_exception_message() -> String {
    "unknown V8 exception".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_error_helpers_preserve_contract_messages() {
        assert!(matches!(
            source_allocation_error(),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "source allocation failed"
        ));
        assert!(matches!(
            filename_allocation_error(),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "filename allocation failed"
        ));
    }

    #[test]
    fn dom_state_error_helper_preserves_contract_message() {
        assert!(matches!(
            dom_state_unavailable_error(),
            HtmlRuntimeError::DomBridge(message) if message == "HTML DOM state is unavailable"
        ));
    }

    #[test]
    fn unknown_v8_exception_message_is_stable() {
        assert_eq!(unknown_v8_exception_message(), "unknown V8 exception");
    }
}
