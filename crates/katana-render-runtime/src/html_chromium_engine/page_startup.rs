use super::{
    chromium_process::{ChromiumProcess, launch_chromium},
    navigation::NavigationMonitor,
    page::string_error,
    popup_guard::PopupGuard,
    runtime, trace,
};
use crate::HtmlBrowserViewport;
use headless_chrome::Browser;
use std::{collections::HashSet, sync::Arc};

pub(super) struct BrowserPageParts {
    pub(super) browser: Browser,
    pub(super) chromium: ChromiumProcess,
    pub(super) tab: Arc<headless_chrome::Tab>,
    pub(super) navigation: NavigationMonitor,
    pub(super) popup_guard: PopupGuard,
}

pub(super) fn open_browser_page(viewport: HtmlBrowserViewport) -> Result<BrowserPageParts, String> {
    trace::stage("page:new:chrome-binary");
    let chrome_binary = runtime::chrome_binary_path()?;
    trace::stage("page:new:launch-chromium");
    let (browser, chromium, debug_ws_url) = launch_chromium(&chrome_binary, viewport)?;
    trace::stage("page:new:new-tab");
    let tab = browser.new_tab().map_err(string_error)?;
    trace::stage("page:new:install-navigation-monitor");
    let navigation = NavigationMonitor::install(&tab)?;
    trace::stage("page:new:install-popup-guard");
    let initial_target_ids = browser_target_ids(&browser)?;
    let popup_guard = PopupGuard::install(&debug_ws_url, tab.get_target_id(), initial_target_ids)
        .map_err(string_error)?;
    Ok(BrowserPageParts {
        browser,
        chromium,
        tab,
        navigation,
        popup_guard,
    })
}

fn browser_target_ids(browser: &Browser) -> Result<HashSet<String>, String> {
    browser
        .get_tabs()
        .lock()
        .map_err(string_error)
        .map(|tabs| tabs.iter().map(|tab| tab.get_target_id().clone()).collect())
}
