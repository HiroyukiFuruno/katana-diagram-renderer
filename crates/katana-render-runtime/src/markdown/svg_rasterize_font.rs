use resvg::usvg;

const BUNDLED_SANS_SERIF_FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

pub(super) fn rasterizer_options() -> usvg::Options<'static> {
    rasterizer_options_with_font_db(bundled_font_db())
}

pub(super) fn html_rasterizer_options() -> usvg::Options<'static> {
    rasterizer_options_with_font_db(html_font_db())
}

pub(super) fn rasterizer_options_with_font_db(
    fontdb: std::sync::Arc<usvg::fontdb::Database>,
) -> usvg::Options<'static> {
    usvg::Options {
        fontdb,
        ..usvg::Options::default()
    }
}

pub(super) fn bundled_font_db() -> std::sync::Arc<usvg::fontdb::Database> {
    static FONT_DB: std::sync::OnceLock<std::sync::Arc<usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    std::sync::Arc::clone(FONT_DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        /* WHY: 公開 SVG rasterizer の pixel 出力を host font から独立させる。 */
        db.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
        std::sync::Arc::new(db)
    }))
}

pub(super) fn html_font_db() -> std::sync::Arc<usvg::fontdb::Database> {
    static FONT_DB: std::sync::OnceLock<std::sync::Arc<usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    std::sync::Arc::clone(FONT_DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        /* WHY: HTML は bundled Latin を優先しつつ、追加 script を host font で補う。 */
        db.load_font_data(BUNDLED_SANS_SERIF_FONT.to_vec());
        db.load_system_fonts();
        std::sync::Arc::new(db)
    }))
}
