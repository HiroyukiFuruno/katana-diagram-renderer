use super::super::dom_state::HtmlDomBridgeState;
use super::super::execution::ExecutionBudget;
use super::super::interaction::{
    click_event, element_reference, event_default_prevented, run_inline_click_handler,
};
use super::super::script::{
    HtmlTryCatchScope, check_bridge_error, dom_state_unavailable_error, exception_message,
    perform_microtask_checkpoint,
};
use super::super::types::{
    HtmlNavigationIntent, HtmlNodeId, HtmlRuntimeDispatch, HtmlRuntimeError, HtmlRuntimeEvent,
};
use super::{StaticHtmlRuntimeSession, discarded_runtime_error};

impl StaticHtmlRuntimeSession {
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
        match event {
            HtmlRuntimeEvent::Click { target } => self.click(target),
        }
    }

    fn click(&mut self, node_id: HtmlNodeId) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let result = self.run_click(node_id);
        if matches!(result, Err(HtmlRuntimeError::ExecutionTimeout)) {
            self.discard();
        }
        result
    }

    fn run_click(&mut self, node_id: HtmlNodeId) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let navigation = {
            let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
            let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
            v8::scope!(let handle_scope, isolate);
            let context = v8::Local::new(handle_scope, context);
            let context_scope = &mut v8::ContextScope::new(handle_scope, context);
            v8::tc_scope!(let scope, &mut **context_scope);
            dispatch_click(scope, node_id.0)?
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

fn dispatch_click(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    let target = element_reference(scope, node_id)?;
    let event = click_event(scope, target)?;
    dispatch_registered_listeners(scope, node_id, target, event)?;
    dispatch_inline_handler(scope, node_id, target, event)?;
    navigation_intent(scope, node_id, event)
}

fn dispatch_registered_listeners(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    target: v8::Local<v8::Object>,
    event: v8::Local<v8::Object>,
) -> Result<(), HtmlRuntimeError> {
    let listeners = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or_else(dom_state_unavailable_error)?
        .click_listeners(node_id);
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
    target: v8::Local<v8::Object>,
    event: v8::Local<v8::Object>,
) -> Result<(), HtmlRuntimeError> {
    let handler = {
        let state = scope
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        state
            .document
            .borrow()
            .attribute(node_id, "onclick")
            .map_err(HtmlRuntimeError::DomBridge)?
    };
    if let Some(handler) = handler {
        run_inline_click_handler(scope, &handler, target, event)?;
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
