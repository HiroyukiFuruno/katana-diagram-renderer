use super::{CssAttributeSelector, CssCompoundSelector, CssSelector};

#[path = "html_css_selector_token.rs"]
mod token;

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
    Some(CssSelector {
        compounds,
        combinators,
        inherited_from_body: selector.eq_ignore_ascii_case("body"),
    })
}

fn compound(source: &str) -> Option<CssCompoundSelector> {
    let mut remaining = source.trim();
    let tag_end = remaining
        .find(|character: char| ['.', '#', '['].contains(&character))
        .unwrap_or(remaining.len());
    let raw_tag = &remaining[..tag_end];
    if !raw_tag.is_empty() && raw_tag != "*" && !raw_tag.chars().all(is_selector_character) {
        return None;
    }
    let tag = (!raw_tag.is_empty() && raw_tag != "*").then(|| raw_tag.to_ascii_lowercase());
    remaining = &remaining[tag_end..];
    let mut compound = CssCompoundSelector {
        tag,
        classes: Vec::new(),
        id: None,
        attributes: Vec::new(),
    };
    while !remaining.is_empty() {
        remaining = parse_suffix(&mut compound, remaining)?;
    }
    (compound.tag.is_some()
        || !compound.classes.is_empty()
        || compound.id.is_some()
        || !compound.attributes.is_empty())
    .then_some(compound)
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
        _ => None,
    }
}

fn parse_identifier(source: &str) -> Option<(&str, &str)> {
    let end = source
        .find(|character: char| !is_selector_character(character))
        .unwrap_or(source.len());
    (end > 0).then_some((&source[..end], &source[end..]))
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

fn is_selector_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '_'
}
