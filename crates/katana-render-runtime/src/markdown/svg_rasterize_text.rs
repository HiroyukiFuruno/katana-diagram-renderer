use super::SvgRasterizeError;
use super::font::{html_font_db, html_rasterizer_options};
use resvg::usvg;

const BOLD_FONT_WEIGHT: u16 = 700;
const NORMAL_FONT_WEIGHT: u16 = 400;
const FALLBACK_SVG_WIDTH: u32 = 16_384;
const FALLBACK_HEIGHT_FACTOR: f32 = 4.0;
const FALLBACK_BASELINE_FACTOR: f32 = 2.0;

pub(super) fn measure_html_text(
    text: &str,
    font_family: &str,
    font_size: f32,
    bold: bool,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Result<f32, SvgRasterizeError> {
    if text.is_empty() {
        return Ok(0.0);
    }
    shaped_text_width(
        text,
        font_family,
        font_size,
        bold,
        italic,
        letter_spacing,
        font_feature_settings,
    )
    .map_or_else(
        || fallback_text_width(text, font_family, font_size, bold, italic, letter_spacing),
        Ok,
    )
}

fn shaped_text_width(
    text: &str,
    font_family: &str,
    font_size: f32,
    bold: bool,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let database = html_font_db();
    let face_id = matching_font_face(&database, font_family, bold, italic)?;
    shape_text_with_face(
        &database,
        face_id,
        text,
        font_size,
        letter_spacing,
        font_feature_settings,
    )
}

fn matching_font_face(
    database: &usvg::fontdb::Database,
    font_family: &str,
    bold: bool,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    let names = css_font_family_names(font_family);
    let families = names
        .iter()
        .map(|name| fontdb_family(name))
        .collect::<Vec<_>>();
    database.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(if bold {
            BOLD_FONT_WEIGHT
        } else {
            NORMAL_FONT_WEIGHT
        }),
        stretch: usvg::fontdb::Stretch::Normal,
        style: if italic {
            usvg::fontdb::Style::Italic
        } else {
            usvg::fontdb::Style::Normal
        },
    })
}

fn shape_text_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font_size: f32,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    database
        .with_face_data(face_id, |data, face_index| {
            let face = rustybuzz::Face::from_slice(data, face_index)?;
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(text);
            buffer.guess_segment_properties();
            let glyphs =
                rustybuzz::shape(&face, &rustybuzz_features(font_feature_settings), buffer);
            let advance = glyphs
                .glyph_positions()
                .iter()
                .map(|position| position.x_advance as f32)
                .sum::<f32>();
            let spacing = text.chars().count().saturating_sub(1) as f32 * letter_spacing;
            Some(advance * font_size / face.units_per_em() as f32 + spacing)
        })
        .flatten()
}

fn css_font_family_names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(|name| name.trim_matches(['\'', '"']).to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

fn fontdb_family(name: &str) -> usvg::fontdb::Family<'_> {
    match name.to_ascii_lowercase().as_str() {
        "serif" => usvg::fontdb::Family::Serif,
        "sans-serif" | "system-ui" => usvg::fontdb::Family::SansSerif,
        "cursive" => usvg::fontdb::Family::Cursive,
        "fantasy" => usvg::fontdb::Family::Fantasy,
        "monospace" => usvg::fontdb::Family::Monospace,
        _ => usvg::fontdb::Family::Name(name),
    }
}

fn rustybuzz_features(settings: Option<&str>) -> Vec<rustybuzz::Feature> {
    settings
        .into_iter()
        .flat_map(|settings| settings.split(','))
        .filter_map(|setting| {
            let mut parts = setting.split_whitespace();
            let tag = parts.next()?.trim_matches(['\'', '"']);
            let value = parts.next().unwrap_or("1");
            format!("{tag}={value}").parse().ok()
        })
        .collect()
}

fn fallback_text_width(
    text: &str,
    font_family: &str,
    font_size: f32,
    bold: bool,
    italic: bool,
    letter_spacing: f32,
) -> Result<f32, SvgRasterizeError> {
    let weight = if bold {
        BOLD_FONT_WEIGHT
    } else {
        NORMAL_FONT_WEIGHT
    };
    let style = if italic { "italic" } else { "normal" };
    let height = (font_size * FALLBACK_HEIGHT_FACTOR).max(1.0);
    let baseline = font_size * FALLBACK_BASELINE_FACTOR;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{FALLBACK_SVG_WIDTH}" height="{height}"><text id="krr-text-measure" x="0" y="{baseline}" font-family="{}" font-size="{font_size}" font-weight="{weight}" font-style="{style}" letter-spacing="{letter_spacing}">{}</text></svg>"#,
        escape_xml_attribute(font_family),
        escape_xml_text(text)
    );
    let tree = usvg::Tree::from_str(&svg, &html_rasterizer_options())
        .map_err(|error| SvgRasterizeError::ParseFailed(error.to_string()))?;
    Ok(tree
        .node_by_id("krr-text-measure")
        .map(|node| node.abs_bounding_box().width())
        .unwrap_or(0.0))
}

fn escape_xml_attribute(value: &str) -> String {
    escape_xml_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{fallback_text_width, measure_html_text};

    #[test]
    fn empty_html_text_has_zero_width() {
        let measured = measure_html_text("", "sans-serif", 16.0, false, false, 0.0, None);

        assert!(matches!(measured, Ok(0.0)));
    }

    #[test]
    fn fallback_measurement_supports_bold_italic_text() {
        let measured = fallback_text_width("Bold", "sans-serif", 16.0, true, true, 0.0);

        assert!(matches!(measured, Ok(width) if width > 0.0));
    }
}
