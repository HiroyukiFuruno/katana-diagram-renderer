use super::dom_state::HtmlDomBridgeState;
use super::script::{
    check_bridge_error, dom_state_unavailable_error, evaluate, install_dom_bridge,
    perform_microtask_checkpoint,
};
use super::types::HtmlRuntimeError;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use crate::renderer::backends::html_browser::HtmlBrowserSource;
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;
use std::collections::HashMap;

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
        Ok(StaticHtmlRuntimeSession {
            context: Some(context),
            isolate: Some(isolate),
            external_stylesheets: HashMap::new(),
        })
    }

    pub(in crate::renderer::backends) fn start_interactive(
        &self,
        source: &HtmlBrowserSource,
    ) -> Result<StaticHtmlRuntimeSession, HtmlRuntimeError> {
        let mut document = HtmlDocument::parse(&source.raw_html);
        let resources = HtmlSubresourceLoader::new(source)
            .load(&mut document)
            .map_err(HtmlRuntimeError::Subresource)?;

        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::new(document));
        let context = Self::execute_inline_scripts(&mut isolate, &resources.scripts)?;
        Ok(StaticHtmlRuntimeSession {
            context: Some(context),
            isolate: Some(isolate),
            external_stylesheets: resources.stylesheets,
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
    context: Option<v8::Global<v8::Context>>,
    isolate: Option<v8::OwnedIsolate>,
    external_stylesheets: HashMap<String, String>,
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

    pub(in crate::renderer::backends) fn interactive_nodes(
        &self,
    ) -> Result<Vec<crate::renderer::backends::html_document::HtmlDocumentNode>, HtmlRuntimeError>
    {
        self.isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .map(|state| {
                state
                    .document
                    .borrow()
                    .interactive_nodes_with_styles(&self.external_stylesheets)
            })
            .ok_or_else(dom_state_unavailable_error)
    }

    pub(crate) fn set_value(&mut self, node_id: u64, value: &str) -> Result<(), HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let state = isolate
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        state
            .document
            .borrow_mut()
            .set_attribute(node_id, "value", value)
            .map_err(HtmlRuntimeError::DomBridge)
    }

    pub(crate) fn toggle_open(&mut self, node_id: u64) -> Result<bool, HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let state = isolate
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        state
            .document
            .borrow_mut()
            .toggle_boolean_attribute(node_id, "open")
            .map_err(HtmlRuntimeError::DomBridge)
    }
}

fn discarded_runtime_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML runtime session was discarded after timeout".to_string())
}

#[path = "session_interaction.rs"]
mod session_interaction;
