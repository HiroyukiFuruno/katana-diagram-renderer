use super::html_css::HtmlAttributes;

#[path = "html_css_selector_parse.rs"]
mod parse;

const CLASS_SPECIFICITY: u16 = 10;
const ID_SPECIFICITY: u16 = 100;

#[derive(Debug, Clone)]
pub(super) struct CssAncestor {
    tag: String,
    attributes: HtmlAttributes,
}

impl CssAncestor {
    pub(super) fn new(tag: &str, attributes: &HtmlAttributes) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: attributes.clone(),
        }
    }
}

#[derive(Debug)]
pub(super) struct CssSelector {
    compounds: Vec<CssCompoundSelector>,
    combinators: Vec<CssCombinator>,
    inherited_from_body: bool,
}

#[derive(Debug)]
struct CssCompoundSelector {
    tag: Option<String>,
    classes: Vec<String>,
    id: Option<String>,
    attributes: Vec<CssAttributeSelector>,
}

#[derive(Debug)]
struct CssAttributeSelector {
    name: String,
    value: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum CssCombinator {
    Descendant,
    Child,
}

impl CssSelector {
    pub(super) fn parse(raw: &str) -> Option<Self> {
        parse::selector(raw)
    }

    pub(super) fn matches(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> bool {
        self.matches_from(self.compounds.len() - 1, tag, attributes, ancestors)
    }

    fn matches_from(
        &self,
        index: usize,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> bool {
        if !self.compounds[index].matches(tag, attributes) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match self.combinators[index - 1] {
            CssCombinator::Child => self.matches_parent(index, ancestors),
            CssCombinator::Descendant => self.matches_ancestor(index, ancestors),
        }
    }

    fn matches_parent(&self, index: usize, ancestors: &[CssAncestor]) -> bool {
        ancestors.last().is_some_and(|parent| {
            self.matches_from(
                index - 1,
                &parent.tag,
                &parent.attributes,
                &ancestors[..ancestors.len() - 1],
            )
        })
    }

    fn matches_ancestor(&self, index: usize, ancestors: &[CssAncestor]) -> bool {
        ancestors.iter().enumerate().rev().any(|(position, item)| {
            self.matches_from(
                index - 1,
                &item.tag,
                &item.attributes,
                &ancestors[..position],
            )
        })
    }

    pub(super) fn matches_static_snapshot(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.inherited_from_body
            || (self.static_snapshot_supported() && self.matches(tag, attributes, &[]))
    }

    fn static_snapshot_supported(&self) -> bool {
        self.compounds.len() == 1
            && self.compounds[0].classes.len() <= 1
            && self.compounds[0].attributes.is_empty()
    }

    pub(super) fn specificity(&self) -> u16 {
        self.compounds
            .iter()
            .map(CssCompoundSelector::specificity)
            .sum()
    }

    pub(super) fn static_snapshot_specificity(&self) -> u16 {
        if self.inherited_from_body {
            0
        } else {
            self.specificity()
        }
    }
}

impl CssCompoundSelector {
    fn matches(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.tag
            .as_ref()
            .is_none_or(|name| name.eq_ignore_ascii_case(tag))
            && self
                .classes
                .iter()
                .all(|class| has_class(attributes, class))
            && self
                .id
                .as_ref()
                .is_none_or(|id| attribute_value(attributes, "id").is_some_and(|value| value == id))
            && self
                .attributes
                .iter()
                .all(|selector| selector.matches(attributes))
    }

    fn specificity(&self) -> u16 {
        self.tag.iter().count() as u16
            + (self.classes.len() + self.attributes.len()) as u16 * CLASS_SPECIFICITY
            + self.id.iter().count() as u16 * ID_SPECIFICITY
    }
}

impl CssAttributeSelector {
    fn matches(&self, attributes: &HtmlAttributes) -> bool {
        attribute_value(attributes, &self.name).is_some_and(|candidate| {
            self.value
                .as_ref()
                .is_none_or(|expected| candidate == expected)
        })
    }
}

fn has_class(attributes: &HtmlAttributes, class: &str) -> bool {
    attribute_value(attributes, "class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn attribute_value<'a>(attributes: &'a HtmlAttributes, name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[cfg(test)]
mod tests {
    use super::{CssAncestor, CssSelector};

    #[test]
    fn selector_parser_rejects_invalid_combinators_and_suffixes() {
        assert!(CssSelector::parse(".bad!").is_none());
        assert!(CssSelector::parse("bad!").is_none());
        assert!(CssSelector::parse("main + p").is_none());
        assert!(CssSelector::parse("main ~ p").is_none());
        assert!(CssSelector::parse("main > > p").is_none());
        assert!(CssSelector::parse("a:hover").is_none());
        assert!(CssSelector::parse("a[href=\"https://example.com\"]:hover").is_none());
        assert!(CssSelector::parse("[data-state").is_none());
        assert!(CssSelector::parse("[data!=ready]").is_none());
    }

    #[test]
    fn attribute_selector_with_scheme_url_matches_stylesheet_selector() -> Result<(), String> {
        let selector = CssSelector::parse("[href=\"https://example.com\"]")
            .ok_or("attribute selector must parse")?;
        let attributes = attributes(&[("href", "https://example.com")]);

        assert!(selector.matches("a", &attributes, &[]));
        Ok(())
    }

    #[test]
    fn attribute_selector_with_scheme_url_matches_query_selector() -> Result<(), String> {
        let selector = CssSelector::parse("a[href=\"https://example.com\"]")
            .ok_or("attribute selector must parse")?;
        let ancestors = vec![ancestor("body", &[])];
        let attributes = attributes(&[("href", "https://example.com")]);

        assert!(selector.matches("a", &attributes, &ancestors));
        Ok(())
    }

    #[test]
    fn compound_child_and_descendant_selectors_match_ancestry() -> Result<(), String> {
        let selector =
            CssSelector::parse("main > section.card[data-state=ready] p.message.emphasis")
                .ok_or("selector must parse")?;
        let ancestors = vec![
            ancestor("html", &[]),
            ancestor("body", &[]),
            ancestor("main", &[]),
            ancestor("section", &[("class", "card"), ("data-state", "ready")]),
        ];
        let attributes = attributes(&[("class", "message emphasis")]);

        assert!(selector.matches("p", &attributes, &ancestors));
        assert_eq!(selector.specificity(), 43);
        Ok(())
    }

    #[test]
    fn body_selector_keeps_static_snapshot_inheritance_only() -> Result<(), String> {
        let selector = CssSelector::parse("body").ok_or("body selector must parse")?;
        let attributes = Vec::new();

        assert!(selector.matches("body", &attributes, &[]));
        assert!(!selector.matches("p", &attributes, &[]));
        assert!(selector.matches_static_snapshot("p", &attributes));
        assert_eq!(selector.static_snapshot_specificity(), 0);
        Ok(())
    }

    #[test]
    fn selector_matching_rejects_compound_mismatches() -> Result<(), String> {
        let selector = CssSelector::parse("button.primary#submit[data-action=save]")
            .ok_or("compound selector must parse")?;
        let attributes = attributes(&[
            ("class", "primary selected"),
            ("id", "submit"),
            ("data-action", "save"),
        ]);

        assert!(selector.matches("button", &attributes, &[]));
        assert!(!selector.matches("a", &attributes, &[]));
        Ok(())
    }

    fn ancestor(tag: &str, values: &[(&str, &str)]) -> CssAncestor {
        CssAncestor::new(tag, &attributes(values))
    }

    fn attributes(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }
}
