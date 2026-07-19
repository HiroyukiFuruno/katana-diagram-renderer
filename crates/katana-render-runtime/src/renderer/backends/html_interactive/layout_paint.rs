use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
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
        baseline_y: f32,
        style: &CssStyle,
    ) {
        for (index, line) in lines.iter().enumerate() {
            self.paint_text_line(
                line,
                x,
                baseline_y + index as f32 * style.line_height,
                style,
            );
        }
    }

    fn paint_text_line(&mut self, line: &str, x: f32, baseline_y: f32, style: &CssStyle) {
        let y = baseline_y - self.scroll_y;
        self.svg.push_str(&format!(
            r#"<text x="{x}" y="{y}" font-family="Noto Sans, sans-serif" font-size="{}" fill="{}"{}{}>{}</text>"#,
            style.font_size,
            escape_xml(&style.color),
            font_weight(style),
            text_decoration(style),
            escape_xml(line)
        ));
    }
}

fn font_weight(style: &CssStyle) -> &'static str {
    if style.bold {
        " font-weight=\"700\""
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
