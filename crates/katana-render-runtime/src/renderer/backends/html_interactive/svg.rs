use super::super::html_browser::HtmlBrowserViewport;
use super::style::CssStyle;

pub(super) fn svg_header(viewport: HtmlBrowserViewport) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="#ffffff"/>"##,
        viewport.width, viewport.height, viewport.width, viewport.height
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
    svg.push_str(&format!(
        r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{}"/>"#,
        escape_xml(background)
    ));
}

fn append_border(svg: &mut String, x: f32, y: f32, width: f32, height: f32, style: &CssStyle) {
    let Some(border) = &style.border else {
        return;
    };
    svg.push_str(&format!(
        r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="none" stroke="{}" stroke-width="1"/>"#,
        escape_xml(border)
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
