use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

pub(super) fn collect_scripts(node: &Handle, scripts: &mut Vec<String>) -> Result<(), String> {
    if let NodeData::Element { name, attrs, .. } = &node.data
        && name.local.as_ref().eq_ignore_ascii_case("script")
    {
        if let Some(source) = attribute_value(&attrs.borrow(), "src") {
            return Err(format!("external script is not supported: {source}"));
        }
        scripts.push(text_content(node));
        return Ok(());
    }
    for child in node.children.borrow().iter() {
        collect_scripts(child, scripts)?;
    }
    Ok(())
}

pub(super) fn find_element(
    node: &Handle,
    predicate: impl Fn(&str, &[html5ever::Attribute]) -> bool + Copy,
) -> Option<Handle> {
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        if predicate(&tag, &attrs.borrow()) {
            return Some(node.clone());
        }
    }
    node.children
        .borrow()
        .iter()
        .find_map(|child| find_element(child, predicate))
}

pub(super) fn selector_matches(
    selector: &str,
    tag: &str,
    attributes: &[html5ever::Attribute],
) -> bool {
    if let Some((base, attribute)) = selector
        .strip_suffix(']')
        .and_then(|selector| selector.split_once('['))
    {
        return base_selector_matches(base, tag, attributes)
            && attribute_selector_matches(attribute, attributes);
    }
    base_selector_matches(selector, tag, attributes)
}

fn base_selector_matches(selector: &str, tag: &str, attributes: &[html5ever::Attribute]) -> bool {
    if selector.is_empty() {
        return true;
    }
    if let Some(id) = selector.strip_prefix('#') {
        return attribute_value(attributes, "id") == Some(id);
    }
    if let Some(class) = selector.strip_prefix('.') {
        return has_class(attributes, class);
    }
    if let Some((tag_name, class)) = selector.split_once('.') {
        return tag == tag_name.to_ascii_lowercase() && has_class(attributes, class);
    }
    tag == selector.to_ascii_lowercase()
}

fn attribute_selector_matches(selector: &str, attributes: &[html5ever::Attribute]) -> bool {
    let (name, value) = match selector.split_once('=') {
        Some((name, value)) => (name.trim(), Some(value.trim().trim_matches(['\'', '"']))),
        None => (selector.trim(), None),
    };
    (!name.is_empty())
        && attribute_value(attributes, name).is_some_and(|candidate| {
            value.is_none_or(|expected| candidate.eq_ignore_ascii_case(expected))
        })
}

pub(super) fn attribute_value<'a>(
    attributes: &'a [html5ever::Attribute],
    name: &str,
) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name.local.as_ref().eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value.as_ref())
}

pub(super) fn text_content(node: &Handle) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => node.children.borrow().iter().map(text_content).collect(),
    }
}

pub(super) fn detach(node: &Handle) {
    let Some(parent) = node.parent.take().and_then(|parent| parent.upgrade()) else {
        return;
    };
    parent
        .children
        .borrow_mut()
        .retain(|child| !Rc::ptr_eq(child, node));
}

fn has_class(attributes: &[html5ever::Attribute], class: &str) -> bool {
    attribute_value(attributes, "class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|candidate| candidate == class)
    })
}

#[cfg(test)]
mod tests {
    use super::selector_matches;
    use html5ever::{Attribute, QualName};

    #[test]
    fn selector_matches_attribute_presence_and_value_forms() {
        let attributes = vec![attribute("data-action", "save"), attribute("id", "submit")];

        assert!(selector_matches("[data-action]", "button", &attributes));
        assert!(selector_matches(
            "[data-action=save]",
            "button",
            &attributes
        ));
        assert!(selector_matches(
            "button[data-action=save]",
            "button",
            &attributes
        ));
        assert!(selector_matches(
            "#submit[data-action='save']",
            "button",
            &attributes
        ));
        assert!(!selector_matches(
            "[data-action=discard]",
            "button",
            &attributes
        ));
    }

    fn attribute(name: &str, value: &str) -> Attribute {
        Attribute {
            name: QualName::new(None, Default::default(), name.into()),
            value: value.into(),
        }
    }
}
