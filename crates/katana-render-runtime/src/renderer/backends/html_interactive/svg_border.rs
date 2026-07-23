use super::super::style::CssStyle;
use super::escape_xml;

const BORDER_EDGE_COUNT: usize = 4;

pub(super) fn append_border(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    if style.has_border_edge_overrides() {
        append_border_edges(svg, x, y, width, height, style);
        return;
    }
    append_uniform_border(svg, x, y, width, height, style);
}

fn append_border_edges(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    for edge in border_edges(x, y, width, height, style) {
        append_border_edge(svg, edge.start, edge.end, edge.color, edge.width);
    }
}

struct BorderEdge<'a> {
    start: (f32, f32),
    end: (f32, f32),
    color: Option<&'a str>,
    width: f32,
}

fn border_edges<'a>(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &'a CssStyle,
) -> [BorderEdge<'a>; BORDER_EDGE_COUNT] {
    let top = style.border_top_width();
    let right = style.border_right_width();
    let bottom = style.border_bottom_width();
    let left = style.border_left_width();
    [
        horizontal_edge(x, y + top / 2.0, width, style.border_top_color(), top),
        vertical_edge(
            x + width - right / 2.0,
            y,
            height,
            style.border_right_color(),
            right,
        ),
        horizontal_edge(
            x,
            y + height - bottom / 2.0,
            width,
            style.border_bottom_color(),
            bottom,
        ),
        vertical_edge(x + left / 2.0, y, height, style.border_left_color(), left),
    ]
}

fn horizontal_edge<'a>(
    x: f32,
    y: f32,
    length: f32,
    color: Option<&'a str>,
    width: f32,
) -> BorderEdge<'a> {
    BorderEdge {
        start: (x, y),
        end: (x + length, y),
        color,
        width,
    }
}

fn vertical_edge<'a>(
    x: f32,
    y: f32,
    length: f32,
    color: Option<&'a str>,
    width: f32,
) -> BorderEdge<'a> {
    BorderEdge {
        start: (x, y),
        end: (x, y + length),
        color,
        width,
    }
}

fn append_uniform_border(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    let Some(border) = &style.border else {
        return;
    };
    if style.border_width <= 0.0 {
        return;
    }
    let inset = style.border_width / 2.0;
    let painted_width = (width - style.border_width).max(0.0);
    let painted_height = (height - style.border_width).max(0.0);
    let radius = style.resolved_border_radius(width, height);
    let radius = ((radius.0 - inset).max(0.0), (radius.1 - inset).max(0.0));
    svg.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{painted_width}" height="{painted_height}" rx="{}" ry="{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
        x + inset,
        y + inset,
        radius.0,
        radius.1,
        escape_xml(border),
        style.border_width
    ));
}

fn append_border_edge(
    svg: &mut String,
    start: (f32, f32),
    end: (f32, f32),
    color: Option<&str>,
    width: f32,
) {
    let Some(color) = color.filter(|_| width > 0.0) else {
        return;
    };
    svg.push_str(&format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{width}"/>"#,
        start.0,
        start.1,
        end.0,
        end.1,
        escape_xml(color)
    ));
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, append_border};

    #[test]
    fn border_edge_without_color_or_positive_width_is_not_painted() {
        let attributes = [("style".to_string(), "border-top-width: 2px".to_string())];
        let style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        let mut svg = String::new();

        append_border(&mut svg, 0.0, 0.0, 20.0, 10.0, &style);

        assert!(svg.is_empty());
    }
}
