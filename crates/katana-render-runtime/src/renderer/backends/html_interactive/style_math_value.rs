use super::super::super::constants::DEFAULT_FONT_SIZE;

pub(in crate::renderer::backends::html_interactive::style) fn css_relative_px(
    value: &str,
    em_base: f32,
    signed: bool,
) -> Option<f32> {
    let value = value.trim();
    let parsed = if let Some(value) = value.strip_suffix("rem") {
        css_scalar(value).map(|value| value * DEFAULT_FONT_SIZE)
    } else if let Some(value) = value.strip_suffix("em") {
        css_scalar(value).map(|value| value * em_base)
    } else {
        css_scalar(value.strip_suffix("px").unwrap_or(value))
    }?;
    (signed || parsed >= 0.0).then_some(parsed)
}

pub(in crate::renderer::backends::html_interactive::style) fn css_resolved_px(
    value: &str,
    em_base: f32,
    viewport_width: f32,
    viewport_height: f32,
    signed: bool,
) -> Option<f32> {
    let resolved = css_math_px(value.trim(), em_base, viewport_width, viewport_height)?;
    (signed || resolved >= 0.0).then_some(resolved)
}

fn css_math_px(
    value: &str,
    em_base: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<f32> {
    if let Some(resolved) = css_named_function_px(value, em_base, viewport_width, viewport_height) {
        return resolved;
    }
    css_viewport_px(value, viewport_width, viewport_height)
        .or_else(|| css_relative_px(value.trim(), em_base, true))
}

fn css_named_function_px(
    value: &str,
    em_base: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<Option<f32>> {
    for name in ["min", "max", "clamp"] {
        let Some(arguments) = css_function_arguments(value, name) else {
            continue;
        };
        let values = arguments
            .into_iter()
            .map(|argument| css_math_px(argument, em_base, viewport_width, viewport_height))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values else {
            return Some(None);
        };
        let resolved = match (name, values.as_slice()) {
            ("min", [first, rest @ ..]) => Some(rest.iter().copied().fold(*first, f32::min)),
            ("max", [first, rest @ ..]) => Some(rest.iter().copied().fold(*first, f32::max)),
            ("clamp", [minimum, preferred, maximum]) => {
                Some(preferred.max(*minimum).min(maximum.max(*minimum)))
            }
            _ => None,
        };
        return Some(resolved);
    }
    None
}

fn css_viewport_px(value: &str, viewport_width: f32, viewport_height: f32) -> Option<f32> {
    let value = value.trim();
    let viewport = [
        ("vmin", viewport_width.min(viewport_height)),
        ("vmax", viewport_width.max(viewport_height)),
        ("vw", viewport_width),
        ("vh", viewport_height),
    ];
    for (suffix, basis) in viewport {
        if let Some(value) = value.strip_suffix(suffix) {
            return css_scalar(value).map(|value| basis * value / 100.0);
        }
    }
    None
}

fn css_function_arguments<'a>(value: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let open = value.find('(')?;
    if !value[..open].trim().eq_ignore_ascii_case(name) || !value.ends_with(')') {
        return None;
    }
    split_top_level_commas(&value[open + 1..value.len() - 1])
}

fn split_top_level_commas(value: &str) -> Option<Vec<&str>> {
    let mut depth = 0_usize;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                let part = value[start..index].trim();
                if part.is_empty() {
                    return None;
                }
                parts.push(part);
                start = index + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let last = value[start..].trim();
    if last.is_empty() {
        return None;
    }
    parts.push(last);
    Some(parts)
}

pub(in crate::renderer::backends::html_interactive::style) fn split_top_level_whitespace(
    value: &str,
) -> Vec<&str> {
    let mut depth = 0_usize;
    let mut start = None;
    let mut parts = Vec::new();
    for (index, character) in value.char_indices() {
        if character.is_whitespace() && depth == 0 {
            if let Some(token_start) = start.take() {
                parts.push(&value[token_start..index]);
            }
            continue;
        }
        start.get_or_insert(index);
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if let Some(token_start) = start {
        parts.push(&value[token_start..]);
    }
    parts
}

fn css_scalar(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::{css_resolved_px, split_top_level_commas, split_top_level_whitespace};

    #[test]
    fn math_function_rejects_unresolvable_nested_values() {
        assert!(css_resolved_px("min(1px, invalid)", 16.0, 100.0, 50.0, false).is_none());
    }

    #[test]
    fn top_level_comma_split_rejects_empty_and_unbalanced_arguments() {
        assert!(split_top_level_commas(", 1px").is_none());
        assert!(split_top_level_commas("min(1px, 2px").is_none());
        assert!(split_top_level_commas("1px,").is_none());
    }

    #[test]
    fn top_level_whitespace_split_preserves_nested_parentheses() {
        assert_eq!(
            split_top_level_whitespace("calc(1px + 2px) 3px"),
            vec!["calc(1px + 2px)", "3px"]
        );
    }

    #[test]
    fn top_level_whitespace_split_returns_final_token_without_trailing_whitespace() {
        assert_eq!(split_top_level_whitespace("  5px  "), vec!["5px"]);
    }
}
