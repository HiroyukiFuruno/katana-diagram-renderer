use super::html_css::HtmlAttributes;

#[path = "html_css_selector_match.rs"]
mod matching;
#[path = "html_css_selector_parse.rs"]
mod parse;

const CLASS_SPECIFICITY: u16 = 10;
const ID_SPECIFICITY: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CssPseudoElement {
    Before,
    After,
}

#[derive(Debug, Clone)]
pub(super) struct CssAncestor {
    tag: String,
    attributes: HtmlAttributes,
    sibling_index: usize,
    hovered: bool,
}

impl CssAncestor {
    #[cfg(test)]
    pub(super) fn new(tag: &str, attributes: &HtmlAttributes) -> Self {
        Self::new_at(tag, attributes, 1)
    }

    pub(super) fn new_at(tag: &str, attributes: &HtmlAttributes, sibling_index: usize) -> Self {
        Self::new_at_state(tag, attributes, sibling_index, false)
    }

    pub(super) fn new_at_state(
        tag: &str,
        attributes: &HtmlAttributes,
        sibling_index: usize,
        hovered: bool,
    ) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: attributes.clone(),
            sibling_index,
            hovered,
        }
    }

    pub(super) fn attributes(&self) -> &HtmlAttributes {
        &self.attributes
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
    root: bool,
    hovered: bool,
    disabled: bool,
    not_disabled: bool,
    checked: bool,
    nth_child: Option<CssNthExpression>,
    pseudo_element: Option<CssPseudoElement>,
}

