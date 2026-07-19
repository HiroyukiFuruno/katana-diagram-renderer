use super::dom_state::{HtmlDomBridgeState, node_id};
use super::types::{DomValue, HtmlRuntimeEventKind};

const LISTENER_NODE_ID_INDEX: i32 = 1;
const LISTENER_EVENT_TYPE_INDEX: i32 = 2;
const LISTENER_FUNCTION_INDEX: i32 = 3;
const MISSING_DOM_STATE_ERROR: &str = "HTML DOM state is unavailable";

pub(super) fn dom_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let operation = args.get(0).to_rust_string_lossy(scope);
    if operation == "addEventListener" {
        match register_listener(scope, &args) {
            Ok(()) => return_value.set(v8::undefined(scope).into()),
            Err(error) => set_bridge_error(scope, &mut return_value, error),
        }
        return;
    }

    let arguments = (1..args.length())
        .map(|index| args.get(index).to_rust_string_lossy(scope))
        .collect::<Vec<_>>();
    let result = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or(MISSING_DOM_STATE_ERROR.to_string())
        .and_then(|state| state.dispatch(&operation, &arguments));
    match result {
        Ok(value) => return_value.set(dom_value(scope, value)),
        Err(error) => set_bridge_error(scope, &mut return_value, error),
    }
}

fn register_listener(
    scope: &mut v8::PinScope,
    args: &v8::FunctionCallbackArguments,
) -> Result<(), String> {
    let node_id = node_id(&args.get(LISTENER_NODE_ID_INDEX).to_rust_string_lossy(scope))?;
    let event_type = args
        .get(LISTENER_EVENT_TYPE_INDEX)
        .to_rust_string_lossy(scope);
    let event = HtmlRuntimeEventKind::parse(&event_type)
        .ok_or_else(|| format!("unsupported event type: {event_type}"))?;
    let Ok(listener) = v8::Local::<v8::Function>::try_from(args.get(LISTENER_FUNCTION_INDEX))
    else {
        return Err("event listener must be a function".to_string());
    };
    let listener = v8::Global::new(scope, listener);
    let state = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or(MISSING_DOM_STATE_ERROR.to_string())?;
    state.add_listener(node_id, event, listener);
    Ok(())
}

fn set_bridge_error(
    scope: &mut v8::PinScope,
    return_value: &mut v8::ReturnValue<v8::Value>,
    error: String,
) {
    if let Some(state) = scope.get_slot::<HtmlDomBridgeState>() {
        let _previous_error = state.error.replace(Some(error));
    }
    return_value.set(v8::undefined(scope).into());
}

fn dom_value<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    value: DomValue,
) -> v8::Local<'scope, v8::Value> {
    match value {
        DomValue::Undefined => v8::undefined(scope).into(),
        DomValue::Null => v8::null(scope).into(),
        DomValue::String(value) => {
            let undefined = v8::undefined(scope).into();
            v8::String::new(scope, &value).map_or(undefined, |value| value.into())
        }
        DomValue::NodeId(value) => {
            let undefined = v8::undefined(scope).into();
            v8::String::new(scope, &value.to_string()).map_or(undefined, |value| value.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dom_state_error_preserves_contract_message() {
        assert_eq!(MISSING_DOM_STATE_ERROR, "HTML DOM state is unavailable");
    }
}
