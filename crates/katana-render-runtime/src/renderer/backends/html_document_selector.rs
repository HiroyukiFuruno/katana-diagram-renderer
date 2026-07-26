use super::attributes;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssSelector};
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn matches_selector(node: &Handle, selector: &CssSelector) -> bool {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return false;
    };
    let tag = name.local.to_string().to_ascii_lowercase();
    let attributes = attributes(&attrs.borrow());
    selector.matches_at(
        &tag,
        &attributes,
        &selector_ancestors(node),
        element_sibling_index(node),
    )
}

fn selector_ancestors(node: &Handle) -> Vec<CssAncestor> {
    let mut current = parent(node);
    let mut ancestors = Vec::new();
    while let Some(node) = current {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            let tag = name.local.to_string().to_ascii_lowercase();
            ancestors.push(CssAncestor::new_at(
                &tag,
                &attributes(&attrs.borrow()),
                element_sibling_index(&node),
            ));
        }
        current = parent(&node);
    }
    ancestors.reverse();
    ancestors
}

fn parent(node: &Handle) -> Option<Handle> {
    let parent = node.parent.take();
    node.parent.set(parent.clone());
    parent.and_then(|parent| parent.upgrade())
}

fn element_sibling_index(node: &Handle) -> usize {
    let Some(parent) = parent(node) else {
        return 1;
    };
    let mut index = 0;
    for child in parent.children.borrow().iter() {
        if matches!(&child.data, NodeData::Element { .. }) {
            index += 1;
        }
        if std::rc::Rc::ptr_eq(child, node) {
            return index.max(1);
        }
    }
    1
}

pub(super) fn find_selector(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
) -> Option<Handle> {
    find_selector_at(node, selector, ancestors, 1)
}

fn find_selector_at(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
    sibling_index: usize,
) -> Option<Handle> {
    let mut child_ancestors = ancestors.to_vec();
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        let attributes = attributes(&attrs.borrow());
        if selector.matches_at(&tag, &attributes, ancestors, sibling_index) {
            return Some(node.clone());
        }
        child_ancestors.push(CssAncestor::new_at(&tag, &attributes, sibling_index));
    }
    let mut child_index = 0;
    for child in node.children.borrow().iter() {
        if matches!(&child.data, NodeData::Element { .. }) {
            child_index += 1;
        }
        if let Some(found) = find_selector_at(child, selector, &child_ancestors, child_index) {
            return Some(found);
        }
    }
    None
}

pub(super) fn collect_selectors(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
    matches: &mut Vec<Handle>,
) {
    collect_selectors_at(node, selector, ancestors, matches, 1);
}

fn collect_selectors_at(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
    matches: &mut Vec<Handle>,
    sibling_index: usize,
) {
    let mut child_ancestors = ancestors.to_vec();
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        let attributes = attributes(&attrs.borrow());
        if selector.matches_at(&tag, &attributes, ancestors, sibling_index) {
            matches.push(node.clone());
        }
        child_ancestors.push(CssAncestor::new_at(&tag, &attributes, sibling_index));
    }
    let mut child_index = 0;
    for child in node.children.borrow().iter() {
        if matches!(&child.data, NodeData::Element { .. }) {
            child_index += 1;
        }
        collect_selectors_at(child, selector, &child_ancestors, matches, child_index);
    }
}

#[cfg(test)]
mod tests {
    use super::super::HtmlDocument;
    use super::{element_sibling_index, parent};

    #[test]
    fn detached_child_uses_the_first_sibling_index() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<div><span id=target></span></div>");
        let node_id = document
            .query_selector("#target")
            .ok_or("target node must exist")?;
        let node = document.node(node_id)?;
        let parent = parent(&node).ok_or("target parent must exist")?;
        parent.children.borrow_mut().clear();

        assert_eq!(element_sibling_index(&node), 1);
        Ok(())
    }
}
