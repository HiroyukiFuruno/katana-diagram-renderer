mod constants;
mod control_style;
mod document;
mod input;
mod input_focus;
mod layout;
mod layout_container;
mod layout_control_paint;
mod layout_controls;
mod layout_dispatch;
mod layout_flow;
mod layout_flow_item;
mod layout_flow_measure;
mod layout_grid_measure;
mod layout_grid_track;
mod layout_inline;
mod layout_input;
mod layout_media;
mod layout_paint;
mod layout_structures;
mod layout_summary;
mod layout_table;
mod layout_text;
mod session;
mod session_geometry;
mod style;
mod svg;
#[cfg(test)]
mod tests;
mod types;

use super::html_browser::HtmlBrowserError;

pub(super) use session::HtmlInteractiveSession;

fn runtime_failure(error: impl ToString) -> HtmlBrowserError {
    HtmlBrowserError::RuntimeFailure {
        error: error.to_string(),
    }
}

#[cfg(test)]
mod runtime_failure_tests {
    use super::runtime_failure;
    use crate::renderer::backends::html_browser::HtmlBrowserError;

    #[test]
    fn runtime_failure_keeps_the_underlying_message() {
        assert_eq!(
            runtime_failure("raster failed"),
            HtmlBrowserError::RuntimeFailure {
                error: "raster failed".to_string(),
            }
        );
    }
}
