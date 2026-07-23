use super::font::html_font_db;
use resvg::usvg;

const BOLD_FONT_WEIGHT: u16 = 700;
const NORMAL_FONT_WEIGHT: u16 = 400;

pub(super) fn shaped_text_width(
    text: &str,
    font_family: &str,
    font_size: f32,
    bold: bool,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let database = html_font_db();
    let base_face_id = matching_font_face(&database, font_family, bold, italic)?;
    let mut advance = 0.0;
    for (face_id, run) in font_runs(&database, base_face_id, text) {
        advance +=
            shape_text_with_face(&database, face_id, &run, font_size, font_feature_settings)?;
    }
    let spacing = text.chars().count().saturating_sub(1) as f32 * letter_spacing;
    Some(advance + spacing)
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
            Some(advance * font_size / face.units_per_em() as f32)
        })
        .flatten()
}

fn font_runs(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    text: &str,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    for character in text.chars() {
        let face_id = if font_has_char(database, base_face_id, character) {
            base_face_id
        } else {
            matching_fallback_face(database, base_face_id, character).unwrap_or(base_face_id)
        };
        if let Some((run_face_id, run)) = runs.last_mut()
            && *run_face_id == face_id
        {
            run.push(character);
        } else {
            runs.push((face_id, character.to_string()));
        }
    }
    runs
}

fn matching_fallback_face(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
) -> Option<usvg::fontdb::ID> {
    let base_face = database.face(base_face_id)?;
    database
        .faces()
        .filter(|face| face.id != base_face_id)
        .filter(|face| {
            [
                face.style == base_face.style,
                face.weight == base_face.weight,
                face.stretch == base_face.stretch,
            ]
            .into_iter()
            .any(|matches| matches)
        })
        .find(|face| font_has_char(database, face.id, character))
        .map(|face| face.id)
}

fn font_has_char(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    character: char,
) -> bool {
    database
        .with_face_data(face_id, |data, face_index| {
            rustybuzz::Face::from_slice(data, face_index)
                .is_some_and(|face| face.glyph_index(character).is_some())
        })
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::{
        font_has_char, font_runs, fontdb_family, matching_fallback_face, matching_font_face,
        rustybuzz_features, shape_text_with_face,
    };
    use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db};
    use resvg::usvg;

    #[test]
    fn html_text_measurement_uses_per_character_font_fallback() {
        let database = html_font_db();
        let used_fallback =
            matching_font_face(&database, "Noto Sans", false, false).and_then(|base| {
                matching_fallback_face(&database, base, '日').map(|fallback| {
                    !font_has_char(&database, base, '日')
                        && font_has_char(&database, fallback, '日')
                        && font_runs(&database, base, "A日本B").len() == 3
                })
            });
        assert_eq!(used_fallback, Some(true));

        let bundled_database = bundled_font_db();
        let bundled_only = matching_font_face(&bundled_database, "Noto Sans", false, false)
            .is_some_and(|base| {
                matching_fallback_face(&bundled_database, base, '日').is_none()
                    && font_runs(&bundled_database, base, "A日本B").len() == 1
            });
        assert!(bundled_only);
    }

    #[test]
    fn removed_font_face_fails_without_using_stale_font_data() {
        let mut database = (*bundled_font_db()).clone();
        let face_id = matching_font_face(&database, "Noto Sans", false, false);
        assert!(face_id.is_some(), "bundled Noto Sans must be available");
        let removed_face_is_rejected = face_id.is_some_and(|face_id| {
            database.remove_face(face_id);
            matching_fallback_face(&database, face_id, '日').is_none()
                && !font_has_char(&database, face_id, 'A')
                && shape_text_with_face(&database, face_id, "A", 16.0, None).is_none()
        });
        assert!(removed_face_is_rejected);
    }

    #[test]
    fn generic_families_and_feature_settings_cover_css_shaping_inputs() {
        assert!(matches!(
            fontdb_family("serif"),
            usvg::fontdb::Family::Serif
        ));
        assert!(matches!(
            fontdb_family("cursive"),
            usvg::fontdb::Family::Cursive
        ));
        assert!(matches!(
            fontdb_family("fantasy"),
            usvg::fontdb::Family::Fantasy
        ));
        assert!(rustybuzz_features(None).is_empty());
        assert_eq!(rustybuzz_features(Some("'liga', 'kern' 0")).len(), 2);
    }
}
