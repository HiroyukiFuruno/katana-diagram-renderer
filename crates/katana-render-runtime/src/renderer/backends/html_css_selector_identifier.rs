pub(super) fn parse_identifier(source: &str) -> Option<(&str, &str)> {
    let end = source
        .find(|character: char| !is_selector_character(character))
        .unwrap_or(source.len());
    (end > 0).then_some((&source[..end], &source[end..]))
}

pub(super) fn is_selector_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '_'
}
