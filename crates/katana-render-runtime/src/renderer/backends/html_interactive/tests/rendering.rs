use super::support::{TestResult, frame_contains_rgb, start, to_string};
use crate::renderer::backends::html_browser::{
    HTML_BROWSER_MAX_SOURCE_BYTES, HtmlBrowserError, HtmlBrowserSource, HtmlBrowserViewport,
};

const TEST_VIEWPORT_WIDTH: u32 = 320;
const TEST_VIEWPORT_HEIGHT: u32 = 240;

#[test]
fn renders_css_and_hides_document_metadata() -> TestResult {
    let session = start(metadata_document())?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert_eq!(frame.pixels.len(), 320 * 240 * 4);
    assert!(
        frame
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("Visible title"));
    assert!(!snapshot.contains("Hidden"));
    Ok(())
}

#[test]
fn body_layout_rules_do_not_expand_descendant_boxes() -> TestResult {
    let session = start(
        r#"<style>body { min-height: 720px; background: #ef4444; color: #172554; }</style><p>Visible body text</p>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "body layout frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [239, 68, 68]));
    assert!(session.content_height < 800.0, "{}", session.content_height);
    Ok(())
}

#[test]
fn styled_summary_and_link_paint_their_clickable_boxes() -> TestResult {
    let mut session = start(styled_link_document())?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(frame_contains_rgb(frame, [219, 234, 254]));
    assert!(frame_contains_rgb(frame, [237, 233, 254]));
    assert_target_exists(&mut session, "more")?;
    assert_target_exists(&mut session, "next")
}

#[test]
fn secondary_heading_levels_paint_with_their_tag_metrics() -> TestResult {
    let session = start("<h2>Section</h2><h3>Subsection</h3>")?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "heading frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    Ok(())
}

#[test]
fn styled_details_paints_its_closed_summary_container() -> TestResult {
    let session = start(
        r#"<style>details { background: #c4e2ff; border: 1px solid #2563eb; padding: 8px; }</style><details><summary>More</summary><p>Hidden panel</p></details>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(frame_contains_rgb(frame, [196, 226, 255]));
    Ok(())
}

#[test]
fn standalone_summary_paints_without_becoming_a_details_toggle_target() -> TestResult {
    let session = start("<summary>Standalone summary</summary>")?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "summary frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    assert!(session.hit_targets.is_empty());
    Ok(())
}

#[test]
fn structural_html_is_laid_out_as_tables_lists_rules_and_wrapped_text() -> TestResult {
    let session = start(
        r#"<main style="padding: 6px; margin: 4px; width: 280px; min-height: 120px; border: 1px solid #334155">
plain text that must use the direct text rendering path rather than a label
<hr style="border: 1px solid #dc2626">
<ul><li>unordered first</li><li>unordered second</li></ul>
<ol><li>ordered first</li><li>ordered second</li></ol>
<table><thead><tr><th>Feature</th><th>Status</th></tr></thead><tbody><tr></tr><tr><td>Direct runtime frame</td><td>Ready</td></tr></tbody></table>
<p style="display: none">hidden body content</p>
</main>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "structural frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("Direct runtime frame"));
    Ok(())
}

#[test]
fn inline_style_applies_box_font_and_visibility_declarations() -> TestResult {
    let session = start(
        r#"<p style="color: #14532d; background: #bbf7d0; border: 2px dashed #166534; padding: 8px; margin-top: 4px; margin-bottom: 6px; width: 240px; height: 48px; min-height: 64px; font-size: 18px; line-height: 24px; font-weight: 700; text-decoration: underline; letter-spacing: 1px; malformed">Styled content</p>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "styled frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [187, 247, 208]));
    assert!(frame_contains_rgb(frame, [20, 83, 45]));
    Ok(())
}

#[test]
fn raster_dimension_mismatch_is_a_typed_runtime_failure() -> TestResult {
    let session = start("<p>frame</p>")?;

    assert!(matches!(
        session.validate_raster_dimensions(319, 240),
        Err(crate::renderer::backends::html_browser::HtmlBrowserError::RuntimeFailure { error })
            if error.contains("319x240") && error.contains("320x240")
    ));
    Ok(())
}

#[test]
fn session_start_validates_mutated_sources_and_viewports() -> TestResult {
    assert!(matches!(
        super::super::HtmlInteractiveSession::start(oversized_source()?, test_viewport()?),
        Err(HtmlBrowserError::SourceTooLarge { .. })
    ));
    assert!(matches!(
        super::super::HtmlInteractiveSession::start(
            HtmlBrowserSource::new("<p>frame</p>", "https://example.test/docs/index.html")
                .map_err(to_string)?,
            HtmlBrowserViewport {
                width: 0,
                height: TEST_VIEWPORT_HEIGHT,
                device_scale_factor: 1.0,
            },
        ),
        Err(HtmlBrowserError::InvalidViewport)
    ));
    Ok(())
}

#[test]
fn border_color_uses_the_color_component_not_the_border_style() {
    assert_eq!(
        super::super::document::border_color("1px solid #93b4cf"),
        Some("#93b4cf".to_string())
    );
    assert_eq!(
        super::super::document::border_color("2px dashed red"),
        Some("red".to_string())
    );
    assert_eq!(super::super::document::border_color("solid"), None);
}

fn metadata_document() -> &'static str {
    r#"<!doctype html><html><head><title>Hidden</title><style>h1 { color: #0b74c7; }</style><script>window.hidden = true;</script></head><body><h1>Visible title</h1><p>Visible body</p></body></html>"#
}

fn styled_link_document() -> &'static str {
    r#"<style>
summary { background: #dbeafe; border: 1px solid #2563eb; padding: 8px; }
a { background: #ede9fe; border: 1px solid #7c3aed; padding: 8px; }
</style><details><summary id=more>More</summary><p>Hidden panel</p></details><a id=next href=next.html style="color:#4c1d95">Next</a>"#
}

fn oversized_source() -> TestResult<HtmlBrowserSource> {
    let mut source = HtmlBrowserSource::new("<p>frame</p>", "https://example.test/docs/index.html")
        .map_err(to_string)?;
    source.raw_html = "x".repeat(HTML_BROWSER_MAX_SOURCE_BYTES + 1);
    Ok(source)
}

fn test_viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0).map_err(to_string)
}

fn assert_target_exists(
    session: &mut super::super::HtmlInteractiveSession,
    id: &str,
) -> TestResult {
    let node_id = session
        .runtime
        .node_for_element_id(id)
        .ok_or_else(|| format!("{id} node must exist"))?
        .0;
    assert!(
        session
            .hit_targets
            .iter()
            .any(|target| target.node_id == node_id)
    );
    Ok(())
}
