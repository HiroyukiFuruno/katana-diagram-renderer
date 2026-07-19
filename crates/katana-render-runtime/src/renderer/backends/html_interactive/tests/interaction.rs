use super::super::types::HitTargetKind;
use super::support::{
    TestResult, click_element, click_first_target, has_open_details, input_value, start, to_string,
};
use crate::renderer::backends::html_browser::{HtmlBrowserInput, HtmlBrowserViewport};

const NO_HORIZONTAL_SCROLL: f32 = 0.0;
const UNIT_SCROLL_DELTA: f32 = 1.0;

#[test]
fn button_click_runs_v8_handler_and_repaints() -> TestResult {
    let mut session = start(button_document())?;
    let before = frame_generation(&session, "initial frame")?;
    click_first_target(&mut session)?;
    let after = frame_generation(&session, "updated frame")?;
    assert!(after > before);
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("Done")
    );
    Ok(())
}

#[test]
fn summary_click_toggles_details_without_host_dom_logic() -> TestResult {
    let mut session = start("<details><summary>More</summary><p>Expanded content</p></details>")?;
    click_first_target(&mut session)?;
    let nodes = session.runtime.interactive_nodes().map_err(to_string)?;
    assert!(has_open_details(&nodes));
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("Expanded content")
    );
    Ok(())
}

#[test]
fn text_input_updates_the_v8_backed_dom_and_frame() -> TestResult {
    let mut session = start("<label>Name<input value=initial></label>")?;
    click_first_target(&mut session)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: " value".to_string(),
        })
        .map_err(to_string)?;
    let nodes = session.runtime.interactive_nodes().map_err(to_string)?;
    assert_eq!(input_value(&nodes), Some("initial value"));
    Ok(())
}

#[test]
fn attribute_selectors_dataset_and_input_toggle_events_repaint_the_dom() -> TestResult {
    let mut session = start(event_document())?;
    assert_initial_status(&session)?;
    click_element(&mut session, "summary")?;
    assert_open_status(&session)?;
    update_event_input(&mut session)?;
    assert_event_input_snapshot(&session)
}

#[test]
fn relative_link_emits_resolved_navigation() -> TestResult {
    let mut session = start("<a href=guide/next.html>Next</a>")?;
    click_first_target(&mut session)?;
    assert_eq!(
        session
            .take_navigation()
            .map(|navigation| navigation.url.as_str().to_string()),
        Some("https://example.test/docs/guide/next.html".to_string())
    );
    Ok(())
}

#[test]
fn cross_origin_link_is_rejected_without_a_navigation_event() -> TestResult {
    let mut session = start("<a id=remote href=https://other.example/next.html>Remote</a>")?;
    let error = required_error(click_element(&mut session, "remote"), "cross-origin link")?;

    assert!(error.contains("navigation is not allowed"));
    assert!(session.take_navigation().is_none());
    Ok(())
}

#[test]
fn summary_click_propagates_click_and_toggle_listener_errors() -> TestResult {
    assert_summary_click_error("click", "summary click error")?;
    assert_summary_click_error("toggle", "summary toggle error")
}

#[test]
fn resize_and_scroll_keep_the_frame_at_the_requested_viewport() -> TestResult {
    let mut session = start(long_document())?;
    let viewport = HtmlBrowserViewport::new(480, 160, 1.0).map_err(to_string)?;
    session.resize(viewport).map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: 80.0,
        })
        .map_err(to_string)?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "resized frame must exist".to_string())?;
    assert_eq!(frame.viewport, viewport);
    assert_eq!(frame.pixels.len(), 480 * 160 * 4);
    Ok(())
}

#[test]
fn passive_invalid_and_unfocused_input_paths_preserve_runtime_ownership() -> TestResult {
    let mut session = start("<input id=entry value=initial><button id=action>Run</button>")?;
    let initial = session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| "initial frame must exist".to_string())?;
    dispatch_passive_input_events(&mut session)?;
    assert_click_miss_and_mismatch(&mut session)?;
    assert_focus_loss_blocks_text(&mut session, initial)?;
    assert_invalid_pointer_is_rejected(&mut session);
    Ok(())
}

#[test]
fn input_dispatch_surfaces_runtime_errors_without_host_fallbacks() -> TestResult {
    assert_invalid_link_target_is_reported()?;
    assert_unsupported_link_scheme_is_reported()?;
    assert_removed_details_target_is_reported()?;
    assert_timeout_discards_runtime_for_later_input()?;
    Ok(())
}

fn assert_invalid_link_target_is_reported() -> TestResult {
    let mut session = start("<a id=broken href='http://[invalid'>Broken</a>")?;
    let error = required_error(click_element(&mut session, "broken"), "invalid href")?;
    assert!(error.contains("link target is invalid"));
    Ok(())
}

