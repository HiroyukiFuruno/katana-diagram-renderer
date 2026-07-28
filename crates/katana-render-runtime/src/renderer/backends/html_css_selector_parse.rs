use super::{CssAttributeSelector, CssCompoundSelector, CssPseudoElement, CssSelector};

#[path = "html_css_selector_identifier.rs"]
mod identifier;
#[path = "html_css_selector_token.rs"]
mod token;

use identifier::{is_selector_character, parse_identifier};
use token::{SelectorToken, selector_tokens};

pub(super) fn selector(raw: &str) -> Option<CssSelector> {
    let selector = raw.trim();
    let tokens = selector_tokens(selector)?;
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    for token in tokens {
        match token {
            SelectorToken::Compound(value) => compounds.push(compound(&value)?),
            SelectorToken::Combinator(value) => combinators.push(value),
        }
    }
    if compounds
        .iter()
        .take(compounds.len().saturating_sub(1))
        .any(|compound| compound.pseudo_element.is_some())
    {
        return None;
    }
    Some(CssSelector {
        compounds,
        combinators,
        inherited_from_body: selector.eq_ignore_ascii_case("body"),
    })
}

fn compound(source: &str) -> Option<CssCompoundSelector> {
    let source = source.trim();
    let (tag, mut remaining) = parse_tag(source)?;
    let mut compound = empty_compound(tag);
    while !remaining.is_empty() {
        remaining = parse_suffix(&mut compound, remaining)?;
    }
    (source == "*" || has_selector_part(&compound)).then_some(compound)
}

fn parse_tag(source: &str) -> Option<(Option<String>, &str)> {
    let tag_end = source
        .find(|character: char| ['.', '#', '[', ':'].contains(&character))
        .unwrap_or(source.len());
    let raw_tag = &source[..tag_end];
    if !raw_tag.is_empty() && raw_tag != "*" && !raw_tag.chars().all(is_selector_character) {
        return None;
    }
    let tag = (!raw_tag.is_empty() && raw_tag != "*").then(|| raw_tag.to_ascii_lowercase());
    Some((tag, &source[tag_end..]))
}

fn empty_compound(tag: Option<String>) -> CssCompoundSelector {
    CssCompoundSelector {
        tag,
        classes: Vec::new(),
        id: None,
        attributes: Vec::new(),
        root: false,
        hovered: false,
        disabled: false,
        not_disabled: false,
        checked: false,
        nth_child: None,
        pseudo_element: None,
    }
}

fn has_selector_part(compound: &CssCompoundSelector) -> bool {
    compound.tag.is_some()
        || !compound.classes.is_empty()
        || compound.id.is_some()
        || !compound.attributes.is_empty()
        || compound.root
        || compound.hovered
        || compound.disabled
        || compound.not_disabled
        || compound.checked
        || compound.nth_child.is_some()
        || compound.pseudo_element.is_some()
}

fn parse_suffix<'a>(compound: &mut CssCompoundSelector, source: &'a str) -> Option<&'a str> {
    match source.chars().next()? {
        '.' => parse_identifier(&source[1..]).map(|(value, rest)| {
            compound.classes.push(value.to_string());
            rest
        }),
        '#' => parse_identifier(&source[1..]).and_then(|(value, rest)| {
            compound
                .id
                .replace(value.to_string())
                .is_none()
                .then_some(rest)
        }),
        '[' => parse_attribute(source).map(|(attribute, rest)| {
            compound.attributes.push(attribute);
            rest
        }),
        ':' => parse_pseudo_class(compound, source),
        _ => None,
    }
}

fn parse_pseudo_class<'a>(compound: &mut CssCompoundSelector, source: &'a str) -> Option<&'a str> {
    if source.starts_with("::before") {
        return parse_pseudo_element(compound, source, "::before", CssPseudoElement::Before);
    }
    if source.starts_with("::after") {
        return parse_pseudo_element(compound, source, "::after", CssPseudoElement::After);
    }
    if source.starts_with(":before") {
        return parse_pseudo_element(compound, source, ":before", CssPseudoElement::Before);
    }
    if source.starts_with(":after") {
        return parse_pseudo_element(compound, source, ":after", CssPseudoElement::After);
    }
    if source.starts_with(":root") {
        return parse_flag(&mut compound.root, source, ":root");
    }
    if source.starts_with(":nth-child(") {
        return parse_nth_child(compound, source);
    }
    if source.starts_with(":hover") {
        return parse_flag(&mut compound.hovered, source, ":hover");
    }
    if source.starts_with(":not(:disabled)") {
        return parse_flag(&mut compound.not_disabled, source, ":not(:disabled)");
    }
    if source.starts_with(":checked") {
        return parse_flag(&mut compound.checked, source, ":checked");
    }
    parse_flag(&mut compound.disabled, source, ":disabled")
}

fn parse_pseudo_element<'a>(
    compound: &mut CssCompoundSelector,
    source: &'a str,
    prefix: &str,
    pseudo_element: CssPseudoElement,
) -> Option<&'a str> {
    let rest = source.strip_prefix(prefix)?;
    compound.pseudo_element.is_none().then(|| {
        compound.pseudo_element = Some(pseudo_element);
        rest
    })
}

fn parse_flag<'a>(flag: &mut bool, source: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = source.strip_prefix(prefix)?;
    (!*flag).then(|| {
        *flag = true;
        rest
    })
}

fn parse_nth_child<'a>(compound: &mut CssCompoundSelector, source: &'a str) -> Option<&'a str> {
    let rest = source.strip_prefix(":nth-child(")?;
    let end = rest.find(')')?;
    if compound.nth_child.is_some() {
        return None;
    }
    compound.nth_child = Some(super::CssNthExpression::parse(&rest[..end])?);
    Some(&rest[end + 1..])
}

fn parse_attribute(source: &str) -> Option<(CssAttributeSelector, &str)> {
    let end = source.find(']')?;
    let content = source[1..end].trim();
    let (name, value) = match content.split_once('=') {
        Some((name, value)) => (
            name.trim(),
            Some(value.trim().trim_matches(['\'', '"']).to_string()),
        ),
        None => (content, None),
    };
    if name.is_empty() || !name.chars().all(is_selector_character) {
        return None;
    }
    Some((
        CssAttributeSelector {
            name: name.to_ascii_lowercase(),
            value,
        },
        &source[end + 1..],
    ))
}

#[cfg(test)]
mod tests {
    use super::{CssSelector, selector};

    fn must_parse(source: &str) -> CssSelector {
        let parsed = selector(source);
        assert!(parsed.is_some(), "{source} selector should parse");
        let mut selectors = parsed.into_iter().collect::<Vec<_>>();
        selectors.remove(0)
    }

    #[test]
    fn selector_parses_wildcard_selector() {
        assert!(selector("*").is_some());
    }

    #[test]
    fn selector_rejects_no_selector_parts_without_wildcard() {
        assert!(selector("").is_none());
    }

    #[test]
    fn selector_parses_pseudo_element() {
        assert!(selector("div:before").is_some());
    }

    #[test]
    fn selector_parses_pseudo_flags_without_tag() {
        let pseudo = must_parse(":before");
        let compound = &pseudo.compounds[0];
        assert!(compound.pseudo_element.is_some());
    }

    #[test]
    fn selector_parses_state_pseudo_classes() {
        let disabled = must_parse(":disabled");
        assert!(disabled.compounds[0].disabled);

        let hover = must_parse(":hover");
        assert!(hover.compounds[0].hovered);
    }
}
