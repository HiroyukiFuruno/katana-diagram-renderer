use super::html_css::{HtmlAttributes, StaticCss};
use super::html_dom_helpers::{attribute_value, collect_scripts, find_element, selector_matches};
use super::html_snapshot::render_document;
use html5ever::{Attribute, parse_document, tendril::TendrilSink};
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;
use std::rc::Rc;

#[path = "html_document_mutation.rs"]
mod mutation;

/// Canonical HTML5 document state shared by CSS rendering and the V8 bridge.
pub(super) struct HtmlDocument {
    pub(super) document: Handle,
    nodes: HashMap<u64, Handle>,
    node_ids: HashMap<usize, u64>,
    next_node_id: u64,
}

/// Dynamic DOM projection used only by KRR's interactive HTML runtime.
///
/// It deliberately retains node IDs so the runtime can perform hit-testing and
/// dispatch input without exposing DOM details to KDV or KatanA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HtmlDocumentNode {
    Element {
        node_id: u64,
        tag: String,
        attributes: HtmlAttributes,
        children: Vec<HtmlDocumentNode>,
    },
    Text(String),
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

    pub(super) fn interactive_nodes_with_styles(
        &self,
        external_stylesheets: &HashMap<String, String>,
    ) -> Vec<HtmlDocumentNode> {
        let css =
            StaticCss::for_interactive_document_with_styles(&self.document, external_stylesheets);
        self.interactive_children(&self.document, &css)
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

    fn interactive_children(&self, node: &Handle, css: &StaticCss) -> Vec<HtmlDocumentNode> {
        node.children
            .borrow()
            .iter()
            .filter_map(|child| self.interactive_node(child, css))
            .collect()
    }

    fn interactive_node(&self, node: &Handle, css: &StaticCss) -> Option<HtmlDocumentNode> {
        match &node.data {
            NodeData::Text { contents } => {
                let text = contents.borrow().to_string();
                (!text.is_empty()).then_some(HtmlDocumentNode::Text(text))
            }
            NodeData::Element { name, attrs, .. } => {
                let tag = name.local.to_string().to_ascii_lowercase();
                if is_non_rendered_tag(&tag) {
                    return None;
                }
                let attributes = attributes(&attrs.borrow());
                let attributes = css.apply(&tag, &attributes);
                let pointer = Rc::as_ptr(node) as usize;
                let node_id = self.node_ids.get(&pointer).copied()?;
                Some(HtmlDocumentNode::Element {
                    node_id,
                    tag,
                    attributes,
                    children: self.interactive_children(node, css),
                })
            }
            NodeData::Document => None,
            _ => None,
        }
    }
}

fn attributes(source: &[Attribute]) -> HtmlAttributes {
    source
        .iter()
        .map(|attribute| {
            (
                attribute.name.local.to_string().to_ascii_lowercase(),
                attribute.value.to_string(),
            )
        })
        .collect()
}

fn is_non_rendered_tag(tag: &str) -> bool {
    matches!(
        tag,
        "head" | "iframe" | "link" | "meta" | "script" | "style" | "template" | "title"
    )
}

#[cfg(test)]
mod tests {
    use super::{HtmlDocument, StaticCss};
    use std::collections::HashMap;

    #[test]
    fn document_root_is_never_projected_as_an_interactive_node() {
        let document = HtmlDocument::parse("<p>Visible</p>");
        let css =
            StaticCss::for_interactive_document_with_styles(&document.document, &HashMap::new());

        assert_eq!(document.interactive_node(&document.document, &css), None);
    }

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
