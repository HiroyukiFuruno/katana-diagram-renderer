use crate::HtmlBrowserNavigationEvent;
use headless_chrome::{
    Tab,
    protocol::cdp::{Page, types::Event},
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};
use url::Url;

#[derive(Clone)]
pub(super) struct NavigationMonitor {
    root_frame_id: Page::FrameId,
    state: Arc<Mutex<NavigationState>>,
}

#[derive(Default)]
struct NavigationState {
    requested_target: Option<String>,
    confirmed_targets: VecDeque<String>,
    popup_targets: VecDeque<String>,
    closed_popup_targets: usize,
}

impl NavigationMonitor {
    pub(super) fn install(tab: &Tab) -> Result<Self, String> {
        let root_frame_id = tab
            .call_method(Page::GetFrameTree(None))
            .map_err(string_error)?
            .frame_tree
            .frame
            .id;
        let monitor = Self {
            root_frame_id,
            state: Arc::new(Mutex::new(NavigationState::default())),
        };
        let listener_monitor = monitor.clone();
        tab.add_event_listener(Arc::new(move |event: &Event| match event {
            Event::PageFrameRequestedNavigation(event)
                if event.params.frame_id == listener_monitor.root_frame_id
                    && event.params.disposition
                        == Page::ClientNavigationDisposition::CurrentTab =>
            {
                lock(&listener_monitor.state).requested_target = Some(event.params.url.clone());
            }
            Event::PageWindowOpen(event) => {
                listener_monitor.record_popup(event.params.url.clone());
            }
            _ => {}
        }))
        .map(|_| ())
        .map_err(string_error)?;
        Ok(monitor)
    }

    pub(super) fn is_root_frame(&self, frame_id: &Page::FrameId) -> bool {
        frame_id == &self.root_frame_id
    }

    pub(super) fn confirm(&self, request_url: &str) {
        let mut state = lock(&self.state);
        let requested = state.requested_target.take();
        let target = requested
            .filter(|target| same_request_without_fragment(target, request_url))
            .unwrap_or_else(|| request_url.to_string());
        state.confirmed_targets.push_back(target);
    }

    pub(super) fn confirm_closed_popups(&self, count: usize) {
        let mut state = lock(&self.state);
        state.closed_popup_targets = state.closed_popup_targets.saturating_add(count);
        pair_popup_targets(&mut state);
    }

    pub(super) fn has_pending_popup(&self) -> bool {
        !lock(&self.state).popup_targets.is_empty()
    }

    pub(super) fn has_confirmed(&self) -> bool {
        !lock(&self.state).confirmed_targets.is_empty()
    }

    pub(super) fn take(&self) -> Result<Option<HtmlBrowserNavigationEvent>, String> {
        lock(&self.state)
            .confirmed_targets
            .pop_front()
            .map(HtmlBrowserNavigationEvent::new)
            .transpose()
            .map_err(string_error)
    }

    fn record_popup(&self, target: String) {
        let mut state = lock(&self.state);
        state.popup_targets.push_back(target);
        pair_popup_targets(&mut state);
    }
}

fn pair_popup_targets(state: &mut NavigationState) {
    while state.closed_popup_targets != 0 {
        let Some(target) = state.popup_targets.pop_front() else {
            return;
        };
        state.closed_popup_targets -= 1;
        if !target.is_empty() && target != "about:blank" {
            state.confirmed_targets.push_back(target);
        }
    }
}

fn same_request_without_fragment(candidate: &str, request_url: &str) -> bool {
    let Ok(mut candidate) = Url::parse(candidate) else {
        return false;
    };
    candidate.set_fragment(None);
    candidate.as_str() == request_url
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(state: NavigationState) -> NavigationMonitor {
        NavigationMonitor {
            root_frame_id: "root".to_string(),
            state: Arc::new(Mutex::new(state)),
        }
    }

    #[test]
    fn confirmed_navigation_preserves_the_requested_fragment() -> Result<(), String> {
        let monitor = monitor(NavigationState {
            requested_target: Some("https://example.test/next.html#section".to_string()),
            ..NavigationState::default()
        });

        monitor.confirm("https://example.test/next.html");

        assert!(monitor.has_confirmed());
        let navigation = monitor.take()?.ok_or("navigation was not captured")?;
        assert_eq!(
            navigation.url.as_str(),
            "https://example.test/next.html#section"
        );
        assert!(!monitor.has_confirmed());
        assert!(monitor.take()?.is_none());
        Ok(())
    }

    #[test]
    fn confirmed_navigation_falls_back_to_the_network_request() -> Result<(), String> {
        let monitor = monitor(NavigationState {
            requested_target: Some("not a url".to_string()),
            ..NavigationState::default()
        });

        monitor.confirm("https://example.test/form-target");

        let navigation = monitor.take()?.ok_or("navigation was not captured")?;
        assert_eq!(navigation.url.as_str(), "https://example.test/form-target");
        Ok(())
    }

    #[test]
    fn popup_navigation_requires_both_window_request_and_closed_target() -> Result<(), String> {
        let monitor = monitor(NavigationState::default());

        monitor.record_popup("https://example.test/new-tab".to_string());
        assert!(monitor.has_pending_popup());
        assert!(!monitor.has_confirmed());
        monitor.confirm_closed_popups(1);

        let navigation = monitor.take()?.ok_or("navigation was not captured")?;
        assert_eq!(navigation.url.as_str(), "https://example.test/new-tab");
        assert!(!monitor.has_pending_popup());
        Ok(())
    }

    #[test]
    fn popup_pairing_handles_target_event_before_window_open_and_ignores_blank() {
        let monitor = monitor(NavigationState::default());

        monitor.confirm_closed_popups(2);
        monitor.record_popup("about:blank".to_string());
        monitor.record_popup("https://example.test/after-blank".to_string());

        assert!(monitor.has_confirmed());
        assert_eq!(
            monitor
                .take()
                .ok()
                .flatten()
                .map(|event| event.url.as_str().to_string()),
            Some("https://example.test/after-blank".to_string())
        );
    }

    #[test]
    fn take_rejects_an_invalid_confirmed_target() {
        let monitor = monitor(NavigationState {
            confirmed_targets: VecDeque::from(["not a url".to_string()]),
            ..NavigationState::default()
        });

        assert!(monitor.take().is_err());
    }

    #[test]
    fn poisoned_navigation_state_is_recovered() {
        let state = Arc::new(Mutex::new(None::<String>));
        let poisoned = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = lock(&poisoned);
            std::panic::resume_unwind(Box::new("poison navigation state"));
        })
        .join();

        *lock(&state) = Some("recovered".to_string());

        assert_eq!(lock(&state).as_deref(), Some("recovered"));
    }

    #[test]
    fn string_error_preserves_navigation_errors() {
        assert_eq!(string_error("navigation failed"), "navigation failed");
    }
}
