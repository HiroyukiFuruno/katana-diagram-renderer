use super::super::dom_state::HtmlDomBridgeState;
use super::super::execution::ExecutionBudget;
use super::super::interaction::{
    element_reference, event, event_default_prevented, run_inline_handler,
};
use super::super::script::{
    HtmlTryCatchScope, check_bridge_error, dom_state_unavailable_error, exception_message,
    perform_microtask_checkpoint,
};
#[cfg(test)]
use super::super::types::HtmlNodeId;
use super::super::types::{
    HtmlNavigationIntent, HtmlRuntimeDispatch, HtmlRuntimeError, HtmlRuntimeEvent,
    HtmlRuntimeEventKind,
};
use super::{StaticHtmlRuntimeSession, discarded_runtime_error};

impl StaticHtmlRuntimeSession {
    #[cfg(test)]
    pub(crate) fn node_for_element_id(&mut self, id: &str) -> Option<HtmlNodeId> {
        self.isolate
            .as_mut()?
            .get_slot::<HtmlDomBridgeState>()?
            .document
            .borrow_mut()
            .get_element_by_id(id)
            .map(HtmlNodeId)
    }

    pub(crate) fn dispatch(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let result = self.run_event(event);
        if matches!(result, Err(HtmlRuntimeError::ExecutionTimeout)) {
            self.discard();
        }
        result
    }

    fn run_event(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let kind = event.kind();
        let target = event.target();
        let key = event.key();
        let navigation = {
            let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
            let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
            v8::scope!(let handle_scope, isolate);
            let context = v8::Local::new(handle_scope, context);
            let context_scope = &mut v8::ContextScope::new(handle_scope, context);
            v8::tc_scope!(let scope, &mut **context_scope);
            dispatch_event(scope, target.0, kind, key)?
        };
        Ok(HtmlRuntimeDispatch {
            content: self.snapshot()?,
            navigation,
        })
    }

    fn discard(&mut self) {
        self.context.take();
        self.isolate.take();
    }
}

fn dispatch_event(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    kind: HtmlRuntimeEventKind,
    key: Option<&str>,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    let target = element_reference(scope, node_id)?;
    let event = event(scope, target, kind.as_str(), key)?;
    dispatch_registered_listeners(scope, node_id, kind, target, event)?;
    dispatch_inline_handler(scope, node_id, kind, target, event)?;
    if kind == HtmlRuntimeEventKind::Click {
        navigation_intent(scope, node_id, event)
    } else {
        Ok(None)
    }
}

fn dispatch_registered_listeners(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    kind: HtmlRuntimeEventKind,
    target: v8::Local<v8::Object>,
    event: v8::Local<v8::Object>,
) -> Result<(), HtmlRuntimeError> {
    let listeners = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or_else(dom_state_unavailable_error)?
        .listeners(node_id, kind);
    for listener in listeners {
        let listener = v8::Local::new(scope, listener);
        let budget = ExecutionBudget::start(scope);
        let result = listener
            .call(scope, target.into(), &[event.into()])
            .ok_or_else(|| HtmlRuntimeError::JavaScriptException(exception_message(scope)));
        budget.finish()?;
        result?;
        perform_microtask_checkpoint(scope)?;
        check_bridge_error(scope)?;
    }
    Ok(())
}

fn dispatch_inline_handler(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    kind: HtmlRuntimeEventKind,
    target: v8::Local<v8::Object>,
    event: v8::Local<v8::Object>,
) -> Result<(), HtmlRuntimeError> {
    let attribute = format!("on{}", kind.as_str());
    let handler = {
        let state = scope
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        state
            .document
            .borrow()
            .attribute(node_id, &attribute)
            .map_err(HtmlRuntimeError::DomBridge)?
    };
    if let Some(handler) = handler {
        run_inline_handler(scope, &handler, target, event)?;
        check_bridge_error(scope)?;
    }
    Ok(())
}

fn navigation_intent(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    event: v8::Local<v8::Object>,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    if event_default_prevented(scope, event)? {
        return Ok(None);
    }
    scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or_else(dom_state_unavailable_error)?
        .document
        .borrow()
        .attribute(node_id, "href")
        .map_err(HtmlRuntimeError::DomBridge)
        .map(|href| href.map(|href| HtmlNavigationIntent { href }))
}
