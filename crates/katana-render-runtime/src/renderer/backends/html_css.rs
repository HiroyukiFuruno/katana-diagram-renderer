use super::html_css_cascade::{inherited_custom_properties, resolved_css_values};
use super::html_css_rule::{CssRule, CssRuleMatchState, parse_rules};
use super::html_css_selector::{CssAncestor, CssPseudoElement};
use super::html_css_sources::{inline_styles, interactive_styles};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;

#[path = "html_css_generated_content.rs"]
mod generated_content;
#[path = "html_css_resolution.rs"]
mod resolution;

use generated_content::parse_generated_content;
pub(super) use generated_content::{CssGeneratedContent, CssPseudoStyle};
use resolution::select_inline_declarations;

pub(super) type HtmlAttributes = Vec<(String, String)>;

pub(super) struct CssPseudoRequest<'a> {
    pub(super) tag: &'a str,
    pub(super) attributes: &'a HtmlAttributes,
    pub(super) ancestors: &'a [CssAncestor],
    pub(super) inheritance_ancestors: &'a [CssAncestor],
    pub(super) sibling_index: usize,
    pub(super) hovered: bool,
    pub(super) pseudo_element: CssPseudoElement,
}

const DEFAULT_CSS_VIEWPORT_WIDTH: f32 = 1024.0;

#[derive(Debug)]
pub(super) struct StaticCss {
    rules: Vec<CssRule>,
    mode: CssResolutionMode,
    viewport_width: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum CssResolutionMode {
    #[default]
    StaticSnapshot,
    InteractiveRuntime,
}

impl StaticCss {
    pub(super) fn from_document(document: &Handle) -> Self {
        Self::with_mode(document, CssResolutionMode::StaticSnapshot)
    }

    #[cfg(test)]
    pub(super) fn for_interactive_document_with_styles(
        document: &Handle,
        external_stylesheets: &HashMap<String, String>,
    ) -> Self {
        Self::for_interactive_document_with_styles_at_width(
            document,
            external_stylesheets,
            DEFAULT_CSS_VIEWPORT_WIDTH,
        )
    }

    pub(super) fn for_interactive_document_with_styles_at_width(
        document: &Handle,
        external_stylesheets: &HashMap<String, String>,
        viewport_width: f32,
    ) -> Self {
        let source = interactive_styles(document, external_stylesheets);
        Self {
            rules: parse_rules(&source),
            mode: CssResolutionMode::InteractiveRuntime,
            viewport_width,
        }
    }

    fn with_mode(document: &Handle, mode: CssResolutionMode) -> Self {
        let source = inline_styles(document);
        Self {
            rules: parse_rules(&source),
            mode,
            viewport_width: DEFAULT_CSS_VIEWPORT_WIDTH,
        }
    }

    pub(super) fn apply(&self, tag: &str, attributes: &HtmlAttributes) -> HtmlAttributes {
        self.apply_with_ancestors(tag, attributes, &[])
    }

    pub(super) fn apply_with_ancestors(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> HtmlAttributes {
        self.apply_with_ancestors_at(tag, attributes, ancestors, 1)
    }

    pub(super) fn apply_with_ancestors_at(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
    ) -> HtmlAttributes {
        self.apply_with_ancestors_at_state(tag, attributes, ancestors, sibling_index, false)
    }

    pub(super) fn apply_with_ancestors_at_state(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
    ) -> HtmlAttributes {
        let mut rendered = attributes
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("style"))
            .cloned()
            .collect::<HtmlAttributes>();
        let stylesheet =
            self.resolved_declarations(tag, attributes, ancestors, sibling_index, hovered);
        if stylesheet.is_empty() {
            return rendered;
        }
        rendered.push(("style".to_string(), stylesheet.join("; ")));
        rendered
    }

    fn resolved_declarations(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
    ) -> Vec<String> {
        let mut selected = inherited_custom_properties(ancestors);
        let state = CssRuleMatchState::element(tag, attributes, ancestors, sibling_index, hovered);
        self.select_rule_declarations(state, &mut selected);
        select_inline_declarations(attributes, &mut selected);
        resolved_css_values(selected)
    }

    pub(super) fn pseudo_style_at_state(
        &self,
        request: CssPseudoRequest<'_>,
    ) -> Option<CssPseudoStyle> {
        if self.mode != CssResolutionMode::InteractiveRuntime {
            return None;
        }
        let mut selected = inherited_custom_properties(request.inheritance_ancestors);
        let state = CssRuleMatchState::element(
            request.tag,
            request.attributes,
            request.ancestors,
            request.sibling_index,
            request.hovered,
        )
        .for_pseudo(request.pseudo_element);
        self.select_rule_declarations(state, &mut selected);
        let declarations = resolved_css_values(selected);
        let content = declarations
            .iter()
            .find_map(|declaration| declaration.strip_prefix("content: "))
            .and_then(parse_generated_content)?;
        Some(CssPseudoStyle {
            attributes: vec![("style".to_string(), declarations.join("; "))],
            content,
        })
    }

    fn select_rule_declarations(
        &self,
        state: CssRuleMatchState<'_>,
        selected: &mut Vec<super::html_css_cascade::SelectedDeclaration>,
    ) {
        for (rule_order, rule) in self.rules.iter().enumerate() {
            let Some(specificity) = self.rule_specificity(rule, state) else {
                continue;
            };
            self.select_matched_declarations(rule, rule_order, specificity, selected);
        }
    }

