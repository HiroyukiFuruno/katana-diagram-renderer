use super::value::{css_resolved_px, split_top_level_whitespace};
use super::{CssBoxShadow, CssStyle};

const BOX_SHADOW_LENGTH_COUNT: usize = 4;

impl CssStyle {
    pub(super) fn apply_box_shadow(&mut self, value: &str) {
        if value.trim().eq_ignore_ascii_case("none") {
            self.box_shadow = None;
            return;
        }
        self.box_shadow = CssBoxShadow::parse(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        )
        .or_else(|| self.box_shadow.clone());
    }
}

impl CssBoxShadow {
    fn parse(value: &str, em_base: f32, viewport_width: f32, viewport_height: f32) -> Option<Self> {
        let first_shadow = first_top_level_shadow(value)?;
        let (lengths, color) =
            shadow_components(first_shadow, em_base, viewport_width, viewport_height)?;
        let [offset_x, offset_y, rest @ ..] = lengths.as_slice() else {
            return None;
        };
        Some(Self {
            offset_x: *offset_x,
            offset_y: *offset_y,
            blur_radius: rest.first().copied().unwrap_or(0.0).max(0.0),
            spread_radius: rest.get(1).copied().unwrap_or(0.0),
            color: if color.is_empty() {
                "#000000".to_string()
            } else {
                color.join(" ")
            },
        })
    }
}

fn shadow_components(
    value: &str,
    em_base: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> Option<(Vec<f32>, Vec<&str>)> {
    let mut lengths = Vec::new();
    let mut color = Vec::new();
    for token in split_top_level_whitespace(value) {
        if token.eq_ignore_ascii_case("inset") {
            return None;
        }
        if lengths.len() < BOX_SHADOW_LENGTH_COUNT
            && let Some(length) =
                css_resolved_px(token, em_base, viewport_width, viewport_height, true)
        {
            lengths.push(length);
            continue;
        }
        color.push(token);
    }
    Some((lengths, color))
}

fn first_top_level_shadow(value: &str) -> Option<&str> {
    let mut depth = 0_usize;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => return Some(value[..index].trim()),
            _ => {}
        }
    }
    (depth == 0)
        .then(|| value.trim())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::CssBoxShadow;

    #[test]
    fn shadow_requires_two_offsets() {
        assert!(CssBoxShadow::parse("1px", 16.0, 100.0, 100.0).is_none());
    }

    #[test]
    fn shadow_without_color_uses_css_initial_black() {
        let shadow = CssBoxShadow::parse("1px 2px", 16.0, 100.0, 100.0);

        assert_eq!(shadow.map(|value| value.color), Some("#000000".to_string()));
    }
}
