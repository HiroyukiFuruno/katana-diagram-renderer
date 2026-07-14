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
            | "text-align"
            | "text-decoration"
    )
}
