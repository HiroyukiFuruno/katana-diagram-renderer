use super::session_interaction::discarded_runtime_error;
use super::{HtmlNodeIds, StaticHtmlRuntimeSession};
use crate::renderer::backends::html_document::HtmlDocumentNode;
use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
use std::collections::HashSet;

impl StaticHtmlRuntimeSession {
    pub(crate) fn snapshot(&self) -> Result<String, HtmlRuntimeError> {
        self.isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .map(|state| state.document.borrow().render())
            .ok_or_else(dom_state_unavailable_error)
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes(
        &self,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        self.interactive_nodes_at_width(1024.0)
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_at_width(
        &self,
        viewport_width: f32,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        self.interactive_nodes_at_width_with_hover(viewport_width, &HashSet::new())
    }

    pub(in crate::renderer::backends) fn interactive_nodes_at_width_with_hover(
        &self,
        viewport_width: f32,
        hovered_nodes: &HtmlNodeIds,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        self.interactive_nodes_with_hover(viewport_width, hovered_nodes)
    }

    fn interactive_nodes_with_hover(
        &self,
        viewport_width: f32,
        hovered_nodes: &HtmlNodeIds,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        let state = self
            .isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        Ok(state
            .document
            .borrow()
            .interactive_nodes_with_styles_at_width_and_hover(
                &self.external_stylesheets,
                viewport_width,
                hovered_nodes,
            ))
    }

    pub(in crate::renderer::backends) fn event_target_ids(
        &self,
        event_type: &str,
    ) -> Result<HashSet<u64>, HtmlRuntimeError> {
        self.isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .map(|state| state.event_target_ids(event_type))
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

    pub(crate) fn toggle_checked(&mut self, node_id: u64) -> Result<bool, HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let state = isolate
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        state
            .document
            .borrow_mut()
            .toggle_boolean_attribute(node_id, "checked")
            .map_err(HtmlRuntimeError::DomBridge)
    }
}

fn dom_state_unavailable_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML DOM state is unavailable".to_string())
}

#[cfg(test)]
mod tests {
    use super::StaticHtmlRuntimeSession;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
    use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
    use std::collections::HashMap;

    #[test]
    fn snapshot_reports_unavailable_dom_state_when_bridge_slot_is_missing() {
        DiagramV8Runtime::ensure_initialized();
        let session = StaticHtmlRuntimeSession {
            context: None,
            isolate: Some(v8::Isolate::new(Default::default())),
            external_stylesheets: HashMap::new(),
        };

        assert!(matches!(
            session.snapshot(),
            Err(HtmlRuntimeError::DomBridge(message))
                if message == "HTML DOM state is unavailable"
        ));
    }
}
