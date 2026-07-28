use super::GradientStop;

const TOP_RIGHT_DEGREES: f32 = 45.0;
const RIGHT_DEGREES: f32 = 90.0;
const BOTTOM_RIGHT_DEGREES: f32 = 135.0;
const BOTTOM_DEGREES: f32 = 180.0;
const BOTTOM_LEFT_DEGREES: f32 = 225.0;
const LEFT_DEGREES: f32 = 270.0;
const TOP_LEFT_DEGREES: f32 = 315.0;
const PERCENT_MAXIMUM: f32 = 100.0;

pub(super) fn gradient_direction(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(degrees) = value.strip_suffix("deg") {
        return degrees
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite());
    }
    match value.as_str() {
        "to top" => Some(0.0),
        "to top right" | "to right top" => Some(TOP_RIGHT_DEGREES),
        "to right" => Some(RIGHT_DEGREES),
        "to bottom right" | "to right bottom" => Some(BOTTOM_RIGHT_DEGREES),
        "to bottom" => Some(BOTTOM_DEGREES),
        "to bottom left" | "to left bottom" => Some(BOTTOM_LEFT_DEGREES),
        "to left" => Some(LEFT_DEGREES),
        "to top left" | "to left top" => Some(TOP_LEFT_DEGREES),
        _ => None,
    }
}

pub(super) fn distribute_offsets(stops: &mut [GradientStop]) {
    if stops.first().is_some_and(|stop| stop.offset.is_none()) {
        stops[0].offset = Some(0.0);
    }
    let last = stops.len() - 1;
    if stops[last].offset.is_none() {
        stops[last].offset = Some(PERCENT_MAXIMUM);
    }
    let mut start = 0;
    while start < last {
        let mut end = start + 1;
        while stops[end].offset.is_none() {
            end += 1;
        }
        let start_offset = stops[start].offset.unwrap_or(0.0);
        let end_offset = stops[end].offset.unwrap_or(start_offset).max(start_offset);
        let span = (end - start) as f32;
        for (step, stop) in stops[start + 1..end].iter_mut().enumerate() {
            stop.offset =
                Some(start_offset + (end_offset - start_offset) * (step + 1) as f32 / span);
        }
        start = end;
    }
}

pub(super) fn split_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut depth = 0_u32;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if character == separator && depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}

pub(super) fn split_top_level_whitespace(value: &str) -> Vec<&str> {
    let mut depth = 0_u32;
    let mut start = None;
    let mut parts = Vec::new();
    for (index, character) in value.char_indices() {
        match character {
            '(' => {
                depth += 1;
                start.get_or_insert(index);
            }
            ')' => depth = depth.saturating_sub(1),
            _ if character.is_whitespace() && depth == 0 => {
                if let Some(token_start) = start.take() {
                    parts.push(value[token_start..index].trim());
                }
            }
            _ => {
                start.get_or_insert(index);
            }
        }
    }
    if let Some(token_start) = start {
        parts.push(value[token_start..].trim());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::{gradient_direction, split_top_level_whitespace};

    #[test]
    fn gradient_direction_supports_every_corner_and_cardinal_alias() {
        let cases = [
            ("to bottom right", 135.0),
            ("to bottom", 180.0),
            ("to bottom left", 225.0),
            ("to left", 270.0),
            ("to top left", 315.0),
        ];

        for (value, expected) in cases {
            assert_eq!(gradient_direction(value), Some(expected));
        }
    }

    #[test]
    fn top_level_whitespace_split_preserves_nested_parentheses() {
        assert_eq!(
            split_top_level_whitespace("url(image.svg) 10px 20px"),
            vec!["url(image.svg)", "10px", "20px"]
        );
    }

    #[test]
    fn top_level_whitespace_split_handles_single_token_without_trailing_whitespace() {
        assert_eq!(split_top_level_whitespace("45deg"), vec!["45deg"]);
    }

    #[test]
    fn top_level_whitespace_split_ignores_surrounding_whitespace() {
        assert_eq!(split_top_level_whitespace("  red  "), vec!["red"]);
    }
}
