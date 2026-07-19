use super::html_css::HtmlAttributes;

const CLASS_SPECIFICITY: u16 = 10;
const ID_SPECIFICITY: u16 = 100;

#[derive(Debug)]
pub(super) struct CssSelector {
    tag: Option<String>,
    class: Option<String>,
    id: Option<String>,
    inherited_from_body: bool,
}

impl CssSelector {
    pub(super) fn parse(raw: &str) -> Option<Self> {
        let selector = raw.trim();
        if selector.eq_ignore_ascii_case("body") {
            return Some(Self {
                tag: None,
                class: None,
                id: None,
                inherited_from_body: true,
            });
        }
        if selector.is_empty() || selector.contains(|ch: char| " >+~:[".contains(ch)) {
            return None;
        }
        let (tag, suffix) = selector_parts(selector);
        let (class, id) = suffix_parts(suffix)?;
        if tag.is_none() && class.is_none() && id.is_none() {
            return None;
        }
        Some(Self {
            tag,
            class,
            id,
            inherited_from_body: false,
        })
    }

    pub(super) fn matches(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        (!self.inherited_from_body || tag.eq_ignore_ascii_case("body"))
            && self
                .tag
                .as_ref()
                .is_none_or(|name| name.eq_ignore_ascii_case(tag))
            && self.class.as_ref().is_none_or(|class| {
                attribute_value(attributes, "class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|item| item.eq_ignore_ascii_case(class))
                })
            })
            && self.id.as_ref().is_none_or(|id| {
                attribute_value(attributes, "id")
                    .is_some_and(|value| value.eq_ignore_ascii_case(id))
            })
    }

    pub(super) fn matches_static_snapshot(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.inherited_from_body || self.matches(tag, attributes)
    }

    pub(super) fn specificity(&self) -> u16 {
        if self.inherited_from_body {
            return 1;
        }
        self.tag.iter().count() as u16
            + self.class.iter().count() as u16 * CLASS_SPECIFICITY
            + self.id.iter().count() as u16 * ID_SPECIFICITY
    }

    pub(super) fn static_snapshot_specificity(&self) -> u16 {
        if self.inherited_from_body {
            0
        } else {
            self.specificity()
        }
    }
}

fn selector_parts(selector: &str) -> (Option<String>, &str) {
    let end = selector
        .char_indices()
        .find(|(_, character)| *character == '.' || *character == '#')
        .map_or(selector.len(), |(index, _)| index);
    let tag = selector[..end].trim_matches('*').trim();
    let tag = (!tag.is_empty()).then(|| tag.to_ascii_lowercase());
    (tag, &selector[end..])
}

fn suffix_parts(mut suffix: &str) -> Option<(Option<String>, Option<String>)> {
    let mut class = None;
    let mut id = None;
    while let Some(prefix) = suffix.chars().next() {
        let next = &suffix[prefix.len_utf8()..];
        let end = next
            .find(|character: char| ['.', '#'].contains(&character))
            .unwrap_or(next.len());
        let value = &next[..end];
        if value.is_empty() || !value.chars().all(is_selector_character) {
            return None;
        }
        match prefix {
            '.' if class.replace(value.to_string()).is_none() => {}
            '#' if id.replace(value.to_string()).is_none() => {}
            _ => return None,
        }
        suffix = &next[end..];
    }
    Some((class, id))
}

fn is_selector_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '_'
}

fn attribute_value(attributes: &HtmlAttributes, name: &str) -> Option<String> {
    attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    use super::CssSelector;

    #[test]
    fn selector_parser_rejects_duplicate_and_invalid_suffixes() {
        assert!(CssSelector::parse(".first.second").is_none());
        assert!(CssSelector::parse("#first#second").is_none());
        assert!(CssSelector::parse(".bad!").is_none());
    }

    #[test]
    fn selector_parser_accepts_a_plain_tag() -> Result<(), String> {
        let selector = CssSelector::parse("article").ok_or("tag selector must parse")?;

        assert!(selector.matches("article", &Vec::new()));
        Ok(())
    }

    #[test]
    fn body_selector_matches_only_the_body_in_interactive_runtime() -> Result<(), String> {
        let specificity = CssSelector::parse("body").map(|selector| selector.specificity());
        let selector = CssSelector::parse("body").ok_or("body selector must parse")?;
        let attributes = Vec::new();

        assert_eq!(specificity, Some(1));
        assert!(selector.matches("body", &attributes));
        assert!(!selector.matches("p", &attributes));
        assert!(selector.matches_static_snapshot("p", &attributes));
        assert_eq!(selector.static_snapshot_specificity(), 0);
        Ok(())
    }

    #[test]
    fn selector_matching_rejects_tag_class_and_id_mismatches() {
        let attributes = vec![
            ("class".to_string(), "primary selected".to_string()),
            ("id".to_string(), "submit".to_string()),
        ];

        assert!(!selector(Some("button"), None, None).matches("a", &attributes));
        assert!(!selector(None, Some("missing"), None).matches("button", &attributes));
        assert!(!selector(None, None, Some("cancel")).matches("button", &attributes));
        assert!(
            selector(Some("button"), Some("primary"), Some("submit"))
                .matches("button", &attributes)
        );
    }

    fn selector(tag: Option<&str>, class: Option<&str>, id: Option<&str>) -> CssSelector {
        CssSelector {
            tag: tag.map(str::to_string),
            class: class.map(str::to_string),
            id: id.map(str::to_string),
            inherited_from_body: false,
        }
    }
}
