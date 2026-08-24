use super::font::{font_has_char, matching_fallback_face};
use resvg::usvg;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static HTML_RESOLVED_FACE_CACHE: RefCell<HashMap<(usvg::fontdb::ID, char), usvg::fontdb::ID>> =
        RefCell::new(HashMap::new());
}

pub(super) fn html_font_runs(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    text: &str,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    for character in text.chars() {
        let face_id = HTML_RESOLVED_FACE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            *cache.entry((base_face_id, character)).or_insert_with(|| {
                if font_has_char(database, base_face_id, character) {
                    base_face_id
                } else {
                    matching_fallback_face(database, base_face_id, character)
                        .unwrap_or(base_face_id)
                }
            })
        });
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

#[cfg(test)]
mod tests {
    use super::super::font::matching_font_face;
    use super::{HTML_RESOLVED_FACE_CACHE, html_font_runs};
    use crate::markdown::svg_rasterize::font::html_font_db;

    #[test]
    fn html_font_fallback_is_cached_per_character() {
        HTML_RESOLVED_FACE_CACHE.with(|cache| cache.borrow_mut().clear());
        let database = html_font_db();
        let cache_size = matching_font_face(&database, "Noto Sans", 400, false).map(|base| {
            let runs = html_font_runs(&database, base, "日本日本");
            assert!(!runs.is_empty());
            HTML_RESOLVED_FACE_CACHE.with(|cache| cache.borrow().len())
        });

        assert_eq!(cache_size, Some(2));
    }
}
