use super::super::html_browser::HtmlBrowserViewport;
use super::style::CssStyle;

pub(super) fn svg_header(viewport: HtmlBrowserViewport) -> String {
    let logical_width = viewport.logical_width();
    let logical_height = viewport.logical_height();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {logical_width} {logical_height}"><rect width="100%" height="100%" fill="#ffffff"/>"##,
        viewport.width, viewport.height
    )
}

pub(super) fn box_svg(x: f32, y: f32, width: f32, height: f32, style: &CssStyle) -> String {
    let mut svg = String::new();
    append_background(&mut svg, x, y, width, height, style);
    append_border(&mut svg, x, y, width, height, style);
    svg
}

fn append_background(svg: &mut String, x: f32, y: f32, width: f32, height: f32, style: &CssStyle) {
    let Some(background) = &style.background else {
        return;
    };
    if style.border_radius > 0.0 {
        svg.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{}" ry="{}" fill="{}"/>"#,
            style.border_radius,
            style.border_radius,
            escape_xml(background)
        ));
    } else {
        svg.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{}"/>"#,
            escape_xml(background)
        ));
    }
}

fn append_border(svg: &mut String, x: f32, y: f32, width: f32, height: f32, style: &CssStyle) {
    let Some(border) = &style.border else {
        return;
    };
    if style.border_width <= 0.0 {
        return;
    }
    let inset = style.border_width / 2.0;
    let painted_width = (width - style.border_width).max(0.0);
    let painted_height = (height - style.border_width).max(0.0);
    let radius = (style.border_radius - inset).max(0.0);
    svg.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{painted_width}" height="{painted_height}" rx="{radius}" ry="{radius}" fill="none" stroke="{}" stroke-width="{}"/>"#,
        x + inset,
        y + inset,
        escape_xml(border),
        style.border_width
    ));
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
