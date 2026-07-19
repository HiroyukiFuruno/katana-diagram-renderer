use super::*;

type TestResult<T = ()> = Result<T, String>;
const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 120;
const TEST_POINTER_COORDINATE: f32 = 20.0;

#[test]
fn session_rejects_invalid_input_and_out_of_order_internal_frames() -> TestResult {
    let mut session =
        HtmlBrowserSession::new(test_source("https://example.test/a.html")?, viewport()?)
            .map_err(|error| error.to_string())?;
    assert_validation_errors(&mut session);
    assert_sync_rejects_out_of_order_frame(&mut session)?;
    assert_missing_runtime_and_closed_state(&mut session)?;
    Ok(())
}

fn assert_validation_errors(session: &mut HtmlBrowserSession) {
    let invalid_viewport = HtmlBrowserViewport {
        width: 0,
        height: TEST_VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    assert_eq!(
        session.resize(invalid_viewport),
        Err(HtmlBrowserError::InvalidViewport)
    );
    assert_eq!(
        session.dispatch_input(HtmlBrowserInput::PointerMove {
            x: f32::NAN,
            y: TEST_POINTER_COORDINATE,
        }),
        Err(HtmlBrowserError::InvalidInputCoordinates)
    );
}

fn assert_sync_rejects_out_of_order_frame(session: &mut HtmlBrowserSession) -> TestResult {
    let initial = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    let newer = HtmlBrowserFrame::new(
        initial.generation + 1,
        initial.origin.clone(),
        initial.viewport,
        initial.pixel_format,
        initial.pixels.clone(),
    )
    .map_err(|error| error.to_string())?;
    session.latest_frame = Some(newer);
    assert!(matches!(
        session.sync_interactive_state(),
        Err(HtmlBrowserError::StaleFrameGeneration { .. })
    ));
    Ok(())
}

fn assert_missing_runtime_and_closed_state(session: &mut HtmlBrowserSession) -> TestResult {
    let initial = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    session.interactive = None;
    assert_eq!(
        session.dispatch_input(HtmlBrowserInput::Focus { focused: true }),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    assert_eq!(
        session.sync_interactive_state(),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    session.close().map_err(|error| error.to_string())?;
    assert_eq!(
        session.accept_frame(initial),
        Err(HtmlBrowserError::SessionClosed)
    );
    Ok(())
}

fn test_source(origin: &str) -> TestResult<HtmlBrowserSource> {
    HtmlBrowserSource::new("<button id=action>Run</button>", origin)
        .map_err(|error| error.to_string())
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())
}
