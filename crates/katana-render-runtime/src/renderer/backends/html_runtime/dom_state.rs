use super::super::html_document::HtmlDocument;
use super::style::{kebab_case, property as style_property, set_property as set_style_property};
use super::types::DomValue;
use std::cell::RefCell;
use std::collections::HashMap;

pub(super) struct HtmlDomBridgeState {
    pub(super) document: RefCell<HtmlDocument>,
    pub(super) error: RefCell<Option<String>>,
    click_listeners: RefCell<HashMap<u64, Vec<v8::Global<v8::Function>>>>,
}

impl HtmlDomBridgeState {
    pub(super) fn new(document: HtmlDocument) -> Self {
        Self {
            document: RefCell::new(document),
            error: RefCell::new(None),
            click_listeners: RefCell::new(HashMap::new()),
        }
    }

    pub(super) fn dispatch(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        match operation {
            "getElementById" | "querySelector" | "createElement" | "textContent" | "innerHTML"
            | "getAttribute" => self.lookup(operation, arguments),
            "appendChild" | "remove" => self.mutate_tree(operation, arguments),
            "setTextContent" | "setInnerHTML" => self.mutate_content(operation, arguments),
            "setAttribute" => self.set_attribute(arguments),
            "styleGet" | "styleSet" => self.style(operation, arguments),
            _ => Err(format!("unsupported DOM operation: {operation}")),
        }
    }

    fn lookup(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
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
            "createElement" => document
                .create_element(argument(arguments, 0)?)
                .map(DomValue::NodeId),
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
            _ => Err(format!("unsupported HTML lookup operation: {operation}")),
        }
    }

    fn mutate_tree(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        match operation {
            "appendChild" => {
                let parent = node_id(argument(arguments, 0)?)?;
                let child = node_id(argument(arguments, 1)?)?;
                document.append_child(parent, child)?;
                Ok(DomValue::Undefined)
            }
            "remove" => {
                document.remove(node_id(argument(arguments, 0)?)?)?;
                Ok(DomValue::Undefined)
            }
            _ => Err(format!("unsupported HTML mutation operation: {operation}")),
        }
    }

    fn mutate_content(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        let node_id = node_id(argument(arguments, 0)?)?;
        let value = argument(arguments, 1)?;
        match operation {
            "setTextContent" => document.set_text_content(node_id, value)?,
            "setInnerHTML" => document.set_inner_html(node_id, value)?,
            _ => return Err(format!("unsupported HTML content operation: {operation}")),
        }
        Ok(DomValue::Undefined)
    }

    fn set_attribute(&self, arguments: &[String]) -> Result<DomValue, String> {
        self.document.borrow_mut().set_attribute(
            node_id(argument(arguments, 0)?)?,
            argument(arguments, 1)?,
            argument(arguments, 2)?,
        )?;
        Ok(DomValue::Undefined)
    }

    fn style(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        match operation {
            "styleGet" => {
                let node_id = node_id(argument(arguments, 0)?)?;
                let property = argument(arguments, 1)?;
                Ok(document
                    .attribute(node_id, "style")?
                    .and_then(|style| style_property(&style, property))
                    .map(DomValue::String)
                    .unwrap_or(DomValue::Null))
            }
            "styleSet" => {
                let node_id = node_id(argument(arguments, 0)?)?;
                let property = kebab_case(argument(arguments, 1)?);
                let value = argument(arguments, 2)?;
                let current = document
                    .attribute(node_id, "style")?
                    .unwrap_or_else(String::new);
                let style = set_style_property(&current, &property, value);
                document.set_attribute(node_id, "style", &style)?;
                Ok(DomValue::Undefined)
            }
            _ => Err(format!("unsupported HTML style operation: {operation}")),
        }
    }

    pub(super) fn add_click_listener(&self, node_id: u64, listener: v8::Global<v8::Function>) {
        self.click_listeners
            .borrow_mut()
            .entry(node_id)
            .or_default()
            .push(listener);
    }

    #[cfg(test)]
    pub(super) fn click_listeners(&self, node_id: u64) -> Vec<v8::Global<v8::Function>> {
        match self.click_listeners.borrow().get(&node_id) {
            Some(listeners) => listeners.clone(),
            None => Vec::new(),
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
    }
}