#[derive(Debug)]
struct CssAttributeSelector {
    name: String,
    value: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CssNthExpression {
    step: i32,
    offset: i32,
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

    pub(super) fn matches_static_snapshot(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.inherited_from_body
            || (self.pseudo_element().is_none()
                && self.static_snapshot_supported()
                && self.matches(tag, attributes, &[]))
    }

    fn static_snapshot_supported(&self) -> bool {
        self.compounds.len() == 1
            && self.compounds[0].classes.len() <= 1
            && self.compounds[0].attributes.is_empty()
            && self.compounds[0].specificity() > 0
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

    pub(super) fn pseudo_element(&self) -> Option<CssPseudoElement> {
        self.compounds
            .last()
            .and_then(|compound| compound.pseudo_element)
    }
}

impl CssCompoundSelector {
    fn specificity(&self) -> u16 {
        self.tag.iter().count() as u16
            + (self.classes.len()
                + self.attributes.len()
                + usize::from(self.root)
                + usize::from(self.hovered)
                + usize::from(self.disabled)
                + usize::from(self.not_disabled)
                + usize::from(self.checked)
                + usize::from(self.nth_child.is_some())) as u16
                * CLASS_SPECIFICITY
            + self.pseudo_element.iter().count() as u16
            + self.id.iter().count() as u16 * ID_SPECIFICITY
    }
}

impl CssNthExpression {
    fn parse(source: &str) -> Option<Self> {
        let normalized = source
            .chars()
            .filter(|character| !character.is_ascii_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        match normalized.as_str() {
            "odd" => return Some(Self { step: 2, offset: 1 }),
            "even" => return Some(Self { step: 2, offset: 0 }),
            _ => {}
        }
        let Some((step, offset)) = normalized.split_once('n') else {
            return normalized
                .parse::<i32>()
                .ok()
                .map(|offset| Self { step: 0, offset });
        };
        let step = match step {
            "" | "+" => 1,
            "-" => -1,
            value => value.parse().ok()?,
        };
        let offset = if offset.is_empty() {
            0
        } else {
            offset.parse().ok()?
        };
        Some(Self { step, offset })
    }
}

#[cfg(test)]
mod tests {
    use super::{CssAncestor, CssNthExpression, CssPseudoElement, CssSelector};

    #[test]
    fn selector_parser_rejects_invalid_combinators() {
        assert!(CssSelector::parse("main + p").is_none());
        assert!(CssSelector::parse("main ~ p").is_none());
        assert!(CssSelector::parse("main > > p").is_none());
    }

    #[test]
    fn universal_selector_matches_every_element_with_zero_specificity() -> Result<(), String> {
        let selector = CssSelector::parse("*").ok_or("universal selector must parse")?;

        assert!(selector.matches("button", &Vec::new(), &[]));
        assert!(selector.matches("section", &Vec::new(), &[]));
        assert!(!selector.matches_static_snapshot("button", &Vec::new()));
        assert_eq!(selector.specificity(), 0);
        Ok(())
    }

    #[test]
    fn bare_dynamic_pseudo_selectors_parse_without_a_tag_or_class() {
        for source in [
            ":hover",
            ":disabled",
            ":not(:disabled)",
            ":checked",
            ":nth-child(2)",
        ] {
            assert!(CssSelector::parse(source).is_some(), "{source}");
        }
    }

    #[test]
    fn pseudo_elements_are_terminal_and_add_type_specificity() -> Result<(), String> {
        let before = CssSelector::parse(".toggle:before").ok_or(":before must parse")?;
        let after = CssSelector::parse("li > a::after").ok_or("::after must parse")?;

        assert_eq!(before.pseudo_element(), Some(CssPseudoElement::Before));
        assert_eq!(before.specificity(), 11);
        assert_eq!(after.pseudo_element(), Some(CssPseudoElement::After));
        assert_eq!(after.specificity(), 3);
        assert!(CssSelector::parse("main::before > span").is_none());
        assert!(CssSelector::parse("main::before::after").is_none());
        Ok(())
    }

    #[test]
    fn selector_parser_rejects_invalid_suffixes_and_attributes() {
        assert!(CssSelector::parse(".bad!").is_none());
        assert!(CssSelector::parse("bad!").is_none());
        assert!(CssSelector::parse("a:focus").is_none());
        assert!(CssSelector::parse("li:nth-child(broken)").is_none());
        assert!(CssSelector::parse("li:nth-child(2):nth-child(3)").is_none());
        assert!(CssSelector::parse("[data-state").is_none());
        assert!(CssSelector::parse("[data!=ready]").is_none());
    }

    #[test]
    fn nth_child_handles_zero_overflow_and_implicit_offsets() -> Result<(), String> {
        let even = CssNthExpression::parse("2n").ok_or("2n must parse")?;

        assert!(even.matches(2));
        assert!(!even.matches(0));
        assert!(!even.matches(usize::MAX));
        Ok(())
    }

    #[test]
    fn nth_child_matches_element_sibling_positions_and_ancestry() -> Result<(), String> {
        let odd = CssSelector::parse(".layer:nth-child(2n + 1)")
            .ok_or("nth-child selector must parse")?;
        let bounded =
            CssSelector::parse("li:nth-child(-n+3)").ok_or("bounded selector must parse")?;
        let nested = CssSelector::parse("main:nth-child(2) > p:nth-child(even)")
            .ok_or("nested selector must parse")?;
        let attributes = attributes(&[("class", "layer")]);

        assert!(odd.matches_at("div", &attributes, &[], 1));
        assert!(!odd.matches_at("div", &attributes, &[], 2));
        assert!(odd.matches_at("div", &attributes, &[], 5));
        assert!(bounded.matches_at("li", &Vec::new(), &[], 3));
        assert!(!bounded.matches_at("li", &Vec::new(), &[], 4));
        assert!(nested.matches_at(
            "p",
            &Vec::new(),
            &[CssAncestor::new_at("main", &Vec::new(), 2)],
            4,
        ));
        assert_eq!(nested.specificity(), 22);
        Ok(())
    }

    #[test]
    fn root_pseudo_class_matches_the_html_element_only() -> Result<(), String> {
        let selector = CssSelector::parse(":root").ok_or(":root selector must parse")?;

        assert!(selector.matches("html", &Vec::new(), &[]));
        assert!(!selector.matches("body", &Vec::new(), &[]));
        assert_eq!(selector.specificity(), 10);
        Ok(())
    }

    #[test]
    fn disabled_pseudo_class_matches_the_boolean_attribute() -> Result<(), String> {
        let selector =
            CssSelector::parse("button:disabled").ok_or(":disabled selector must parse")?;

        assert!(selector.matches("button", &attributes(&[("disabled", "")]), &[]));
        assert!(!selector.matches("button", &attributes(&[]), &[]));
        assert_eq!(selector.specificity(), 11);
        Ok(())
    }

    #[test]
    fn checked_pseudo_class_matches_the_boolean_attribute() -> Result<(), String> {
        let selector = CssSelector::parse("input:checked").ok_or(":checked selector must parse")?;

        assert!(selector.matches("input", &attributes(&[("checked", "")]), &[]));
        assert!(!selector.matches("input", &attributes(&[]), &[]));
        assert_eq!(selector.specificity(), 11);
        Ok(())
    }

    #[test]
    fn hover_and_not_disabled_match_dynamic_element_state() -> Result<(), String> {
        let selector = CssSelector::parse(".nav button:hover:not(:disabled)")
            .ok_or("dynamic selector must parse")?;
        let ancestors = [CssAncestor::new_at_state(
            "div",
            &attributes(&[("class", "nav")]),
            1,
            false,
        )];

        assert!(selector.matches_at_state("button", &Vec::new(), &ancestors, 1, true));
        assert!(!selector.matches_at_state("button", &Vec::new(), &ancestors, 1, false));
        assert!(!selector.matches_at_state(
            "button",
            &attributes(&[("disabled", "")]),
            &ancestors,
            1,
            true,
        ));
        assert_eq!(selector.specificity(), 31);
        Ok(())
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
    fn descendant_selector_does_not_match_a_child_below_its_terminal_compound() -> Result<(), String>
    {
        let selector = CssSelector::parse("#filters li").ok_or("selector must parse")?;
        let list_ancestors = [ancestor("ul", &[("id", "filters")])];
        let anchor_ancestors = [ancestor("ul", &[("id", "filters")]), ancestor("li", &[])];

        assert!(selector.matches("li", &Vec::new(), &list_ancestors));
        assert!(!selector.matches("a", &Vec::new(), &anchor_ancestors));
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
