use super::*;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 120;

#[test]
fn open_starts_the_in_process_rust_runtime() -> TestResult {
    let mut session = HtmlRuntime.open(test_source()?, viewport()?)?;

    assert!(session.has_in_process_runtime());
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(1)
    );
    session.close()?;
    Ok(())
}

#[test]
fn html_runtime_traits_are_value_like() {
    let runtime = HtmlRuntime;
    let copied = runtime;
    let cloned = <HtmlRuntime as Clone>::clone(&copied);

    assert_eq!(format!("{runtime:?}"), "HtmlRuntime");
    assert_eq!(format!("{cloned:?}"), format!("{copied:?}"));
}

#[test]
fn test_source_helper_propagates_invalid_origin() {
    assert!(matches!(
        test_source_with_origin("not a url"),
        Err(error)
            if error
                .downcast_ref::<HtmlBrowserError>()
                .is_some_and(|error| matches!(error, HtmlBrowserError::InvalidOrigin { .. }))
    ));
}

fn test_source() -> TestResult<HtmlBrowserSource> {
    test_source_with_origin("https://example.test/index.html")
}

fn test_source_with_origin(origin: &str) -> TestResult<HtmlBrowserSource> {
    Ok(HtmlBrowserSource::new("<p>ok</p>", origin)?)
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    Ok(HtmlBrowserViewport::new(
        TEST_VIEWPORT_WIDTH,
        TEST_VIEWPORT_HEIGHT,
        1.0,
    )?)
}
