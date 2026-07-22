use super::dom_state::HtmlDomBridgeState;
use super::types::DomValue;
const MISSING_DOM_STATE_ERROR: &str = "HTML DOM state is unavailable";

pub(super) fn dom_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    let operation = args.get(0).to_rust_string_lossy(scope);
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
        DomValue::NodeIds(values) => node_id_array(scope, &values).into(),
    }
}

fn node_id_array<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    values: &[u64],
) -> v8::Local<'scope, v8::Array> {
    let length = i32::try_from(values.len()).unwrap_or(i32::MAX);
    let array = v8::Array::new(scope, length);
    for (index, value) in values.iter().enumerate() {
        let _assigned = v8::String::new(scope, &value.to_string())
            .map(|value| array.set_index(scope, index as u32, value.into()));
    }
    array
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dom_state_error_preserves_contract_message() {
        assert_eq!(MISSING_DOM_STATE_ERROR, "HTML DOM state is unavailable");
    }
}
