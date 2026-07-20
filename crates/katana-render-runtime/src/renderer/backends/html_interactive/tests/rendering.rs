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
fn compound_child_and_descendant_selectors_control_paint() -> TestResult {
    let session = start(
        r#"<style>
main > section.card[data-state="ready"] p.message.emphasis {
  background: #35a853;
  color: #17372a;
  padding: 4px;
}
</style>
<main>
  <section class="card" data-state="ready">
    <p class="message emphasis">Selector target</p>
  </section>
  <section class="card" data-state="pending">
    <p class="message emphasis">Non-target</p>
  </section>
</main>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "selector frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [53, 168, 83]));
    Ok(())
}

#[test]
fn url_attribute_selector_controls_stylesheet_paint() -> TestResult {
    let session = start(
        r#"<style>
a[href="https://example.com"] { background: #35a853; padding: 4px; }
</style>
<a href="https://example.com">Matching URL</a>
<a href="https://example.net">Different URL</a>"#,
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "selector frame must exist".to_string())?;

    assert!(frame_contains_rgb(frame, [53, 168, 83]));
    Ok(())
}

#[test]
fn four_value_box_shorthands_control_painted_geometry() -> TestResult {
    let mut session = start(
        r#"<main style="background: #123456; margin: 2px 4px 6px 8px; padding: 10px 12px 14px 16px"><p>Box model</p></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="24" y="18" width="276" height="56" fill="#123456"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn more_specific_padding_longhand_wins_over_later_shorthand_during_paint() -> TestResult {
    let mut session = start(
        r#"<style>
.card { padding-left: 20px; }
div { padding: 0; }
</style>
<div class="card">Cascade text</div>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(layout.svg.contains(r#"<text x="36""#), "{}", layout.svg);
    Ok(())
}

#[test]
fn flex_flow_positions_items_with_css_gap() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: flex; gap: 10px; width: 280px; }
.card { width: 100px; height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="100" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="126" y="16" width="100" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn flex_flow_resolves_percentage_item_width_once() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex; width:280px">
<section style="width:50%; height:40px; background:#ef4444">Half width</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="140" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn flex_flow_paints_a_direct_text_node() -> TestResult {
    let mut session = start(r#"<main style="display:flex; width:280px">Direct flow text</main>"#)?;
    let layout = session.layout().map_err(to_string)?;

    assert!(layout.svg.contains("Direct flow text"), "{}", layout.svg);
    Ok(())
}

#[test]
fn hidden_flex_item_does_not_consume_width_or_gap() -> TestResult {
    let mut session = start(
        r#"<main style="display:flex; gap:10px; width:280px">
<section hidden style="width:100px; height:40px; background:#ef4444">Hidden</section>
<section style="width:100px; height:40px; background:#3b82f6">Visible</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="100" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    assert!(!layout.svg.contains("Hidden"), "{}", layout.svg);
    Ok(())
}

#[test]
fn hidden_grid_item_does_not_consume_a_track_or_gap() -> TestResult {
    let mut session = start(
        r#"<main style="display:grid; grid-template-columns:100px 100px; gap:10px; width:280px">
<section hidden style="height:40px; background:#ef4444">Hidden</section>
<section style="height:40px; background:#3b82f6">Visible</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="100" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    assert!(!layout.svg.contains("Hidden"), "{}", layout.svg);
    Ok(())
}

#[test]
fn grid_flow_positions_items_in_declared_columns() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; width: 280px; }
.card { height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="134" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="162" y="16" width="134" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn grid_flow_preserves_fixed_and_fractional_track_sizes() -> TestResult {
    let mut session = start(
        r#"<style>
main { display: grid; grid-template-columns: 100px 1fr; gap: 12px; width: 280px; }
.card { height: 40px; }
#first { background: #ef4444; }
#second { background: #3b82f6; }
</style>
<main><section id=first class=card>First</section><section id=second class=card>Second</section></main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="100" height="40" fill="#ef4444"/>"##),
        "{}",
        layout.svg
    );
    assert!(
        layout
            .svg
            .contains(r##"<rect x="128" y="16" width="168" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn grid_flow_resolves_percentage_item_width_once() -> TestResult {
    let mut session = start(
        r#"<main style="display:grid; grid-template-columns:1fr; width:280px">
<section style="width:50%; height:40px; background:#3b82f6">Half width</section>
</main>"#,
    )?;
    let layout = session.layout().map_err(to_string)?;

    assert!(
        layout
            .svg
            .contains(r##"<rect x="16" y="16" width="140" height="40" fill="#3b82f6"/>"##),
        "{}",
        layout.svg
    );
    Ok(())
}

#[test]
fn percentage_width_reflows_against_resized_viewport() -> TestResult {
    let mut session = start(r#"<main style="width:50%; height:40px; background:#123456"></main>"#)?;
    let initial = session.layout().map_err(to_string)?;
    assert!(
        initial
            .svg
            .contains(r##"<rect x="16" y="16" width="144" height="40" fill="#123456"/>"##),
        "{}",
        initial.svg
    );

    session
        .resize(HtmlBrowserViewport::new(480, 240, 1.0).map_err(to_string)?)
        .map_err(to_string)?;
    let resized = session.layout().map_err(to_string)?;
    assert!(
        resized
            .svg
            .contains(r##"<rect x="16" y="16" width="224" height="40" fill="#123456"/>"##),
        "{}",
        resized.svg
    );
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
fn high_density_viewport_uses_logical_css_layout_and_physical_frame() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<main style='background:#123456'><p>Retina frame</p></main>",
        "https://example.test/docs/index.html",
    )
    .map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(640, 480, 2.0).map_err(to_string)?;
    let mut session =
        super::super::HtmlInteractiveSession::start(source, viewport).map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "retina frame must exist".to_string())?;

    assert_eq!(frame.pixels.len(), 640 * 480 * 4);
    let layout = session.layout().map_err(to_string)?;
    assert!(
        layout
            .svg
            .starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 320 240">"#),
        "{}",
        layout.svg
    );
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
