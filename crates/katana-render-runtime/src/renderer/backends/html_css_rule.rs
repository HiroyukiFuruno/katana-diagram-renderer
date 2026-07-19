use super::html_css::HtmlAttributes;
use super::html_css_selector::CssSelector;

#[derive(Debug)]
pub(super) struct CssRule {
    selectors: Vec<CssSelector>,
    pub(super) declarations: Vec<CssDeclaration>,
}

impl CssRule {
    pub(super) fn matches(&self, tag: &str, attributes: &HtmlAttributes) -> Option<u16> {
        self.selectors
            .iter()
            .filter(|selector| selector.matches(tag, attributes))
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
            (supported_property(&name) && !value.is_empty()).then_some(CssDeclaration {
                name,
                value: value.to_string(),
            })
        })
        .collect()
}

fn supported_property(name: &str) -> bool {
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
            | "border"
            | "border-color"
            | "padding"
            | "margin"
            | "margin-top"
            | "margin-bottom"
            | "width"
            | "height"
            | "min-height"
            | "display"
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_rules, supported_property};

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
            vec!["margin", "padding", "width", "min-height", "font-size"]
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
}
