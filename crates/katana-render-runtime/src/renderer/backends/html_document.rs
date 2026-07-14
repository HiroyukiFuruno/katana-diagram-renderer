use super::html_dom_helpers::{
    attribute_value, collect_scripts, detach, find_element, selector_matches, text_content,
};
use super::html_snapshot::render_document;
use html5ever::{Attribute, QualName, parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, Node, NodeData, RcDom};
use std::collections::HashMap;
use std::rc::Rc;

/// Canonical HTML5 document state shared by CSS rendering and the V8 bridge.
pub(super) struct HtmlDocument {
    document: Handle,
    nodes: HashMap<u64, Handle>,
    node_ids: HashMap<usize, u64>,
    next_node_id: u64,
}

impl HtmlDocument {
    pub(super) fn parse(source: &str) -> Self {
        let parsed = parse_document(RcDom::default(), Default::default()).one(source.to_string());
        let mut document = Self {
            document: parsed.document,
            nodes: HashMap::new(),
            node_ids: HashMap::new(),
            next_node_id: 1,
        };
        document.register_subtree(&document.document.clone());
        document
    }

    pub(super) fn render(&self) -> String {
        render_document(&self.document)
    }

    pub(super) fn inline_scripts(&self) -> Result<Vec<String>, String> {
        let mut scripts = Vec::new();
        collect_scripts(&self.document, &mut scripts)?;
        Ok(scripts)
    }

    pub(super) fn get_element_by_id(&mut self, id: &str) -> Option<u64> {
        let handle = find_element(&self.document, |tag, attributes| {
            tag == "*" || attribute_value(attributes, "id") == Some(id)
        })?;
        Some(self.register_subtree(&handle))
    }

    pub(super) fn query_selector(&mut self, selector: &str) -> Option<u64> {
        let selector = selector.trim();
        if selector.is_empty() {
            return None;
        }
        let handle = find_element(&self.document, |tag, attributes| {
            selector_matches(selector, tag, attributes)
        })?;
        Some(self.register_subtree(&handle))
    }

    pub(super) fn create_element(&mut self, tag: &str) -> Result<u64, String> {
        let tag = tag.trim().to_ascii_lowercase();
        if tag.is_empty()
            || !tag
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(format!("unsupported element name: {tag}"));
        }
        let node = Node::new(NodeData::Element {
            name: QualName::new(None, Default::default(), tag.into()),
            attrs: Default::default(),
            template_contents: Default::default(),
            mathml_annotation_xml_integration_point: false,
        });
        Ok(self.register_subtree(&node))
    }

    pub(super) fn append_child(&mut self, parent_id: u64, child_id: u64) -> Result<(), String> {
        let parent = self.node(parent_id)?;
        let child = self.node(child_id)?;
        if !matches!(parent.data, NodeData::Document | NodeData::Element { .. }) {
            return Err("appendChild target is not a container".to_string());
        }
        detach(&child);
        child.parent.set(Some(Rc::downgrade(&parent)));
        parent.children.borrow_mut().push(child);
        Ok(())
    }

    pub(super) fn remove(&mut self, node_id: u64) -> Result<(), String> {
        let node = self.node(node_id)?;
        detach(&node);
        Ok(())
    }

    pub(super) fn text_content(&self, node_id: u64) -> Result<String, String> {
        let node = self.node(node_id)?;
        Ok(text_content(&node))
    }

    pub(super) fn set_text_content(&mut self, node_id: u64, value: &str) -> Result<(), String> {
        let node = self.node(node_id)?;
        let children = std::mem::take(&mut *node.children.borrow_mut());
        for child in children {
            child.parent.set(None);
        }
        let text = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(value.into()),
        });
        text.parent.set(Some(Rc::downgrade(&node)));
        node.children.borrow_mut().push(text.clone());
        self.register_subtree(&text);
        Ok(())
    }

    pub(super) fn attribute(&self, node_id: u64, name: &str) -> Result<Option<String>, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        Ok(attribute_value(&attrs.borrow(), name).map(ToOwned::to_owned))
    }

    pub(super) fn set_attribute(
        &mut self,
        node_id: u64,
        name: &str,
        value: &str,
    ) -> Result<(), String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("attribute name is empty".to_string());
        }
        let mut attrs = attrs.borrow_mut();
        if let Some(attribute) = attrs
            .iter_mut()
            .find(|attribute| attribute.name.local.as_ref() == name)
        {
            attribute.value = value.into();
        } else {
            attrs.push(Attribute {
                name: QualName::new(None, Default::default(), name.into()),
                value: value.into(),
            });
        }
        Ok(())
    }

    pub(super) fn node(&self, node_id: u64) -> Result<Handle, String> {
        self.nodes
            .get(&node_id)
            .cloned()
            .ok_or_else(|| format!("HTML node {node_id} does not exist"))
    }

    pub(super) fn register_subtree(&mut self, node: &Handle) -> u64 {
        let pointer = Rc::as_ptr(node) as usize;
        let node_id = match self.node_ids.get(&pointer) {
            Some(node_id) => *node_id,
            None => {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                self.node_ids.insert(pointer, node_id);
                self.nodes.insert(node_id, node.clone());
                node_id
            }
        };
        let children = node.children.borrow().clone();
        for child in children {
            self.register_subtree(&child);
        }
        node_id
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;

    #[test]
    fn rejects_container_and_attribute_operations_on_text_nodes() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = must_some(
            document.get_element_by_id("target"),
            "target element must exist",
        );
        must(document.set_text_content(target, "replacement"));
        let text_node = document.next_node_id - 1;

        let append = document.append_child(text_node, target);
        let attribute = document.attribute(text_node, "class");
        let set_attribute = document.set_attribute(text_node, "class", "note");

        assert!(matches!(append, Err(error) if error.contains("not a container")));
        assert!(matches!(attribute, Err(error) if error.contains("not an element")));
        assert!(matches!(set_attribute, Err(error) if error.contains("not an element")));
    }

    #[test]
    fn rejects_external_scripts_and_missing_selectors() {
        let mut document =
            HtmlDocument::parse("<script src=\"app.js\"></script><p class=visible>Visible</p>");

        assert!(matches!(
            document.inline_scripts(),
            Err(error) if error.contains("external script is not supported: app.js")
        ));
        assert_eq!(document.query_selector(".missing"), None);
        assert_eq!(document.get_element_by_id("missing"), None);
    }

    #[test]
    fn rejects_operations_on_missing_nodes() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = must_some(
            document.get_element_by_id("target"),
            "target element must exist",
        );

        assert_missing_node(document.append_child(999, target));
        assert_missing_node(document.append_child(target, 999));
        assert_missing_node(document.remove(999));
        assert_missing_node(document.text_content(999));
        assert_missing_node(document.set_text_content(999, "replacement"));
        assert_missing_node(document.set_attribute(999, "class", "note"));
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_reports_unexpected_test_errors() {
        let _: () = must(Err("boom".to_string()));
    }

    #[test]
    #[should_panic(expected = "target missing")]
    fn must_some_reports_missing_test_values() {
        let _: () = must_some(None, "target missing");
    }

    #[test]
    fn must_some_error_branch_covers_node_ids() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: u64 = must_some(None, "target missing");
            })
            .is_err()
        );
    }

    fn must<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn must_some<T>(value: Option<T>, message: &str) -> T {
        match value {
            Some(value) => value,
            None => fail(message.to_string()),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    fn assert_missing_node<T>(result: Result<T, String>) {
        assert_eq!(
            result.err(),
            Some("HTML node 999 does not exist".to_string())
        );
    }
}
