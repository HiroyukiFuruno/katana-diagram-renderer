use super::layout::{DeferredPaint, HtmlLayoutRenderer};

impl HtmlLayoutRenderer {
    pub(super) fn defer_painted_range(&mut self, start: usize, z_index: i32) {
        let svg = self.svg.split_off(start);
        let order = self.next_paint_order;
        self.next_paint_order += 1;
        self.deferred_paint.push(DeferredPaint {
            z_index,
            order,
            svg,
        });
    }

    pub(super) fn wrap_painted_range(&mut self, start: usize, opacity: f32) {
        if self.svg.len() == start {
            return;
        }
        self.svg
            .insert_str(start, &format!(r#"<g opacity="{opacity}">"#));
        self.svg.push_str("</g>");
    }

    pub(super) fn finish_deferred_paint(&mut self) {
        self.deferred_paint
            .sort_by_key(|paint| (paint.z_index, paint.order));
        let mut negative = String::new();
        let mut foreground = String::new();
        for paint in self.deferred_paint.drain(..) {
            if paint.z_index < 0 {
                negative.push_str(&paint.svg);
            } else {
                foreground.push_str(&paint.svg);
            }
        }
        self.svg.insert_str(self.document_paint_start, &negative);
        self.svg.push_str(&foreground);
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlLayoutRenderer;
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use std::collections::HashMap;

    fn renderer() -> HtmlLayoutRenderer {
        HtmlLayoutRenderer::new(
            HtmlBrowserViewport {
                width: 100,
                height: 100,
                device_scale_factor: 1.0,
            },
            0.0,
            &HashMap::new(),
            None,
        )
    }

    #[test]
    fn wrapping_empty_range_is_a_noop() {
        let mut renderer = renderer();
        let start = renderer.svg.len();

        renderer.wrap_painted_range(start, 0.5);

        assert_eq!(renderer.svg.len(), start);
    }

    #[test]
    fn deferred_paint_places_negative_layers_behind_document() {
        let mut renderer = renderer();
        renderer.svg.push_str("document");
        let foreground = renderer.svg.len();
        renderer.svg.push_str("foreground");
        renderer.defer_painted_range(foreground, 1);
        let negative = renderer.svg.len();
        renderer.svg.push_str("negative");
        renderer.defer_painted_range(negative, -1);

        renderer.finish_deferred_paint();

        assert!(renderer.svg.ends_with("foreground"));
        assert!(renderer.svg.find("negative") < renderer.svg.find("document"));
    }
}
