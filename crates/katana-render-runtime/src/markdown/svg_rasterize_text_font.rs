use resvg::usvg;

pub(super) fn matching_font_face(
    database: &usvg::fontdb::Database,
    font_family: &str,
    font_weight: u16,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    let names = css_font_family_names(font_family);
    let families = names
        .iter()
        .map(|name| fontdb_family(name))
        .collect::<Vec<_>>();
    database.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font_weight),
        stretch: usvg::fontdb::Stretch::Normal,
        style: if italic {
            usvg::fontdb::Style::Italic
        } else {
            usvg::fontdb::Style::Normal
        },
    })
}

#[cfg(test)]
pub(super) fn font_runs(
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

pub(super) fn matching_fallback_face(
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

pub(super) fn font_has_char(
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

pub(super) fn fontdb_family(name: &str) -> usvg::fontdb::Family<'_> {
    match name.to_ascii_lowercase().as_str() {
        "serif" => usvg::fontdb::Family::Serif,
        "sans-serif" | "system-ui" => usvg::fontdb::Family::SansSerif,
        "cursive" => usvg::fontdb::Family::Cursive,
        "fantasy" => usvg::fontdb::Family::Fantasy,
        "monospace" => usvg::fontdb::Family::Monospace,
        _ => usvg::fontdb::Family::Name(name),
    }
}
