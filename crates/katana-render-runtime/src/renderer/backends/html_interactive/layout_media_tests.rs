use super::super::layout::HtmlLayoutRenderer;
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::HtmlDocumentNode;
use std::collections::HashMap;

type TestResult<T = ()> = Result<T, String>;
const TEST_VIEWPORT_WIDTH: u32 = 320;
const TEST_VIEWPORT_HEIGHT: u32 = 240;

#[test]
fn image_without_a_source_does_not_emit_svg_image_content() -> TestResult {
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let nodes = vec![HtmlDocumentNode::Element {
        node_id: 1,
        tag: "img".to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    }];
    let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None);

    assert!(!layout.svg.contains("<image"));
    Ok(())
}