    fn rule_specificity(&self, rule: &CssRule, state: CssRuleMatchState<'_>) -> Option<u16> {
        match self.mode {
            CssResolutionMode::StaticSnapshot => state
                .pseudo_element
                .is_none()
                .then(|| rule.matches_static_snapshot(state.tag, state.attributes))
                .flatten(),
            CssResolutionMode::InteractiveRuntime => rule.matches_state(state, self.viewport_width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::html_css_cascade::resolve_css_variables;
    use super::super::html_document::{HtmlDocument, HtmlDocumentNode};
    use super::{CssPseudoElement, CssPseudoRequest, HtmlAttributes, StaticCss};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn longhand_padding_is_resolved_before_shorthand() {
        let document = HtmlDocument::parse(
            "<style>.card { padding-left: 20px; } div { padding: 0; }</style>prefix<div class='card'>A</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "div");

        assert_eq!(
            style.as_deref(),
            Some("padding-left: 20px; padding-top: 0; padding-right: 0; padding-bottom: 0")
        );
    }

    #[test]
    fn later_longhand_wins_after_an_intervening_shorthand() {
        let document = HtmlDocument::parse(
            "<style>.card { padding-left: 20px; padding: 0; padding-left: 10px; }</style><div class='card'>A</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "div");

        assert_eq!(
            style.as_deref(),
            Some("padding-left: 10px; padding-top: 0; padding-right: 0; padding-bottom: 0")
        );
    }

    #[test]
    fn comma_separated_tag_reset_participates_in_the_author_cascade() {
        let document = HtmlDocument::parse(
            "<style>h1, h2, p { margin: 0; } .heading { margin-bottom: 20px; }</style><h1 class=heading>Title</h1>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "h1");

        assert!(
            style.as_ref().is_some_and(|style| {
                style.contains("margin-top: 0")
                    && style.contains("margin-right: 0")
                    && style.contains("margin-bottom: 20px")
                    && style.contains("margin-left: 0")
            }),
            "tag reset was missing from computed style: {style:?}"
        );
    }

    #[test]
    fn important_inline_and_custom_properties_follow_cascade_precedence() {
        let document = HtmlDocument::parse(
            "<style>:root { --accent: #123456; } .card { color: red !important; } #card { color: blue; }</style><div id=card class=card style='color: var(--accent)'>A</div>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let style = find_style_attribute(&nodes, "div");
        assert!(
            style.as_ref().is_some_and(|style| {
                style.contains("color: red") && style.contains("--accent: #123456")
            }),
            "computed style did not preserve cascade precedence: {style:?}"
        );
    }

    #[test]
    fn nth_child_cascade_uses_element_sibling_order_not_text_nodes() {
        let document = HtmlDocument::parse(
            "<style>.layer { color: red; }.layer:nth-child(3) { color: blue; }</style><main>\n<div id=first class=layer>A</div>\n<span>B</span>\n<div id=third class=layer>C</div>\n</main>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());

        assert_eq!(
            find_style_for_id(&nodes, "first").as_deref(),
            Some("color: red")
        );
        assert_eq!(
            find_style_for_id(&nodes, "third").as_deref(),
            Some("color: blue")
        );
    }

    #[test]
    fn css_variables_resolve_nested_values_fallbacks_and_cycles() {
        let custom = HashMap::from([
            ("--a".to_string(), "var(--b)".to_string()),
            ("--b".to_string(), "12px".to_string()),
            ("--cycle".to_string(), "var(--cycle)".to_string()),
        ]);

        assert_eq!(
            resolve_css_variables("calc(var(--a) + 2px)", &custom, &mut HashSet::new()),
            Some("calc(12px + 2px)".to_string())
        );
        assert_eq!(
            resolve_css_variables("var(--missing, red)", &custom, &mut HashSet::new()),
            Some("red".to_string())
        );
        assert_eq!(
            resolve_css_variables("var(--cycle)", &custom, &mut HashSet::new()),
            None
        );
    }

    #[test]
    fn pseudo_style_is_unavailable_in_static_mode() {
        let document =
            HtmlDocument::parse("<style>div:before { content: 'x' }</style><div>Test</div>");
        let css = StaticCss::from_document(&document.document);
        let style = css.pseudo_style_at_state(CssPseudoRequest {
            tag: "div",
            attributes: &HtmlAttributes::new(),
            ancestors: &[],
            inheritance_ancestors: &[],
            sibling_index: 1,
            hovered: false,
            pseudo_element: CssPseudoElement::Before,
        });

        assert!(style.is_none());
    }

    fn find_style_attribute(nodes: &[HtmlDocumentNode], target_tag: &str) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            HtmlDocumentNode::Element {
                tag,
                attributes,
                children,
                ..
            } => {
                if tag == target_tag {
                    attributes
                        .iter()
                        .find(|(name, _)| name == "style")
                        .map(|(_, value)| value.clone())
                } else {
                    find_style_attribute(children, target_tag)
                }
            }
            HtmlDocumentNode::Text(_) => None,
        })
    }

    fn find_style_for_id(nodes: &[HtmlDocumentNode], target_id: &str) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            HtmlDocumentNode::Element {
                attributes,
                children,
                ..
            } => {
                let matches = attributes
                    .iter()
                    .any(|(name, value)| name == "id" && value == target_id);
                if matches {
                    attributes
                        .iter()
                        .find(|(name, _)| name == "style")
                        .map(|(_, value)| value.clone())
                } else {
                    find_style_for_id(children, target_id)
                }
            }
            HtmlDocumentNode::Text(_) => None,
        })
    }
}
