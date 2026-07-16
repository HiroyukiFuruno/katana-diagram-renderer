use super::runtime;
use crate::HtmlBrowserViewport;
use std::{ffi::OsString, path::Path};

const CHROMIUM_NO_SANDBOX_ENV: &str = "KRR_CHROMIUM_NO_SANDBOX";
#[cfg(target_os = "linux")]
const BASE_CHROMIUM_ARGUMENT_COUNT: usize = 17;
#[cfg(not(target_os = "linux"))]
const BASE_CHROMIUM_ARGUMENT_COUNT: usize = 16;

pub(super) fn chromium_arguments(
    profile_directory: &Path,
    viewport: HtmlBrowserViewport,
) -> Vec<OsString> {
    let mut arguments = base_chromium_arguments(profile_directory, viewport);
    if sandbox_disabled_by_environment() {
        arguments.push(OsString::from("--no-sandbox"));
    }
    let [timer_arg, occluded_windows_arg, renderer_arg] = runtime::rendering_args();
    arguments.push(OsString::from(timer_arg));
    arguments.push(OsString::from(occluded_windows_arg));
    arguments.push(OsString::from(renderer_arg));
    arguments
}

fn base_chromium_arguments(
    profile_directory: &Path,
    viewport: HtmlBrowserViewport,
) -> Vec<OsString> {
    let user_data_dir = format!("--user-data-dir={}", profile_directory.display());
    let window_size = format!("--window-size={},{}", viewport.width, viewport.height);
    let mut arguments = Vec::with_capacity(BASE_CHROMIUM_ARGUMENT_COUNT);
    arguments.push(OsString::from("--remote-debugging-port=0"));
    arguments.push(OsString::from("--no-first-run"));
    arguments.push(OsString::from("--no-default-browser-check"));
    arguments.push(OsString::from("--headless=new"));
    push_swiftshader_arguments(&mut arguments);
    arguments.push(OsString::from("--disable-background-networking"));
    arguments.push(OsString::from("--disable-default-apps"));
    arguments.push(OsString::from("--disable-dev-shm-usage"));
    arguments.push(OsString::from("--disable-extensions"));
    arguments.push(OsString::from("--disable-popup-blocking"));
    arguments.push(OsString::from("--disable-sync"));
    arguments.push(OsString::from("--metrics-recording-only"));
    arguments.push(OsString::from("--mute-audio"));
    arguments.push(OsString::from(user_data_dir));
    arguments.push(OsString::from(window_size));
    arguments
}

#[cfg(target_os = "linux")]
fn push_swiftshader_arguments(arguments: &mut Vec<OsString>) {
    arguments.push(OsString::from("--use-gl=angle"));
    arguments.push(OsString::from("--use-angle=swiftshader"));
    arguments.push(OsString::from("--enable-unsafe-swiftshader"));
}

#[cfg(not(target_os = "linux"))]
fn push_swiftshader_arguments(arguments: &mut Vec<OsString>) {
    arguments.push(OsString::from("--use-gl=swiftshader"));
    arguments.push(OsString::from("--enable-unsafe-swiftshader"));
}

fn sandbox_disabled_by_environment() -> bool {
    match std::env::var_os(CHROMIUM_NO_SANDBOX_ENV) {
        Some(value) => value == "1",
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static SANDBOX_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn chromium_arguments_keep_the_browser_sandbox_enabled() {
        let _guard = sandbox_env_guard();
        unsafe { std::env::remove_var(CHROMIUM_NO_SANDBOX_ENV) };
        let profile = std::env::temp_dir().join("krr-page-test-profile");
        let viewport = test_viewport(16, 8, 1.0);
        let arguments = chromium_arguments(&profile, viewport);

        assert_base_browser_arguments(&arguments);
        assert_browser_sandbox_arguments(&arguments);
    }

    fn assert_base_browser_arguments(arguments: &[OsString]) {
        assert!(arguments.contains(&OsString::from("--remote-debugging-port=0")));
        assert!(arguments.contains(&OsString::from("--headless=new")));
        assert_swiftshader_arguments(arguments);
        assert!(!arguments.contains(&OsString::from("--disable-gpu")));
        assert!(arguments.contains(&OsString::from("--disable-dev-shm-usage")));
        assert!(arguments.contains(&OsString::from("--window-size=16,8")));
    }

    #[cfg(target_os = "linux")]
    fn assert_swiftshader_arguments(arguments: &[OsString]) {
        assert!(arguments.contains(&OsString::from("--use-gl=angle")));
        assert!(arguments.contains(&OsString::from("--use-angle=swiftshader")));
        assert!(arguments.contains(&OsString::from("--enable-unsafe-swiftshader")));
        assert!(!arguments.contains(&OsString::from("--use-gl=swiftshader")));
    }

    #[cfg(not(target_os = "linux"))]
    fn assert_swiftshader_arguments(arguments: &[OsString]) {
        assert!(arguments.contains(&OsString::from("--use-gl=swiftshader")));
        assert!(arguments.contains(&OsString::from("--enable-unsafe-swiftshader")));
        assert!(!arguments.contains(&OsString::from("--use-gl=angle")));
        assert!(!arguments.contains(&OsString::from("--use-angle=swiftshader")));
    }

    fn assert_browser_sandbox_arguments(arguments: &[OsString]) {
        assert!(!arguments.contains(&OsString::from("--no-sandbox")));
    }

    #[test]
    fn chromium_arguments_allow_ci_to_disable_the_browser_sandbox() {
        let _guard = sandbox_env_guard();
        unsafe { std::env::set_var(CHROMIUM_NO_SANDBOX_ENV, "1") };
        let profile = std::env::temp_dir().join("krr-page-test-profile");
        let viewport = test_viewport(16, 8, 1.0);
        let arguments = chromium_arguments(&profile, viewport);
        unsafe { std::env::remove_var(CHROMIUM_NO_SANDBOX_ENV) };

        assert!(arguments.contains(&OsString::from("--no-sandbox")));
    }

    #[test]
    fn chromium_arguments_ignore_other_sandbox_override_values() {
        let _guard = sandbox_env_guard();
        unsafe { std::env::set_var(CHROMIUM_NO_SANDBOX_ENV, "0") };
        let profile = std::env::temp_dir().join("krr-page-test-profile");
        let viewport = test_viewport(16, 8, 1.0);
        let arguments = chromium_arguments(&profile, viewport);
        unsafe { std::env::remove_var(CHROMIUM_NO_SANDBOX_ENV) };

        assert!(!arguments.contains(&OsString::from("--no-sandbox")));
    }

    #[test]
    fn sandbox_env_guard_recovers_from_poisoned_lock() {
        assert_panics(|| {
            let _guard = sandbox_env_guard();
            std::panic::resume_unwind(Box::new("poison sandbox env lock"));
        });
        drop(sandbox_env_guard());
    }

    #[test]
    fn test_viewport_reports_unexpected_validation_errors() {
        assert_panics(|| {
            let _ = test_viewport(0, 8, 1.0);
        });
    }

    fn test_viewport(width: u32, height: u32, device_scale_factor: f32) -> HtmlBrowserViewport {
        match HtmlBrowserViewport::new(width, height, device_scale_factor) {
            Ok(viewport) => viewport,
            Err(error) => fail(format!("unexpected viewport error: {error}")),
        }
    }

    fn assert_panics(action: impl FnOnce()) {
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)).is_err());
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    fn sandbox_env_guard() -> MutexGuard<'static, ()> {
        match SANDBOX_ENV_LOCK.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        }
    }
}
