use super::super::svg::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, serialize_embedded_svg};
use super::{HtmlAttributes, HtmlDocumentNode};
use markup5ever_rcdom::Handle;

pub(super) fn embedded_svg_node(
    node_id: u64,
    tag: String,
    mut attributes: HtmlAttributes,
    node: &Handle,
) -> HtmlDocumentNode {
    let root_style = attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .map(|(_, value)| value.as_str());
    attributes.push((
        EMBEDDED_SVG_MARKUP_ATTRIBUTE.to_string(),
        serialize_embedded_svg(node, root_style),
    ));
    HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children: Vec::new(),
    }
}
