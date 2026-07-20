use super::html_css::HtmlAttributes;
use super::html_css_selector::{CssAncestor, CssSelector};
#[path = "html_css_shorthand.rs"]
mod html_css_shorthand;

#[derive(Debug)]
pub(super) struct CssRule {
    selectors: Vec<CssSelector>,
    pub(super) declarations: Vec<CssDeclaration>,
}

impl CssRule {
    pub(super) fn matches(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> Option<u16> {
        self.selectors
            .iter()
            .filter(|selector| selector.matches(tag, attributes, ancestors))
            .map(CssSelector::specificity)
            .max()
    }

    pub(super) fn matches_static_snapshot(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
    ) -> Option<u16> {
        self.selectors
            .iter()
            .filter(|selector| selector.matches_static_snapshot(tag, attributes))
            .map(CssSelector::static_snapshot_specificity)
            .max()
    }
}

#[derive(Debug)]
pub(super) struct CssDeclaration {
    pub(super) name: String,
    pub(super) value: String,
}

pub(super) fn parse_rules(source: &str) -> Vec<CssRule> {
    remove_comments(source)
        .split('}')
        .filter_map(|rule| rule.split_once('{'))
        .filter_map(|(selectors, declarations)| {
            let selectors = selectors
                .split(',')
                .filter_map(CssSelector::parse)
                .collect::<Vec<_>>();
            let declarations = parse_declarations(declarations);
            (!selectors.is_empty() && !declarations.is_empty()).then_some(CssRule {
                selectors,
                declarations,
            })
        })
        .collect()
}

fn remove_comments(source: &str) -> String {
    let mut output = String::new();
    let mut remaining = source;
    while let Some(start) = remaining.find("/*") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find("*/") else {
            return output;
        };
        remaining = &after_start[end + 2..];
    }
    output.push_str(remaining);
    output
}

fn parse_declarations(source: &str) -> Vec<CssDeclaration> {
    source
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .filter_map(|(name, value)| {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            (supported_property(&name) && !value.is_empty()).then(|| {
                expand_box_shorthand(&name, value).unwrap_or_else(|| {
                    vec![CssDeclaration {
                        name: name.clone(),
                        value: value.to_string(),
                    }]
                })
            })
        })
        .flatten()
        .collect()
}

fn expand_box_shorthand(name: &str, value: &str) -> Option<Vec<CssDeclaration>> {
    html_css_shorthand::expand_box_shorthand(name, value)
}

fn supported_property(name: &str) -> bool {
    is_paint_property(name) || is_box_property(name) || is_flow_property(name)
}

fn is_paint_property(name: &str) -> bool {
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

fn is_box_property(name: &str) -> bool {
    matches!(
        name,
        "border"
            | "border-color"
            | "padding"
            | "padding-top"
            | "padding-right"
            | "padding-bottom"
            | "padding-left"
            | "margin"
            | "margin-top"
            | "margin-right"
            | "margin-bottom"
            | "margin-left"
            | "width"
            | "max-width"
            | "height"
            | "min-height"
    )
}

fn is_flow_property(name: &str) -> bool {
    matches!(
        name,
        "display"
            | "gap"
            | "flex-direction"
            | "flex-wrap"
            | "flex-grow"
            | "flex-shrink"
            | "align-items"
            | "justify-content"
            | "grid-template-columns"
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_declarations, parse_rules, supported_property};

    #[test]
    fn rules_keep_layout_declarations_for_the_interactive_runtime() {
        let rules = parse_rules(
            "main { margin: 24px; padding: 16px; width: 600px; min-height: 400px; font-size: 18px; }",
        );

        assert_eq!(rules.len(), 1);
        let rule = &rules[0];
        let declarations = rule
            .declarations
            .iter()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            declarations,
            vec![
                "margin-top",
                "margin-right",
                "margin-bottom",
                "margin-left",
                "padding-top",
                "padding-right",
                "padding-bottom",
                "padding-left",
                "width",
                "min-height",
                "font-size",
            ]
        );
    }

    #[test]
    fn display_is_an_interactive_css_property_but_unknown_names_are_not() {
        assert!(supported_property("display"));
        assert!(!supported_property("position"));
        let rules = parse_rules("p { display: none; position: fixed; }");

        assert_eq!(rules[0].declarations.len(), 1);
        assert_eq!(rules[0].declarations[0].name, "display");
    }

    #[test]
    fn parse_box_shorthand_is_expanded_into_longhands() {
        let declarations = parse_declarations(
            "padding: 4px 5px 6px 7px; margin: 10px 20px; margin-top: 30px; padding: 1px 2px 3px 4px 5px;",
        );
        let actual = declarations
            .iter()
            .map(|declaration| (declaration.name.as_str(), declaration.value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(actual, expected_box_declarations());
    }

    fn expected_box_declarations() -> Vec<(&'static str, &'static str)> {
        vec![
            ("padding-top", "4px"),
            ("padding-right", "5px"),
            ("padding-bottom", "6px"),
            ("padding-left", "7px"),
            ("margin-top", "10px"),
            ("margin-right", "20px"),
            ("margin-bottom", "10px"),
            ("margin-left", "20px"),
            ("margin-top", "30px"),
            ("padding", "1px 2px 3px 4px 5px"),
        ]
    }
}
