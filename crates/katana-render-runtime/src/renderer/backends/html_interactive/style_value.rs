use super::super::constants::{BOLD_FONT_WEIGHT_MINIMUM, DEFAULT_FONT_SIZE, LINE_HEIGHT_FACTOR};
use super::{CssGridTrack, CssLength};

const CSS_BOX_SIDE_COUNT: usize = 4;
const MAX_GRID_TRACKS: usize = 64;

impl CssLength {
    pub(super) fn parse(value: &str, em_base: f32) -> Option<Self> {
        let value = value.trim();
        if let Some(percent) = value.strip_suffix('%') {
            return css_number(percent).map(|value| Self::Percent(value / 100.0));
        }
        css_relative_px(value, em_base, false).map(Self::Px)
    }

    pub(super) fn resolve(self, available: f32) -> f32 {
        match self {
            Self::Px(value) => value,
            Self::Percent(value) => available * value,
        }
    }
}

pub(super) fn box_sides(
    value: &str,
    em_base: f32,
    signed: bool,
) -> Option<[f32; CSS_BOX_SIDE_COUNT]> {
    let values = value
        .split_whitespace()
        .map(|value| css_relative_px(value, em_base, signed))
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [all] => Some([*all; CSS_BOX_SIDE_COUNT]),
        [vertical, horizontal] => Some([*vertical, *horizontal, *vertical, *horizontal]),
        [top, horizontal, bottom] => Some([*top, *horizontal, *bottom, *horizontal]),
        [top, right, bottom, left] => Some([*top, *right, *bottom, *left]),
        _ => None,
    }
}

pub(super) fn css_relative_px(value: &str, em_base: f32, signed: bool) -> Option<f32> {
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

pub(super) fn css_font_size(value: &str, inherited: f32) -> Option<f32> {
    if let Some(value) = value.trim().strip_suffix('%') {
        return css_number(value).map(|value| inherited * value / 100.0);
    }
    css_relative_px(value, inherited, false)
}

pub(super) fn css_line_height(value: &str, font_size: f32) -> Option<(f32, Option<f32>)> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("normal") {
        return Some((font_size * LINE_HEIGHT_FACTOR, Some(LINE_HEIGHT_FACTOR)));
    }
    if let Some(factor) = css_number(value) {
        return Some((factor * font_size, Some(factor)));
    }
    if let Some(value) = value.strip_suffix('%') {
        return css_number(value).map(|value| (font_size * value / 100.0, None));
    }
    css_relative_px(value, font_size, false).map(|value| (value, None))
}

pub(super) fn css_number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

pub(super) fn grid_tracks(value: &str, em_base: f32) -> Option<Vec<CssGridTrack>> {
    let value = value.trim();
    if let Some(arguments) = value
        .strip_prefix("repeat(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (count, tracks) = split_top_level_once(arguments, ',')?;
        let count = count.trim().parse::<usize>().ok()?;
        let repeated = parse_grid_track_list(tracks, em_base)?;
        let total = count.checked_mul(repeated.len())?;
        if count == 0 || total > MAX_GRID_TRACKS {
            return None;
        }
        return Some((0..count).flat_map(|_| repeated.iter().copied()).collect());
    }
    parse_grid_track_list(value, em_base)
}

fn parse_grid_track_list(value: &str, em_base: f32) -> Option<Vec<CssGridTrack>> {
    let tokens = split_top_level_whitespace(value);
    if tokens.is_empty() || tokens.len() > MAX_GRID_TRACKS {
        return None;
    }
    tokens
        .into_iter()
        .map(|token| parse_grid_track(token, em_base))
        .collect()
}

fn parse_grid_track(value: &str, em_base: f32) -> Option<CssGridTrack> {
    let value = value.trim();
    match value.to_ascii_lowercase().as_str() {
        "auto" => return Some(CssGridTrack::Auto),
        "min-content" => return Some(CssGridTrack::MinContent),
        "max-content" => return Some(CssGridTrack::MaxContent),
        _ => {}
    }
    if let Some(value) = value.strip_suffix("fr") {
        return css_number(value).map(CssGridTrack::Fraction);
    }
    if let Some(value) = value.strip_suffix('%') {
        return css_number(value).map(|value| CssGridTrack::Percent(value / 100.0));
    }
    css_relative_px(value, em_base, false).map(CssGridTrack::Length)
}

fn split_top_level_once(value: &str, separator: char) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            _ if character == separator && depth == 0 => {
                return Some((&value[..index], &value[index + character.len_utf8()..]));
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_whitespace(value: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (index, character) in value.char_indices() {
        if character.is_whitespace() && depth == 0 {
            if let Some(token_start) = start.take() {
                tokens.push(&value[token_start..index]);
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
        tokens.push(&value[token_start..]);
    }
    tokens
}

pub(super) fn is_bold(value: &str) -> bool {
    value.eq_ignore_ascii_case("bold")
        || value
            .parse::<u16>()
            .is_ok_and(|weight| weight >= BOLD_FONT_WEIGHT_MINIMUM)
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
    use super::{MAX_GRID_TRACKS, grid_tracks, split_top_level_once, split_top_level_whitespace};

    #[test]
    fn grid_parser_rejects_empty_invalid_and_oversized_repeat_values() {
        assert!(grid_tracks("", 16.0).is_none());
        assert!(grid_tracks("repeat(0, 1fr)", 16.0).is_none());
        let oversized = format!("repeat({}, 1fr)", MAX_GRID_TRACKS + 1);
        assert!(grid_tracks(&oversized, 16.0).is_none());
        assert!(split_top_level_once(")", ',').is_none());
        assert!(split_top_level_once("no separator", ',').is_none());
    }

    #[test]
    fn grid_tokenizer_keeps_nested_function_whitespace_together() {
        assert_eq!(
            split_top_level_once("fn(a,b), tail", ','),
            Some(("fn(a,b)", " tail"))
        );
        assert_eq!(
            split_top_level_whitespace(" repeat(2, 1fr) auto "),
            ["repeat(2, 1fr)", "auto"]
        );
    }
}
