use super::attributes;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssSelector};
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn find_selector(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
) -> Option<Handle> {
    let mut child_ancestors = ancestors.to_vec();
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        let attributes = attributes(&attrs.borrow());
        if selector.matches(&tag, &attributes, ancestors) {
            return Some(node.clone());
        }
        child_ancestors.push(CssAncestor::new(&tag, &attributes));
    }
    node.children
        .borrow()
        .iter()
        .find_map(|child| find_selector(child, selector, &child_ancestors))
}

pub(super) fn collect_selectors(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
    matches: &mut Vec<Handle>,
) {
    let mut child_ancestors = ancestors.to_vec();
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        let attributes = attributes(&attrs.borrow());
        if selector.matches(&tag, &attributes, ancestors) {
            matches.push(node.clone());
        }
        child_ancestors.push(CssAncestor::new(&tag, &attributes));
    }
    for child in node.children.borrow().iter() {
        collect_selectors(child, selector, &child_ancestors, matches);
    }
}
