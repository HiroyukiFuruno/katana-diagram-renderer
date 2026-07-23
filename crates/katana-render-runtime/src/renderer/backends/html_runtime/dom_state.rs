use super::super::html_document::HtmlDocument;
use super::types::DomValue;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[path = "dom_state_mutation.rs"]
mod mutation;

pub(super) struct HtmlDomBridgeState {
    pub(super) document: RefCell<HtmlDocument>,
    pub(super) error: RefCell<Option<String>>,
    event_targets: RefCell<HashMap<String, HashSet<u64>>>,
}

impl HtmlDomBridgeState {
    pub(super) fn new(document: HtmlDocument) -> Self {
        Self {
            document: RefCell::new(document),
            error: RefCell::new(None),
            event_targets: RefCell::new(HashMap::new()),
        }
    }

    pub(super) fn dispatch(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        match operation {
            "getElementById" | "querySelector" | "querySelectorAll" | "createElement"
            | "textContent" | "innerHTML" | "getAttribute" | "eventPath" | "closest" => {
                self.lookup(operation, arguments)
            }
            "appendChild" | "remove" => self.mutate_tree(operation, arguments),
            "setTextContent" | "setInnerHTML" => self.mutate_content(operation, arguments),
            "setAttribute" | "removeAttribute" => self.set_attribute(operation, arguments),
            "styleGet" | "styleSet" => self.style(operation, arguments),
            "setEventTarget" => self.set_event_target(arguments),
            _ => Err(format!("unsupported DOM operation: {operation}")),
        }
    }

    pub(super) fn event_target_ids(&self, event_type: &str) -> HashSet<u64> {
        self.event_targets
            .borrow()
            .get(event_type)
            .cloned()
            .unwrap_or_else(HashSet::new)
    }

    fn set_event_target(&self, arguments: &[String]) -> Result<DomValue, String> {
        let node_id = node_id(argument(arguments, 0)?)?;
        self.document.borrow().node(node_id)?;
        let event_type = argument(arguments, 1)?.to_string();
        let enabled = argument(arguments, 2)? == "true";
        let mut targets = self.event_targets.borrow_mut();
        let nodes = targets.entry(event_type.clone()).or_default();
        if enabled {
            nodes.insert(node_id);
        } else {
            nodes.remove(&node_id);
            if nodes.is_empty() {
                targets.remove(&event_type);
            }
        }
        Ok(DomValue::Undefined)
    }

    fn lookup(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        match operation {
            "getElementById" | "querySelector" | "querySelectorAll" | "createElement" => {
                self.lookup_node(operation, arguments)
            }
            _ => self.lookup_content(operation, arguments),
        }
    }

    fn lookup_node(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        match operation {
            "getElementById" => Ok(document
                .get_element_by_id(argument(arguments, 0)?)
                .map(DomValue::NodeId)
                .unwrap_or(DomValue::Null)),
            "querySelector" => Ok(document
                .query_selector(argument(arguments, 0)?)
                .map(DomValue::NodeId)
                .unwrap_or(DomValue::Null)),
            "querySelectorAll" => Ok(DomValue::NodeIds(
                document.query_selector_all(argument(arguments, 0)?),
            )),
            "createElement" => document
                .create_element(argument(arguments, 0)?)
                .map(DomValue::NodeId),
            _ => Err(format!(
                "unsupported HTML node lookup operation: {operation}"
            )),
        }
    }

    fn lookup_content(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        let document = self.document.borrow();
        match operation {
            "textContent" => document
                .text_content(node_id(argument(arguments, 0)?)?)
                .map(DomValue::String),
            "innerHTML" => document
                .inner_html(node_id(argument(arguments, 0)?)?)
                .map(DomValue::String),
            "getAttribute" => Ok(document
                .attribute(node_id(argument(arguments, 0)?)?, argument(arguments, 1)?)?
                .map(DomValue::String)
                .unwrap_or(DomValue::Null)),
            "eventPath" => document
                .event_path(node_id(argument(arguments, 0)?)?)
                .map(DomValue::NodeIds),
            "closest" => document
                .closest_selector(node_id(argument(arguments, 0)?)?, argument(arguments, 1)?)
                .map(|node| node.map(DomValue::NodeId).unwrap_or(DomValue::Null)),
            _ => Err(format!(
                "unsupported HTML content lookup operation: {operation}"
            )),
        }
    }
}

fn argument(arguments: &[String], index: usize) -> Result<&str, String> {
    arguments
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("missing DOM argument {index}"))
}

pub(super) fn node_id(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid HTML node id: {value}"))
}

#[cfg(test)]
mod tests {
    use super::HtmlDomBridgeState;
    use crate::renderer::backends::html_document::HtmlDocument;

    fn state() -> HtmlDomBridgeState {
        HtmlDomBridgeState::new(HtmlDocument::parse("<p id=target>Text</p>"))
    }

    #[test]
    fn unsupported_operation_errors_are_explicit() {
        let state = state();
        assert!(state.dispatch("unsupported", &[]).is_err());
        assert!(state.lookup("unsupported", &[]).is_err());
        assert!(state.mutate_tree("unsupported", &[]).is_err());
        assert!(
            state
                .mutate_content("unsupported", &["1".to_string(), "x".to_string()])
                .is_err()
        );
        assert!(state.style("unsupported", &[]).is_err());
        assert!(
            state
                .set_attribute("unsupported", &["1".to_string(), "state".to_string()])
                .is_err()
        );
    }

    #[test]
    fn unsupported_lookup_node_operation_reports_contract_error() {
        let state = state();
        assert!(matches!(
            state.lookup_node("not_supported", &[]),
            Err(error) if error == "unsupported HTML node lookup operation: not_supported"
        ));
    }
}
