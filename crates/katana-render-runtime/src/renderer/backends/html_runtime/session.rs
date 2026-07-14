use super::dom_state::HtmlDomBridgeState;
use super::script::{
    check_bridge_error, dom_state_unavailable_error, evaluate, install_dom_bridge,
    perform_microtask_checkpoint,
};
use super::types::HtmlRuntimeError;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use crate::renderer::backends::html_document::HtmlDocument;

#[derive(Debug, Clone, Default)]
pub(crate) struct StaticHtmlRuntime;

impl StaticHtmlRuntime {
    pub(crate) fn render(&self, source: &str) -> Result<String, HtmlRuntimeError> {
        self.start(source)?.snapshot()
    }

    pub(crate) fn start(&self, source: &str) -> Result<StaticHtmlRuntimeSession, HtmlRuntimeError> {
        let document = HtmlDocument::parse(source);
        let scripts = document
            .inline_scripts()
            .map_err(HtmlRuntimeError::ExternalScript)?;

        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::new(document));

        let context = Self::execute_inline_scripts(&mut isolate, &scripts)?;
        #[cfg(not(test))]
        drop(context);
        Ok(StaticHtmlRuntimeSession {
            #[cfg(test)]
            context: Some(context),
            isolate: Some(isolate),
        })
    }

    fn execute_inline_scripts(
        isolate: &mut v8::OwnedIsolate,
        scripts: &[String],
    ) -> Result<v8::Global<v8::Context>, HtmlRuntimeError> {
        v8::scope!(let handle_scope, isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        install_dom_bridge(scope)?;
        for script in scripts {
            evaluate(scope, "inline-script", script)?;
            perform_microtask_checkpoint(scope)?;
            check_bridge_error(scope)?;
        }
        Ok(v8::Global::new(scope, context))
    }
}

pub(crate) struct StaticHtmlRuntimeSession {
    #[cfg(test)]
    context: Option<v8::Global<v8::Context>>,
    isolate: Option<v8::OwnedIsolate>,
}

impl StaticHtmlRuntimeSession {
    pub(crate) fn snapshot(&self) -> Result<String, HtmlRuntimeError> {
        self.isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .map(|state| state.document.borrow().render())
            .ok_or_else(dom_state_unavailable_error)
    }
}

fn discarded_runtime_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML runtime session was discarded after timeout".to_string())
}

#[cfg(test)]
#[path = "session_interaction.rs"]
mod session_interaction;
