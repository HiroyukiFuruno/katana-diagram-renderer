use super::*;

type TestResult<T = ()> = Result<T, HtmlBrowserError>;
const TEST_VIEWPORT_WIDTH: u32 = 320;
const TEST_VIEWPORT_HEIGHT: u32 = 240;

#[test]
fn raster_and_frame_failures_are_mapped_to_runtime_errors() -> TestResult {
    let source = HtmlBrowserSource::new("<p>frame</p>", "https://example.test/frame.html")?;
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source, viewport)?;

    assert!(session.rasterize("not an svg document").is_err());
    assert!(
        session
            .rasterize("<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>")
            .is_err()
    );
    assert!(matches!(
        session.resize(HtmlBrowserViewport {
            width: 0,
            height: TEST_VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        }),
        Err(HtmlBrowserError::InvalidViewport)
    ));
    let layout = LayoutResult {
        svg: String::new(),
        hit_targets: Vec::new(),
        anchor_positions: HashMap::new(),
        content_height: 0.0,
    };
    assert!(session.store_frame(layout, vec![0]).is_err());
    Ok(())
}
