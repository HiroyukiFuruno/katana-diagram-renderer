use super::html_css_rule::{CssDeclaration, CssRule, parse_rules};
use super::html_css_selector::CssAncestor;
use super::html_css_sources::{inline_styles, interactive_styles};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;

pub(super) type HtmlAttributes = Vec<(String, String)>;

#[derive(Debug, Default)]
pub(super) struct StaticCss {
    rules: Vec<CssRule>,
    mode: CssResolutionMode,
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

    pub(super) fn for_interactive_document_with_styles(
        document: &Handle,
        external_stylesheets: &HashMap<String, String>,
    ) -> Self {
        let source = interactive_styles(document, external_stylesheets);
        Self {
            rules: parse_rules(&source),
            mode: CssResolutionMode::InteractiveRuntime,
        }
    }

    fn with_mode(document: &Handle, mode: CssResolutionMode) -> Self {
        let source = inline_styles(document);
        Self {
            rules: parse_rules(&source),
            mode,
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
        let mut rendered = attributes
            .iter()
            .filter(|(name, _)| !name.eq_ignore_ascii_case("style"))
            .cloned()
            .collect::<HtmlAttributes>();
        let stylesheet = self.resolved_declarations(tag, attributes, ancestors);
        let inline = style_attribute(attributes);
        if stylesheet.is_empty() && inline.trim().is_empty() {
            return rendered;
        }
        let mut declarations = stylesheet;
        if !inline.trim().is_empty() {
            declarations.push(inline);
        }
        rendered.push(("style".to_string(), declarations.join("; ")));
        rendered
    }

    fn resolved_declarations(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> Vec<String> {
        let mut selected = Vec::<SelectedDeclaration>::new();
        for (rule_order, rule) in self.rules.iter().enumerate() {
            let matches = match self.mode {
                CssResolutionMode::StaticSnapshot => rule.matches_static_snapshot(tag, attributes),
                CssResolutionMode::InteractiveRuntime => rule.matches(tag, attributes, ancestors),
            };
            let Some(specificity) = matches else {
                continue;
            };
            for declaration in &rule.declarations {
                if self.mode == CssResolutionMode::StaticSnapshot
                    && !static_snapshot_property(&declaration.name)
                {
                    continue;
                }
                select_declaration(&mut selected, declaration, specificity, rule_order);
            }
        }
        selected
            .into_iter()
            .map(|selected| format!("{}: {}", selected.name, selected.value))
            .collect()
    }
}

fn static_snapshot_property(name: &str) -> bool {
    matches!(
        name,
        "background"
            | "background-color"
            | "color"
            | "font-family"
            | "font-style"
            | "font-weight"
            | "font-size"
            | "line-height"
            | "text-align"
            | "text-decoration"
    )
}

#[derive(Debug)]
struct SelectedDeclaration {
    name: String,
    value: String,
    specificity: u16,
    rule_order: usize,
}

fn style_attribute(attributes: &HtmlAttributes) -> String {
    attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("style"))
        .map_or_else(String::new, |(_, value)| value.clone())
}

fn select_declaration(
    selected: &mut Vec<SelectedDeclaration>,
    declaration: &CssDeclaration,
    specificity: u16,
    rule_order: usize,
) {
    let candidate = SelectedDeclaration {
        name: declaration.name.clone(),
        value: declaration.value.clone(),
        specificity,
        rule_order,
    };
    let Some(current) = selected
        .iter_mut()
        .find(|item| item.name == declaration.name)
    else {
        selected.push(candidate);
        return;
    };
    if (candidate.specificity, candidate.rule_order) >= (current.specificity, current.rule_order) {
        *current = candidate;
    }
}

#[cfg(test)]
mod tests {
    use super::super::html_document::{HtmlDocument, HtmlDocumentNode};
    use std::collections::HashMap;

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
}
