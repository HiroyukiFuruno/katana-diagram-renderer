use super::*;
use crate::{HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport};
use std::{cell::RefCell, path::PathBuf};

const RESIZED_VIEWPORT_WIDTH: u32 = 3;
const RESIZED_VIEWPORT_HEIGHT: u32 = 3;

type TestResult<T = ()> = Result<T, String>;

#[test]
fn handle_command_drives_a_loaded_chromium_page() -> TestResult {
    let _guard = runtime::CHROMIUM_BINARY_ENV_LOCK
        .lock()
        .map_err(|error| error.to_string())?;
    let chromium = test_chromium_binary()?;
    unsafe { std::env::set_var("KRR_CHROME_BIN", &chromium) };
    let slot = RefCell::new(None);
    let result = drive_loaded_page(&slot);
    unsafe { std::env::remove_var("KRR_CHROME_BIN") };
    result
}

fn drive_loaded_page(slot: &RefCell<Option<ChromiumPage>>) -> TestResult {
    let viewport = must_viewport()?;
    assert_frame(load_initial_page(slot, viewport)?);
    assert_frame(resize_loaded_page(slot)?);
    assert_frame(send_loaded_page_input(slot)?);
    assert_frame(navigate_loaded_page(slot, viewport)?);
    assert_closed(close_loaded_page(slot)?);
    Ok(())
}

fn load_initial_page(
    slot: &RefCell<Option<ChromiumPage>>,
    viewport: HtmlBrowserViewport,
) -> TestResult<(HtmlBrowserResponse, bool)> {
    let source = HtmlBrowserSource::new(
        "<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}#pixel{background:rgb(1,2,3)}</style><div id=\"pixel\"></div>",
        "https://example.test/unit-page.html",
    )
    .map_err(|error| error.to_string())?;
    handle_command(slot, HtmlBrowserCommand::Load { source, viewport })
        .map_err(error_pair_to_string)
}

fn resize_loaded_page(
    slot: &RefCell<Option<ChromiumPage>>,
) -> TestResult<(HtmlBrowserResponse, bool)> {
    let resized_viewport =
        HtmlBrowserViewport::new(RESIZED_VIEWPORT_WIDTH, RESIZED_VIEWPORT_HEIGHT, 1.0)
            .map_err(|error| error.to_string())?;
    handle_command(
        slot,
        HtmlBrowserCommand::Resize {
            viewport: resized_viewport,
        },
    )
    .map_err(error_pair_to_string)
}

fn send_loaded_page_input(
    slot: &RefCell<Option<ChromiumPage>>,
) -> TestResult<(HtmlBrowserResponse, bool)> {
    handle_command(
        slot,
        HtmlBrowserCommand::Input {
            input: HtmlBrowserInput::KeyUp {
                key: "Enter".to_string(),
            },
        },
    )
    .map_err(error_pair_to_string)
}

fn navigate_loaded_page(
    slot: &RefCell<Option<ChromiumPage>>,
    viewport: HtmlBrowserViewport,
) -> TestResult<(HtmlBrowserResponse, bool)> {
    let next_source = HtmlBrowserSource::new(
        "<!doctype html><style>html,body,#pixel{margin:0;width:100%;height:100%}#pixel{background:rgb(4,5,6)}</style><div id=\"pixel\"></div>",
        "https://example.test/unit-next.html",
    )
    .map_err(|error| error.to_string())?;
    handle_command(
        slot,
        HtmlBrowserCommand::Load {
            source: next_source,
            viewport,
        },
    )
    .map_err(error_pair_to_string)
}

fn close_loaded_page(
    slot: &RefCell<Option<ChromiumPage>>,
) -> TestResult<(HtmlBrowserResponse, bool)> {
    handle_command(slot, HtmlBrowserCommand::Close).map_err(error_pair_to_string)
}

fn assert_frame((response, close): (HtmlBrowserResponse, bool)) {
    assert!(matches!(response, HtmlBrowserResponse::Frame { .. }));
    assert!(!close);
}

fn assert_closed((response, close): (HtmlBrowserResponse, bool)) {
    assert!(matches!(response, HtmlBrowserResponse::Closed { .. }));
    assert!(close);
}

fn must_viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(2, 2, 1.0).map_err(|error| error.to_string())
}

fn error_pair_to_string((code, message): (String, String)) -> String {
    format!("{code}: {message}")
}

#[cfg(target_os = "macos")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    chromium_candidate([
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from(
            "/Applications/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
        ),
    ])
}

#[cfg(target_os = "linux")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    chromium_candidate([
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
    ])
}

#[cfg(target_os = "windows")]
fn test_chromium_binary() -> TestResult<PathBuf> {
    let mut candidates = Vec::new();
    for base in ["LOCALAPPDATA", "PROGRAMFILES", "PROGRAMFILES(X86)"] {
        if let Some(root) = std::env::var_os(base) {
            candidates.push(PathBuf::from(root).join("Google/Chrome/Application/chrome.exe"));
        }
    }
    chromium_candidate(candidates)
}

fn chromium_candidate(candidates: impl IntoIterator<Item = PathBuf>) -> TestResult<PathBuf> {
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| "test Chromium binary was not found in known install locations".to_string())
}
