use super::value::{css_number, split_top_level_whitespace};
use super::{CssLength, CssStyle};

impl CssStyle {
    pub(super) fn apply_flex(&mut self, value: &str) {
        let value = value.trim();
        let components = match value.to_ascii_lowercase().as_str() {
            "none" => Some((0.0, 0.0, None)),
            "auto" => Some((1.0, 1.0, None)),
            "initial" => Some((0.0, 1.0, None)),
            _ => self.parse_flex_components(value),
        };
        let Some((grow, shrink, basis)) = components else {
            return;
        };
        self.flex_grow = grow;
        self.flex_shrink = shrink;
        self.flex_basis = basis;
    }

    pub(super) fn apply_flex_basis(&mut self, value: &str) {
        if matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "auto" | "content"
        ) {
            self.flex_basis = None;
        } else if let Some(basis) = self.parse_flex_length(value) {
            self.flex_basis = Some(basis);
        }
    }

    fn parse_flex_components(&self, value: &str) -> Option<(f32, f32, Option<CssLength>)> {
        let tokens = split_top_level_whitespace(value);
        let grow = css_number(tokens.first()?)?;
        let mut shrink = 1.0;
        let mut basis = Some(CssLength::Percent(0.0));
        for token in &tokens[1..] {
            if let Some(number) = css_number(token)
                && basis == Some(CssLength::Percent(0.0))
            {
                shrink = number;
                continue;
            }
            basis = if matches!(token.to_ascii_lowercase().as_str(), "auto" | "content") {
                None
            } else {
                Some(self.parse_flex_length(token)?)
            };
        }
        Some((grow, shrink, basis))
    }

    fn parse_flex_length(&self, value: &str) -> Option<CssLength> {
        CssLength::parse(
            value,
            self.font_size,
            self.viewport_width,
            self.viewport_height,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CssLength, CssStyle};

    #[test]
    fn invalid_flex_shorthand_preserves_existing_values() {
        let mut style = CssStyle::browser_default();
        style.flex_grow = 2.0;

        style.apply_flex("invalid");

        assert_eq!(style.flex_grow, 2.0);
    }

    #[test]
    fn flex_basis_handles_auto_and_explicit_lengths() {
        let mut style = CssStyle::browser_default();
        style.flex_basis = Some(CssLength::Px(4.0));

        style.apply_flex_basis("auto");
        assert!(style.flex_basis.is_none());

        style.apply_flex_basis("12px");
        assert_eq!(style.flex_basis, Some(CssLength::Px(12.0)));

        style.apply_flex_basis("invalid");
        assert_eq!(style.flex_basis, Some(CssLength::Px(12.0)));
    }

    #[test]
    fn flex_shorthand_accepts_content_basis() {
        let mut style = CssStyle::browser_default();

        style.apply_flex("2 content");

        assert_eq!((style.flex_grow, style.flex_shrink), (2.0, 1.0));
        assert!(style.flex_basis.is_none());
    }

    #[test]
    fn flex_basis_accepts_percentage_length() {
        let mut style = CssStyle::browser_default();
        style.flex_basis = Some(CssLength::Px(1.0));

        style.apply_flex_basis("50%");

        assert_eq!(style.flex_basis, Some(CssLength::Percent(0.5)));
    }
}
