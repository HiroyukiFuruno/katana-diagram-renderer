use super::super::{CssGridTrack, CssGridTrackBreadth};
use super::{css_number, css_relative_px};

const MAX_GRID_TRACKS: usize = 64;

pub(in super::super) fn grid_tracks(value: &str, em_base: f32) -> Option<Vec<CssGridTrack>> {
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
    if let Some(arguments) = value
        .strip_prefix("minmax(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (min, max) = split_top_level_once(arguments, ',')?;
        let min = parse_grid_track_breadth(min, em_base)?;
        let max = parse_grid_track_breadth(max, em_base)?;
        if matches!(min, CssGridTrackBreadth::Fraction(_)) {
            return None;
        }
        return Some(CssGridTrack::MinMax { min, max });
    }
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

fn parse_grid_track_breadth(value: &str, em_base: f32) -> Option<CssGridTrackBreadth> {
    match parse_grid_track(value, em_base)? {
        CssGridTrack::Length(value) => Some(CssGridTrackBreadth::Length(value)),
        CssGridTrack::Percent(value) => Some(CssGridTrackBreadth::Percent(value)),
        CssGridTrack::Fraction(value) => Some(CssGridTrackBreadth::Fraction(value)),
        CssGridTrack::Auto => Some(CssGridTrackBreadth::Auto),
        CssGridTrack::MinContent => Some(CssGridTrackBreadth::MinContent),
        CssGridTrack::MaxContent => Some(CssGridTrackBreadth::MaxContent),
        CssGridTrack::MinMax { .. } => None,
    }
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

#[cfg(test)]
mod tests {
    use super::{
        CssGridTrack, CssGridTrackBreadth, MAX_GRID_TRACKS, grid_tracks, split_top_level_once,
        split_top_level_whitespace,
    };

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

    #[test]
    fn grid_parser_preserves_minmax_track_constraints() {
        assert_eq!(
            grid_tracks("minmax(240px, 1fr) minmax(min-content, 40%)", 16.0),
            Some(vec![
                CssGridTrack::MinMax {
                    min: CssGridTrackBreadth::Length(240.0),
                    max: CssGridTrackBreadth::Fraction(1.0),
                },
                CssGridTrack::MinMax {
                    min: CssGridTrackBreadth::MinContent,
                    max: CssGridTrackBreadth::Percent(0.4),
                },
            ])
        );
        assert!(grid_tracks("minmax(1fr, 100px)", 16.0).is_none());
        assert!(grid_tracks("minmax(10px)", 16.0).is_none());
        assert!(grid_tracks("minmax(10px, minmax(20px, 1fr))", 16.0).is_none());
        assert_eq!(
            grid_tracks("minmax(auto, max-content)", 16.0),
            Some(vec![CssGridTrack::MinMax {
                min: CssGridTrackBreadth::Auto,
                max: CssGridTrackBreadth::MaxContent,
            }])
        );
    }
}
