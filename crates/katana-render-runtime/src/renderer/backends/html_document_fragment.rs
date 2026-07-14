use super::html_document::HtmlDocument;
use super::html_dom_helpers::find_element;
use super::html_snapshot::render_document;
use html5ever::{driver::parse_fragment_for_element, tendril::TendrilSink};
use markup5ever_rcdom::{Node, NodeData, RcDom};
use std::rc::Rc;

impl HtmlDocument {
    pub(super) fn inner_html(&self, node_id: u64) -> Result<String, String> {
        self.node(node_id).map(|node| render_document(&node))
    }

    pub(super) fn set_inner_html(&mut self, node_id: u64, value: &str) -> Result<(), String> {
        let target = self.node(node_id)?;
        let (name, attributes) = match &target.data {
            NodeData::Element { name, attrs, .. } => (name.clone(), attrs.borrow().clone()),
            _ => return Err("innerHTML target is not an element".to_string()),
        };
        clear_children(&target);
        let context = fragment_context(name, attributes);
        let fragment =
            parse_fragment_for_element(RcDom::default(), Default::default(), context, false, None)
                .one(value.to_string());
        self.attach_fragment_children(&target, &fragment.document);
        Ok(())
    }

    fn attach_fragment_children(
        &mut self,
        target: &markup5ever_rcdom::Handle,
        document: &markup5ever_rcdom::Handle,
    ) {
        let fragment_root = fragment_root(document);
        let children = std::mem::take(&mut *fragment_root.children.borrow_mut());
        for child in children {
            child.parent.set(Some(Rc::downgrade(target)));
            target.children.borrow_mut().push(child.clone());
            self.register_subtree(&child);
        }
    }
}

fn clear_children(target: &markup5ever_rcdom::Handle) {
    let previous = std::mem::take(&mut *target.children.borrow_mut());
    for child in previous {
        child.parent.set(None);
    }
}

fn fragment_context(
    name: html5ever::QualName,
    attributes: Vec<html5ever::Attribute>,
) -> markup5ever_rcdom::Handle {
    Node::new(NodeData::Element {
        name,
        attrs: std::cell::RefCell::new(attributes),
        template_contents: Default::default(),
        mathml_annotation_xml_integration_point: false,
    })
}

fn fragment_root(document: &markup5ever_rcdom::Handle) -> markup5ever_rcdom::Handle {
    find_element(document, |tag, _| tag == "body").unwrap_or_else(|| document.clone())
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;
    use super::*;
    use html5ever::driver::parse_document;
    use html5ever::tendril::TendrilSink;
    use markup5ever_rcdom::RcDom;

    #[test]
    fn rejects_inner_html_on_the_document_root() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");

        let result = document.set_inner_html(1, "<span>replacement</span>");

        assert!(matches!(result, Err(error) if error.contains("not an element")));
    }

    #[test]
    fn rejects_inner_html_on_missing_node() {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");

        assert_eq!(
            document.set_inner_html(999, "<span>replacement</span>"),
            Err("HTML node 999 does not exist".to_string())
        );
    }

    #[test]
    fn replaces_element_inner_html_with_parsed_fragment_children() {
        let mut document = HtmlDocument::parse("<div id=target>Visible</div>");
        let target = must_some(
            document.get_element_by_id("target"),
            "target element was not indexed",
        );

        must(document.set_inner_html(target, "<span>replacement</span>"));

        assert_eq!(
            must(document.inner_html(target)),
            "<span>replacement</span>".to_string()
        );
    }

    #[test]
    fn inner_html_rejects_missing_node() {
        let document = HtmlDocument::parse("<p id=target>Visible</p>");

        assert_eq!(
            document.inner_html(999),
            Err("HTML node 999 does not exist".to_string())
        );
    }

    #[test]
    fn fragment_root_prefers_body_when_present() {
        let dom = parse_document(RcDom::default(), Default::default())
            .one("<html><body><span>body</span></body></html>");
        let root = fragment_root(&dom.document);

        assert!(
            matches!(&root.data, markup5ever_rcdom::NodeData::Element { name, .. } if name.local.as_ref() == "body")
        );
    }

    #[test]
    fn fragment_root_falls_back_to_document_without_body() {
        let dom = RcDom::default();
        let root = fragment_root(&dom.document);

        assert!(Rc::ptr_eq(&root, &dom.document));
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
    fn helper_error_branches_cover_test_value_types() {
        assert!(
            std::panic::catch_unwind(|| {
                let _: String = must::<String, String>(Err("boom".to_string()));
            })
            .is_err()
        );
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
}