fn assert_unsupported_link_scheme_is_reported() -> TestResult {
    let mut session = start("<a id=mail href=mailto:test@example.test>Mail</a>")?;
    let error = required_error(
        click_element(&mut session, "mail"),
        "unsupported link scheme",
    )?;
    assert!(error.contains("unsupported"));
    Ok(())
}

fn assert_removed_details_target_is_reported() -> TestResult {
    let mut session = start("<details><summary id=toggle>More</summary></details>")?;
    let target = session
        .hit_targets
        .iter_mut()
        .find(|target| matches!(target.kind, HitTargetKind::Summary { .. }))
        .ok_or_else(|| "summary target must exist".to_string())?;
    target.kind = HitTargetKind::Summary {
        details_node_id: u64::MAX,
    };
    let error = required_error(
        click_element(&mut session, "toggle"),
        "stale details target",
    )?;
    assert!(error.contains("does not exist"));
    Ok(())
}

fn assert_timeout_discards_runtime_for_later_input() -> TestResult {
    let mut session =
        start("<input id=entry value=initial><button id=run onclick=\"for (;;) {}\">Run</button>")?;
    click_element(&mut session, "entry")?;
    let timeout = required_error(click_element(&mut session, "run"), "handler timeout")?;
    assert!(timeout.contains("timed out"));
    let text_error = required_error(
        session
            .dispatch_input(HtmlBrowserInput::Text {
                text: "after timeout".to_string(),
            })
            .map_err(to_string),
        "discarded runtime text",
    )?;
    assert!(text_error.to_string().contains("discarded"));
    let scroll_error = required_error(
        session
            .dispatch_input(HtmlBrowserInput::Scroll {
                delta_x: NO_HORIZONTAL_SCROLL,
                delta_y: UNIT_SCROLL_DELTA,
            })
            .map_err(to_string),
        "discarded runtime scroll",
    )?;
    assert!(scroll_error.to_string().contains("discarded"));
    Ok(())
}

fn required_error(result: TestResult, subject: &str) -> TestResult<String> {
    match result {
        Ok(()) => Err(format!("{subject} must fail")),
        Err(error) => Ok(error),
    }
}

fn dispatch_passive_input_events(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    dispatch_inputs(session, passive_text_events())?;
    dispatch_inputs(session, secondary_pointer_events())?;
    assert_eq!(
        input_value(&session.runtime.interactive_nodes().map_err(to_string)?),
        Some("initial")
    );
    Ok(())
}

fn passive_text_events() -> [HtmlBrowserInput; 5] {
    [
        HtmlBrowserInput::Focus { focused: true },
        HtmlBrowserInput::PointerMove { x: 4.0, y: 4.0 },
        HtmlBrowserInput::KeyDown {
            key: "Tab".to_string(),
        },
        HtmlBrowserInput::KeyUp {
            key: "Tab".to_string(),
        },
        HtmlBrowserInput::Text {
            text: " ignored".to_string(),
        },
    ]
}

fn secondary_pointer_events() -> [HtmlBrowserInput; 2] {
    [
        HtmlBrowserInput::PointerDown {
            x: 4.0,
            y: 4.0,
            button: 1,
        },
        HtmlBrowserInput::PointerUp {
            x: 4.0,
            y: 4.0,
            button: 1,
        },
    ]
}

fn dispatch_inputs<const N: usize>(
    session: &mut super::super::HtmlInteractiveSession,
    inputs: [HtmlBrowserInput; N],
) -> TestResult {
    for input in inputs {
        session.dispatch_input(input).map_err(to_string)?;
    }
    Ok(())
}

fn assert_click_miss_and_mismatch(
    session: &mut super::super::HtmlInteractiveSession,
) -> TestResult {
    dispatch_primary_click(session, 0.0, 0.0, 0.0, 0.0)?;
    let ((target_x, target_y), (other_x, other_y)) = mismatch_target_coordinates(session)?;
    dispatch_primary_click(
        session,
        target_x + 1.0,
        target_y + 1.0,
        other_x + 1.0,
        other_y + 1.0,
    )
}

type TargetCoordinates = (f32, f32);
type MismatchTargetCoordinates = (TargetCoordinates, TargetCoordinates);

fn mismatch_target_coordinates(
    session: &super::super::HtmlInteractiveSession,
) -> TestResult<MismatchTargetCoordinates> {
    let target = session
        .hit_targets
        .first()
        .cloned()
        .ok_or_else(|| "input target must exist".to_string())?;
    let other_target = session
        .hit_targets
        .get(1)
        .cloned()
        .ok_or_else(|| "button target must exist".to_string())?;
    Ok(((target.x, target.y), (other_target.x, other_target.y)))
}

