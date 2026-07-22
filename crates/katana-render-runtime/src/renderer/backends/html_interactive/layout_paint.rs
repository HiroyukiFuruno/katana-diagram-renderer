use super::constants::TEXT_CHARACTER_WIDTH_FACTOR;
use super::layout::HtmlLayoutRenderer;
use super::style::{CssStyle, CssTextAlign};
use super::svg::{box_svg, escape_xml};

impl HtmlLayoutRenderer {
    pub(super) fn paint_box(&mut self, x: f32, y: f32, width: f32, height: f32, style: &CssStyle) {
        self.svg
            .push_str(&box_svg(x, y - self.scroll_y, width, height, style));
    }

    pub(super) fn insert_box(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        style: &CssStyle,
    ) {
        self.svg
            .insert_str(index, &box_svg(x, y - self.scroll_y, width, height, style));
    }

    pub(super) fn paint_text_lines(
        &mut self,
        lines: &[String],
        x: f32,
        width: f32,
        baseline_y: f32,
        style: &CssStyle,
    ) {
        for (index, line) in lines.iter().enumerate() {
            self.paint_text_line(
                line,
                aligned_text_x(line, x, width, style),
                baseline_y + index as f32 * style.line_height,
                style,
            );
        }
    }

    fn paint_text_line(&mut self, line: &str, x: f32, baseline_y: f32, style: &CssStyle) {
        let y = baseline_y - self.scroll_y;
        self.svg.push_str(&format!(
            r#"<text x="{x}" y="{y}" font-family="{}" font-size="{}" fill="{}"{}{}{}{}>{}</text>"#,
            escape_xml(&style.font_family),
            style.font_size,
            escape_xml(&style.color),
            font_weight(style),
            font_style(style),
            text_decoration(style),
            letter_spacing(style),
            escape_xml(line)
        ));
    }

    pub(super) fn clip_painted_range(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    ) {
        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;
        self.svg.push_str("</g>");
        self.svg.insert_str(
            index,
            &format!(
                r#"<defs><clipPath id="krr-clip-{clip_id}"><rect x="{x}" y="{}" width="{width}" height="{height}" rx="{radius}" ry="{radius}"/></clipPath></defs><g clip-path="url(#krr-clip-{clip_id})">"#,
                y - self.scroll_y
            ),
        );
    }
}

fn aligned_text_x(line: &str, x: f32, width: f32, style: &CssStyle) -> f32 {
    let characters = line.chars().count() as f32;
    let text_width = characters * style.font_size * TEXT_CHARACTER_WIDTH_FACTOR
        + (characters - 1.0).max(0.0) * style.letter_spacing;
    match style.text_align {
        CssTextAlign::Start => x,
        CssTextAlign::Center => x + (width - text_width).max(0.0) / 2.0,
        CssTextAlign::End => x + (width - text_width).max(0.0),
    }
}

fn font_weight(style: &CssStyle) -> &'static str {
    if style.bold {
        " font-weight=\"700\""
    } else {
        ""
    }
}

fn font_style(style: &CssStyle) -> &'static str {
    if style.italic {
        " font-style=\"italic\""
    } else {
        ""
    }
}

fn text_decoration(style: &CssStyle) -> &'static str {
    if style.underline {
        " text-decoration=\"underline\""
    } else {
        ""
    }
}

fn letter_spacing(style: &CssStyle) -> String {
    if style.letter_spacing == 0.0 {
        String::new()
    } else {
        format!(" letter-spacing=\"{}\"", style.letter_spacing)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        constants::TEXT_CHARACTER_WIDTH_FACTOR,
        layout::HtmlLayoutRenderer,
        style::{CssStyle, CssTextAlign},
    };
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_interactive::layout_paint::aligned_text_x;
    use std::collections::HashMap;

    #[test]
    fn text_alignment_and_typography_attributes_are_written_to_svg() {
        let viewport = HtmlBrowserViewport {
            width: 400,
            height: 300,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let lines = vec!["abc".to_string(), "x".to_string()];

        let mut end_style = CssStyle::browser_default();
        end_style.text_align = CssTextAlign::End;
        end_style.letter_spacing = 1.0;
        end_style.bold = true;
        end_style.italic = true;
        end_style.underline = true;
        renderer.paint_text_lines(&lines, 10.0, 120.0, 24.0, &end_style);

        let expected_x = aligned_text_x("abc", 10.0, 120.0, &end_style);
        let text_attr = first_text_x(&renderer.svg);

        assert_eq!(text_attr, Some(expected_x));
        assert!(renderer.svg.contains(" font-weight=\"700\""));
        assert!(renderer.svg.contains(" font-style=\"italic\""));
        assert!(renderer.svg.contains(" text-decoration=\"underline\""));
        assert!(renderer.svg.contains(" letter-spacing=\"1\""));
    }

    #[test]
    fn alignment_center_uses_width_offset_and_start_keeps_origin() {
        let viewport = HtmlBrowserViewport {
            width: 320,
            height: 240,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
        let mut style = CssStyle::browser_default();
        style.text_align = CssTextAlign::Center;
        let width = 100.0;
        let expected = aligned_text_x("ab", 5.0, width, &style);
        let text_width = 2.0 * style.font_size * TEXT_CHARACTER_WIDTH_FACTOR;
        assert_eq!(expected, 5.0 + (width - text_width).max(0.0_f32) / 2.0);
        renderer.paint_text_lines(&["ab".to_string()], 5.0, width, 24.0, &style);
        assert!(renderer.svg.contains("<text"));

        let mut start_style = CssStyle::browser_default();
        start_style.text_align = CssTextAlign::Start;
        let expected = aligned_text_x("ab", 5.0, width, &start_style);
        assert_eq!(expected, 5.0);
    }

    #[test]
    fn font_style_helpers_emit_expected_markers() {
        let viewport = HtmlBrowserViewport {
            width: 10,
            height: 180,
            device_scale_factor: 1.0,
        };
        let mut renderer = HtmlLayoutRenderer::new(viewport, 2.0, &HashMap::new(), None);
        let style = CssStyle::browser_default();
        renderer.paint_text_line("hello", 2.0, 12.0, &style);
        assert!(renderer.svg.contains(" y=\"10\""));
    }

    fn first_text_x(svg: &str) -> Option<f32> {
        svg.lines()
            .find(|line| line.contains("<text"))
            .and_then(|line| {
                let start = line.find(" x=\"")? + 4;
                let suffix = line[start..].find('"')?;
                line[start..start + suffix].parse::<f32>().ok()
            })
    }
}
