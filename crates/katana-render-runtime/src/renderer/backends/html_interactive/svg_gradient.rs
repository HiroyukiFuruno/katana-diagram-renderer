use super::super::style::CssStyle;
use super::escape_xml;

#[path = "svg_gradient_parse.rs"]
mod parse;

use parse::{distribute_offsets, gradient_direction, split_top_level, split_top_level_whitespace};

const DEFAULT_GRADIENT_ANGLE_DEGREES: f32 = 180.0;
const PERCENT_MAXIMUM: f32 = 100.0;

pub(super) fn append_background(
    svg: &mut String,
    gradient_id: u64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    let Some(background) = style.background.as_deref() else {
        return;
    };
    let geometry = (x, y, width, height);
    let Some(fill) = background_fill(svg, gradient_id, background, geometry) else {
        return;
    };
    append_background_rect(svg, geometry, &fill, style);
}

fn background_fill(
    svg: &mut String,
    gradient_id: u64,
    background: &str,
    geometry: (f32, f32, f32, f32),
) -> Option<String> {
    if background
        .trim_start()
        .to_ascii_lowercase()
        .starts_with("linear-gradient(")
    {
        let gradient = LinearGradient::parse(background)?;
        let id = format!("krr-gradient-{gradient_id}");
        gradient.append_definition(svg, &id, geometry);
        Some(format!("url(#{id})"))
    } else if background.contains('(') && !background.trim_start().starts_with("rgb") {
        None
    } else {
        Some(background.to_string())
    }
}

fn append_background_rect(
    svg: &mut String,
    geometry: (f32, f32, f32, f32),
    fill: &str,
    style: &CssStyle,
) {
    let (x, y, width, height) = geometry;
    let radius = style.resolved_border_radius(width, height);
    if radius.0 > 0.0 || radius.1 > 0.0 {
        svg.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{}" ry="{}" fill="{}"/>"#,
            radius.0,
            radius.1,
            escape_xml(fill)
        ));
    } else {
        svg.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{}"/>"#,
            escape_xml(fill)
        ));
    }
}

pub(super) struct LinearGradient {
    pub(super) angle_degrees: f32,
    stops: Vec<GradientStop>,
}

pub(super) struct GradientStop {
    color: String,
    pub(super) offset: Option<f32>,
}

impl LinearGradient {
    pub(super) fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let open = value.find('(')?;
        if !value[..open].trim().eq_ignore_ascii_case("linear-gradient") || !value.ends_with(')') {
            return None;
        }
        let arguments = split_top_level(&value[open + 1..value.len() - 1], ',');
        if arguments.len() < 2 {
            return None;
        }
        let (angle_degrees, stop_start) = gradient_direction(arguments[0])
            .map_or((DEFAULT_GRADIENT_ANGLE_DEGREES, 0), |angle| (angle, 1));
        let mut stops = arguments[stop_start..]
            .iter()
            .map(|value| GradientStop::parse(value))
            .collect::<Option<Vec<_>>>()?;
        if stops.len() < 2 {
            return None;
        }
        distribute_offsets(&mut stops);
        Some(Self {
            angle_degrees,
            stops,
        })
    }

    fn append_definition(&self, svg: &mut String, id: &str, geometry: (f32, f32, f32, f32)) {
        let (x1, y1, x2, y2) = self.line(geometry);
        svg.push_str(&format!(
            r#"<defs><linearGradient id="{}" gradientUnits="userSpaceOnUse" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}">"#,
            escape_xml(id),
        ));
        for stop in &self.stops {
            svg.push_str(&format!(
                r#"<stop offset="{}%" stop-color="{}"/>"#,
                stop.offset.unwrap_or_default(),
                escape_xml(&stop.color)
            ));
        }
        svg.push_str("</linearGradient></defs>");
    }

    fn line(&self, geometry: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
        let (x, y, width, height) = geometry;
        let radians = self.angle_degrees.to_radians();
        let direction_x = radians.sin();
        let direction_y = -radians.cos();
        let line_length = width * direction_x.abs() + height * direction_y.abs();
        let half_x = direction_x * line_length / 2.0;
        let half_y = direction_y * line_length / 2.0;
        let center_x = x + width / 2.0;
        let center_y = y + height / 2.0;
        (
            center_x - half_x,
            center_y - half_y,
            center_x + half_x,
            center_y + half_y,
        )
    }
}

impl GradientStop {
    fn parse(value: &str) -> Option<Self> {
        let tokens = split_top_level_whitespace(value.trim());
        let (color, offset) = match tokens.as_slice() {
            [color] if !color.is_empty() => ((*color).to_string(), None),
            [color, offset] if !color.is_empty() => {
                let offset = offset.strip_suffix('%')?.trim().parse::<f32>().ok()?;
                if !offset.is_finite() {
                    return None;
                }
                (
                    (*color).to_string(),
                    Some(offset.clamp(0.0, PERCENT_MAXIMUM)),
                )
            }
            _ => return None,
        };
        Some(Self { color, offset })
    }
}

#[cfg(test)]
mod tests {
    use super::LinearGradient;

    #[test]
    fn gradient_parser_rejects_invalid_function_and_stop_counts() {
        assert!(LinearGradient::parse("radial-gradient(red, blue)").is_none());
        assert!(LinearGradient::parse("linear-gradient(to right, red)").is_none());
    }

    #[test]
    fn gradient_parser_rejects_non_finite_and_multi_token_offsets() {
        assert!(LinearGradient::parse("linear-gradient(red NaN%, blue)").is_none());
        assert!(LinearGradient::parse("linear-gradient(red 10% extra, blue)").is_none());
    }

    #[test]
    fn horizontal_gradient_projects_the_box_corners() {
        let projected =
            LinearGradient::parse("linear-gradient(90deg, red, blue)").map(|gradient| {
                let (x1, y1, x2, y2) = gradient.line((10.0, 20.0, 200.0, 100.0));
                assert!((x1 - 10.0).abs() < 0.001, "{x1}");
                assert!((y1 - 70.0).abs() < 0.001, "{y1}");
                assert!((x2 - 210.0).abs() < 0.001, "{x2}");
                assert!((y2 - 70.0).abs() < 0.001, "{y2}");
            });
        assert!(projected.is_some());
    }

    #[test]
    fn diagonal_gradient_projects_the_box_corners() {
        let projected =
            LinearGradient::parse("linear-gradient(155deg, red, blue)").map(|gradient| {
                let (x1, y1, x2, y2) = gradient.line((0.0, 0.0, 200.0, 100.0));
                assert!((x1 - 62.99).abs() < 0.02, "{x1}");
                assert!((y1 + 29.38).abs() < 0.02, "{y1}");
                assert!((x2 - 137.01).abs() < 0.02, "{x2}");
                assert!((y2 - 129.38).abs() < 0.02, "{y2}");
            });
        assert!(projected.is_some());
    }
}
