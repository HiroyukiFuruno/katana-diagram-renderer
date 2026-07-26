use super::super::html_browser::HtmlBrowserViewport;
use super::style::CssStyle;

#[path = "svg_border.rs"]
mod border;
#[path = "svg_gradient.rs"]
mod gradient;

#[cfg(test)]
use gradient::LinearGradient;

pub(super) fn svg_header(viewport: HtmlBrowserViewport) -> String {
    let logical_width = viewport.logical_width();
    let logical_height = viewport.logical_height();
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {logical_width} {logical_height}"><rect width="100%" height="100%" fill="#ffffff"/>"##,
        viewport.width, viewport.height
    )
}

pub(super) fn box_svg(
    gradient_id: u64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) -> String {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return String::new();
    }
    let mut svg = String::new();
    append_box_shadow(&mut svg, x, y, width, height, style);
    gradient::append_background(&mut svg, gradient_id, x, y, width, height, style);
    border::append_border(&mut svg, x, y, width, height, style);
    svg
}

fn append_box_shadow(svg: &mut String, x: f32, y: f32, width: f32, height: f32, style: &CssStyle) {
    const LAYER_OPACITIES: [f32; 8] = [0.10, 0.09, 0.08, 0.07, 0.055, 0.04, 0.03, 0.015];
    let Some(shadow) = &style.box_shadow else {
        return;
    };
    let radius = style.resolved_border_radius(width, height);
    for (layer, opacity) in LAYER_OPACITIES.iter().enumerate().rev() {
        let blur_expansion = shadow.blur_radius * (layer + 1) as f32 / LAYER_OPACITIES.len() as f32;
        let expansion = shadow.spread_radius + blur_expansion;
        let shadow_x = x + shadow.offset_x - expansion;
        let shadow_y = y + shadow.offset_y - expansion;
        let shadow_width = width + expansion * 2.0;
        let shadow_height = height + expansion * 2.0;
        if shadow_width <= 0.0 || shadow_height <= 0.0 {
            continue;
        }
        svg.push_str(&format!(
            r#"<rect x="{shadow_x}" y="{shadow_y}" width="{shadow_width}" height="{shadow_height}" rx="{}" ry="{}" fill="{}" fill-opacity="{opacity}"/>"#,
            (radius.0 + expansion).max(0.0),
            (radius.1 + expansion).max(0.0),
            escape_xml(&shadow.color),
        ));
    }
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{LinearGradient, box_svg};
    use crate::renderer::backends::html_interactive::style::CssStyle;

    #[test]
    fn linear_gradient_paints_angles_directions_and_distributed_stops() {
        let mut style = CssStyle::browser_default();
        style.background = Some(
            "linear-gradient(155deg, #173382 0%, rgba(44,74,198,0.8), #3952ff 100%)".to_string(),
        );
        let svg = box_svg(7, 0.0, 0.0, 200.0, 100.0, &style);

        assert!(svg.contains(r#"id="krr-gradient-7""#), "{svg}");
        assert!(svg.contains(r#"fill="url(#krr-gradient-7)""#), "{svg}");
        assert!(
            svg.contains(r##"offset="0%" stop-color="#173382""##),
            "{svg}"
        );
        assert!(
            svg.contains(r#"offset="50%" stop-color="rgba(44,74,198,0.8)""#),
            "{svg}"
        );
        assert!(
            svg.contains(r##"offset="100%" stop-color="#3952ff""##),
            "{svg}"
        );

        assert_eq!(
            LinearGradient::parse("linear-gradient(to right, red, blue)")
                .map(|gradient| gradient.angle_degrees),
            Some(90.0)
        );
        assert!(LinearGradient::parse("linear-gradient(red)").is_none());
    }

    #[test]
    fn unsupported_background_images_do_not_emit_invalid_svg_fill() {
        let mut style = CssStyle::browser_default();
        style.background = Some("url(background.png)".to_string());
        assert_eq!(box_svg(0, 0.0, 0.0, 10.0, 10.0, &style), "");
    }

    #[test]
    fn box_shadow_uses_gaussian_weighted_layers_behind_the_background() {
        let attributes = [(
            "style".to_string(),
            "box-shadow: 0 10px 28px rgba(15,40,89,0.14)".to_string(),
        )];
        let mut style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        style.background = Some("#ffffff".to_string());
        let svg = box_svg(0, 10.0, 20.0, 100.0, 50.0, &style);

        assert_eq!(svg.matches("fill-opacity=\"").count(), 8, "{svg}");
        assert!(svg.contains(r#"fill-opacity="0.1""#), "{svg}");
        assert!(svg.contains(r#"fill-opacity="0.015""#), "{svg}");
        let paint_order = (svg.find("fill-opacity"), svg.rfind(r##"fill="#ffffff""##));
        assert!(
            matches!(paint_order, (Some(shadow), Some(background)) if shadow < background),
            "{svg}"
        );
    }

    #[test]
    fn non_positive_or_non_finite_boxes_do_not_emit_invalid_svg_geometry() {
        let mut style = CssStyle::browser_default();
        style.background = Some("red".to_string());

        assert_eq!(box_svg(0, 0.0, 0.0, 0.0, 10.0, &style), "");
        assert_eq!(box_svg(0, 0.0, 0.0, 10.0, -1.0, &style), "");
        assert_eq!(box_svg(0, 0.0, 0.0, f32::NAN, 10.0, &style), "");
    }

    #[test]
    fn shadow_layers_with_non_positive_geometry_are_not_painted() {
        let attributes = [(
            "style".to_string(),
            "box-shadow: 0 0 0 -20px red".to_string(),
        )];
        let mut style = CssStyle::from_element("div", &attributes, &CssStyle::browser_default());
        style.background = Some("white".to_string());

        let svg = box_svg(0, 0.0, 0.0, 10.0, 10.0, &style);

        assert!(!svg.contains("fill-opacity"), "{svg}");
        assert!(svg.contains("fill=\"white\""), "{svg}");
    }
}