fn dispatch_primary_click(
    session: &mut super::super::HtmlInteractiveSession,
    down_x: f32,
    down_y: f32,
    up_x: f32,
    up_y: f32,
) -> TestResult {
    dispatch_inputs(
        session,
        [
            HtmlBrowserInput::PointerDown {
                x: down_x,
                y: down_y,
                button: 0,
            },
            HtmlBrowserInput::PointerUp {
                x: up_x,
                y: up_y,
                button: 0,
            },
        ],
    )
}

fn assert_focus_loss_blocks_text(
    session: &mut super::super::HtmlInteractiveSession,
    initial: u64,
) -> TestResult {
    click_element(session, "entry")?;
    session
        .dispatch_input(HtmlBrowserInput::Focus { focused: false })
        .map_err(to_string)?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: " ignored again".to_string(),
        })
        .map_err(to_string)?;
    assert_eq!(
        input_value(&session.runtime.interactive_nodes().map_err(to_string)?),
        Some("initial")
    );
    assert!(
        session
            .latest_frame()
            .map(|frame| frame.generation > initial)
            .unwrap_or(false)
    );
    Ok(())
}

fn assert_invalid_pointer_is_rejected(session: &mut super::super::HtmlInteractiveSession) {
    assert!(matches!(
        session.dispatch_input(HtmlBrowserInput::PointerMove {
            x: f32::NAN,
            y: 0.0,
        }),
        Err(crate::renderer::backends::html_browser::HtmlBrowserError::InvalidInputCoordinates)
    ));
}

#[test]
fn orphan_list_item_uses_the_structural_item_layout_path() -> TestResult {
    let session = start(
        "<table></table><article>fallback container</article><br><ul>leading text<li>nested item</li></ul><li>orphan item</li>",
    )?;
    let frame = session
        .latest_frame()
        .ok_or_else(|| "list frame must exist".to_string())?;

    assert!(
        frame
            .pixels
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    Ok(())
}

fn assert_summary_click_error(listener: &str, message: &str) -> TestResult {
    let selector = if listener == "click" {
        "summary"
    } else {
        "details"
    };
    let document = format!(
        "<details><summary id=summary>More</summary></details><script>document.querySelector('{selector}').addEventListener('{listener}', () => {{ throw new Error('{message}'); }});</script>"
    );
    let mut session = start(&document)?;
    let error = required_error(click_element(&mut session, "summary"), message)?;

    assert!(error.contains(message));
    Ok(())
}

fn button_document() -> &'static str {
    r#"<button id=run>Run</button><script>document.getElementById('run').addEventListener('click', () => { const button = document.getElementById('run'); button.textContent = 'Done'; button.style.color = '#0a7a2f'; });</script>"#
}

fn frame_generation(
    session: &super::super::HtmlInteractiveSession,
    message: &str,
) -> TestResult<u64> {
    session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| format!("{message} must exist"))
}

fn event_document() -> &'static str {
    r#"<p id=status data-status>Initial</p><details id=details><summary id=summary>More</summary><input id=entry value=initial><p id=result>Waiting</p></details><script>
const status = document.querySelector('[data-status]');
const details = document.getElementById('details');
const input = document.getElementById('entry');
const result = document.getElementById('result');
document.body.dataset.ready = 'ready';
status.textContent = document.body.dataset.ready;
details.addEventListener('toggle', (event) => { status.textContent = event.currentTarget.open ? 'open' : 'closed'; });
input.addEventListener('input', (event) => { result.textContent = `input:${event.currentTarget.value}`; });
</script>"#
}

fn assert_initial_status(session: &super::super::HtmlInteractiveSession) -> TestResult {
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("ready")
    );
    Ok(())
}

fn assert_open_status(session: &super::super::HtmlInteractiveSession) -> TestResult {
    assert!(
        session
            .runtime
            .snapshot()
            .map_err(to_string)?
            .contains("open")
    );
    Ok(())
}

fn update_event_input(session: &mut super::super::HtmlInteractiveSession) -> TestResult {
    click_element(session, "entry")?;
    session
        .dispatch_input(HtmlBrowserInput::Text {
            text: "!".to_string(),
        })
        .map_err(to_string)
}

fn assert_event_input_snapshot(session: &super::super::HtmlInteractiveSession) -> TestResult {
    let snapshot = session.runtime.snapshot().map_err(to_string)?;
    assert!(snapshot.contains("value=\"initial!\""));
    assert!(snapshot.contains("input:initial!"));
    Ok(())
}

fn long_document() -> &'static str {
    "<p>one</p><p>two</p><p>three</p><p>four</p><p>five</p><p>six</p><p>seven</p><p>eight</p><p>nine</p><p>ten</p><p>eleven</p>"
}
