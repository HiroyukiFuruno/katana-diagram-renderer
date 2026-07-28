use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::style::value::{css_font_size, css_font_weight};

type FontShorthandSize<'a> = (usize, &'a str, Option<&'a str>, usize);

const MIN_FONT_WEIGHT: u16 = 1;
const MAX_FONT_WEIGHT: u16 = 1_000;
const SIZE_LINE_HEIGHT_TOKEN_COUNT: usize = 3;

pub(super) struct FontShorthandParser;

impl FontShorthandParser {
    pub(super) fn size<'a>(
        tokens: &'a [&'a str],
        style: &CssStyle,
    ) -> Option<FontShorthandSize<'a>> {
        for (index, token) in tokens.iter().enumerate() {
            let (size, attached_line_height) = parse_font_size_token(token);
            if !is_font_size_candidate(size, style) {
                continue;
            }
            if let Some(line_height) = attached_line_height {
                return (!line_height.is_empty()).then_some((
                    index,
                    size,
                    Some(line_height),
                    index + 1,
                ));
            }
            if tokens.get(index + 1) == Some(&"/") {
                let line_height = *tokens.get(index + 2)?;
                return Some((
                    index,
                    size,
                    Some(line_height),
                    index + SIZE_LINE_HEIGHT_TOKEN_COUNT,
                ));
            }
            return Some((index, size, None, index + 1));
        }
        None
    }

    pub(super) fn style(values: &[&str], base_weight: u16) -> Option<(bool, u16)> {
        let mut italic = false;
        let mut font_weight = base_weight;
        for value in values {
            let lower = value.to_ascii_lowercase();
            match lower.as_str() {
                "normal" => {}
                "italic" | "oblique" => italic = true,
                _ => {
                    let parsed = css_font_weight(value, font_weight)?;
                    font_weight = parsed;
                }
            }
        }
        Some((italic, font_weight))
    }
}

fn is_font_size_candidate(size: &str, style: &CssStyle) -> bool {
    if size
        .parse::<u16>()
        .ok()
        .is_some_and(|weight| (MIN_FONT_WEIGHT..=MAX_FONT_WEIGHT).contains(&weight))
    {
        return false;
    }
    css_font_size(
        size,
        style.font_size,
        style.viewport_width,
        style.viewport_height,
    )
    .is_some()
}

fn parse_font_size_token(value: &str) -> (&str, Option<&str>) {
    value
        .split_once('/')
        .map_or((value, None), |(size, line_height)| {
            (size, Some(line_height))
        })
}

#[cfg(test)]
mod tests {
    use super::FontShorthandParser;
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn font_shorthand_size_extracts_attach_line_height_from_same_token() {
        let style = CssStyle::browser_default();
        let values = ["14px/2", "serif"];

        assert_eq!(
            FontShorthandParser::size(&values, &style),
            Some((0, "14px", Some("2"), 1))
        );
    }

    #[test]
    fn font_shorthand_size_extracts_line_height_from_slash_separator() {
        let style = CssStyle::browser_default();
        let values = ["italic", "14px", "/", "1.5", "serif"];

        assert_eq!(
            FontShorthandParser::size(&values, &style),
            Some((1, "14px", Some("1.5"), 4))
        );
    }
}
