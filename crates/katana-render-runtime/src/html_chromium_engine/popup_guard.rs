use anyhow::Context;
use headless_chrome::{
    browser::transport::Transport,
    protocol::cdp::{Target, types::Event},
};
use std::{
    collections::HashSet,
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
use url::Url;

const POPUP_GUARD_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const POPUP_TARGET_TIMEOUT: Duration = Duration::from_secs(5);
const POPUP_TARGET_MISSING: &str = "popup guard did not receive a paused browser target";

pub(super) struct PopupGuard {
    transport: Transport,
    events: Receiver<Event>,
    main_target_id: Target::TargetID,
    initial_target_ids: HashSet<Target::TargetID>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetDisposition {
    Ignored,
    Main,
    Existing,
    Closed,
}

impl PopupGuard {
    pub(super) fn install(
        debug_ws_url: &str,
        main_target_id: &str,
        initial_target_ids: HashSet<Target::TargetID>,
    ) -> anyhow::Result<Self> {
        let debug_ws_url = parse_debug_ws_url(debug_ws_url)?;
        let transport = connect_popup_transport(debug_ws_url)?;
        let events = transport.listen_to_browser_events();
        let mut guard = Self {
            transport,
            events,
            main_target_id: main_target_id.to_string(),
            initial_target_ids,
        };
        guard.enable_auto_attach()?;
        guard.resume_main_target()?;
        Ok(guard)
    }

    fn enable_auto_attach(&self) -> anyhow::Result<()> {
        self.transport
            .call_method_on_browser(Target::SetAutoAttach {
                auto_attach: true,
                wait_for_debugger_on_start: true,
                flatten: Some(true),
                filter: Some(page_target_filter()),
            })
            .map(|_| ())
    }

    pub(super) fn close_new_targets(&self, wait_for_popup: bool) -> anyhow::Result<usize> {
        let mut closed = 0;
        let deadline = Instant::now() + POPUP_TARGET_TIMEOUT;
        loop {
            let event = match try_receive_event(&self.events)? {
                Some(event) => event,
                None if wait_for_popup && closed == 0 => recv_until(&self.events, deadline)?,
                None => return Ok(closed),
            };
            if self.handle_event_target(event)? == TargetDisposition::Closed {
                closed += 1;
            }
        }
    }

    fn resume_main_target(&mut self) -> anyhow::Result<()> {
        let deadline = Instant::now() + POPUP_TARGET_TIMEOUT;
        loop {
            let event = recv_until(&self.events, deadline)?;
            if self.handle_event_target(event)? == TargetDisposition::Main {
                return Ok(());
            }
        }
    }

    fn handle_event_target(&self, event: Event) -> anyhow::Result<TargetDisposition> {
        let Event::AttachedToTarget(event) = event else {
            return Ok(TargetDisposition::Ignored);
        };
        if self
            .initial_target_ids
            .contains(&event.params.target_info.target_id)
        {
            ensure_initial_target_running(event.params.waiting_for_debugger)?;
            return if event.params.target_info.target_id == self.main_target_id {
                Ok(TargetDisposition::Main)
            } else {
                Ok(TargetDisposition::Existing)
            };
        }
        self.close_target(&event.params.target_info.target_id)?;
        Ok(TargetDisposition::Closed)
    }

    fn close_target(&self, target_id: &Target::TargetID) -> anyhow::Result<()> {
        let close_target = Target::CloseTarget {
            target_id: target_id.clone(),
        };
        let result = self.transport.call_method_on_browser(close_target);
        let result = result.context("failed to close paused popup target")?;
        ensure_target_closed(result.success)
    }
}

impl Drop for PopupGuard {
    fn drop(&mut self) {
        self.transport.shutdown();
    }
}

fn page_target_filter() -> Target::TargetFilter {
    vec![Target::FilterEntry {
        exclude: None,
        Type: Some("page".to_string()),
    }]
}

fn parse_debug_ws_url(debug_ws_url: &str) -> anyhow::Result<Url> {
    Ok(Url::parse(debug_ws_url)?)
}

fn connect_popup_transport(debug_ws_url: Url) -> anyhow::Result<Transport> {
    Transport::new(debug_ws_url, None, POPUP_GUARD_IDLE_TIMEOUT, None)
}

fn try_receive_event(events: &Receiver<Event>) -> anyhow::Result<Option<Event>> {
    match events.try_recv() {
        Ok(event) => Ok(Some(event)),
        Err(TryRecvError::Empty) => Ok(None),
        Err(TryRecvError::Disconnected) => {
            Err(anyhow::anyhow!("popup guard DevTools connection closed"))
        }
    }
}

fn recv_until(events: &Receiver<Event>, deadline: Instant) -> anyhow::Result<Event> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    events.recv_timeout(remaining).context(POPUP_TARGET_MISSING)
}

fn ensure_initial_target_running(waiting_for_debugger: bool) -> anyhow::Result<()> {
    if waiting_for_debugger {
        Err(anyhow::anyhow!(
            "Chromium unexpectedly paused an existing browser target"
        ))
    } else {
        Ok(())
    }
}

fn ensure_target_closed(success: bool) -> anyhow::Result<()> {
    if success {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Chromium refused to close a paused popup target"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::{TcpListener, TcpStream},
        sync::mpsc::{self, RecvTimeoutError},
    };

    #[derive(Clone, Copy)]
    enum TestTransportBehavior {
        Disconnect,
        CloseTargetSuccess,
    }

    fn must_error(result: anyhow::Result<()>) -> anyhow::Error {
        match result {
            Ok(_) => fail("expected popup guard error".to_string()),
            Err(error) => error,
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    fn test_popup_guard(behavior: TestTransportBehavior) -> anyhow::Result<PopupGuard> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        std::thread::spawn(move || -> anyhow::Result<()> {
            let (stream, _) = listener.accept()?;
            let mut socket = tungstenite::accept(stream)?;
            match behavior {
                TestTransportBehavior::Disconnect => {}
                TestTransportBehavior::CloseTargetSuccess => respond_to_close_target(&mut socket)?,
            }
            Ok(())
        });
        let transport = Transport::new(
            Url::parse(&format!("ws://{address}"))?,
            None,
            Duration::from_secs(1),
            None,
        );
        let transport = transport?;
        let (_sender, events) = mpsc::channel();
        Ok(PopupGuard {
            transport,
            events,
            main_target_id: "main".to_string(),
            initial_target_ids: HashSet::new(),
        })
    }

    fn respond_to_close_target(
        socket: &mut tungstenite::WebSocket<TcpStream>,
    ) -> anyhow::Result<()> {
        let request = socket.read()?;
        let request: serde_json::Value = serde_json::from_str(request.to_text()?)?;
        anyhow::ensure!(request["method"] == "Target.closeTarget");
        anyhow::ensure!(request["params"]["targetId"] == "popup");
        let id = close_target_request_id(&request)?;
        let response = serde_json::json!({"id": id, "result": {"success": true}}).to_string();
        socket.send(tungstenite::Message::Text(response.into()))?;
        Ok(())
    }

    fn close_target_request_id(request: &serde_json::Value) -> anyhow::Result<u64> {
        match request["id"].as_u64() {
            Some(id) => Ok(id),
            None => Err(anyhow::anyhow!("close target request id is missing")),
        }
    }

    fn attached_target_event(target_id: &str) -> Event {
        Event::AttachedToTarget(Target::events::AttachedToTargetEvent {
            params: Target::events::AttachedToTargetEventParams {
                session_id: format!("session-{target_id}"),
                target_info: Target::TargetInfo {
                    target_id: target_id.to_string(),
                    Type: "page".to_string(),
                    title: String::new(),
                    url: "about:blank".to_string(),
                    attached: true,
                    opener_id: None,
                    can_access_opener: false,
                    opener_frame_id: None,
                    parent_frame_id: None,
                    browser_context_id: None,
                    subtype: None,
                },
                waiting_for_debugger: false,
            },
        })
    }

    #[test]
    fn popup_guard_uses_only_page_targets() {
        assert_eq!(
            page_target_filter(),
            vec![Target::FilterEntry {
                exclude: None,
                Type: Some("page".to_string()),
            }]
        );
    }

    #[test]
    fn popup_guard_reports_invalid_and_refused_devtools_endpoints() -> anyhow::Result<()> {
        assert!(parse_debug_ws_url("not a url").is_err());
        let refused = parse_debug_ws_url("ws://127.0.0.1:0/devtools/browser/test")?;
        assert!(connect_popup_transport(refused).is_err());
        Ok(())
    }

    #[test]
    fn popup_guard_shutdowns_its_transport_on_drop() -> anyhow::Result<()> {
        drop(test_popup_guard(TestTransportBehavior::Disconnect)?);

        Ok(())
    }

    #[test]
    fn popup_guard_closes_targets_over_the_browser_transport() -> anyhow::Result<()> {
        let guard = test_popup_guard(TestTransportBehavior::CloseTargetSuccess)?;
        guard.close_target(&"popup".to_string())
    }

    #[test]
    fn popup_guard_close_target_request_requires_a_numeric_id() -> anyhow::Result<()> {
        assert_eq!(close_target_request_id(&serde_json::json!({"id": 7}))?, 7);
        assert!(close_target_request_id(&serde_json::json!({})).is_err());
        Ok(())
    }

    #[test]
    fn popup_guard_resumes_main_after_existing_targets() -> anyhow::Result<()> {
        let mut guard = test_popup_guard(TestTransportBehavior::Disconnect)?;
        let (sender, events) = mpsc::channel();
        guard.events = events;
        guard.main_target_id = "main".to_string();
        guard.initial_target_ids = HashSet::from(["existing".to_string(), "main".to_string()]);
        sender.send(attached_target_event("existing"))?;
        sender.send(attached_target_event("main"))?;
        guard.resume_main_target()
    }

    #[test]
    fn popup_guard_preserves_close_transport_failures() -> anyhow::Result<()> {
        let guard = test_popup_guard(TestTransportBehavior::Disconnect)?;
        guard.transport.shutdown();
        let error = must_error(guard.close_target(&"popup".to_string()));
        assert_eq!(error.to_string(), "failed to close paused popup target");
        assert!(error.source().is_some());

        Ok(())
    }

    #[test]
    fn popup_guard_reports_event_timeout() {
        let (_sender, events) = mpsc::channel();
        let timeout = must_error(recv_until(&events, Instant::now()).map(|_| ()));
        assert_eq!(timeout.to_string(), POPUP_TARGET_MISSING);
        assert!(matches!(
            timeout.downcast_ref::<RecvTimeoutError>(),
            Some(RecvTimeoutError::Timeout)
        ));
    }

    #[test]
    fn popup_guard_receives_events_and_reports_an_empty_channel() -> anyhow::Result<()> {
        let (sender, events) = mpsc::channel();
        let event = Event::TargetCrashed(Target::events::TargetCrashedEvent {
            params: Target::events::TargetCrashedEventParams {
                target_id: "target".to_string(),
                status: String::new(),
                error_code: 0,
            },
        });
        sender.send(event.clone())?;
        assert!(matches!(
            try_receive_event(&events),
            Ok(Some(received)) if received == event
        ));
        assert!(matches!(try_receive_event(&events), Ok(None)));
        Ok(())
    }

    #[test]
    fn popup_guard_reports_disconnected_event_channels() {
        let (sender, events) = mpsc::channel::<Event>();
        drop(sender);
        let disconnected =
            must_error(recv_until(&events, Instant::now() + Duration::from_secs(1)).map(|_| ()));
        assert_eq!(disconnected.to_string(), POPUP_TARGET_MISSING);
        assert!(matches!(
            disconnected.downcast_ref::<RecvTimeoutError>(),
            Some(RecvTimeoutError::Disconnected)
        ));
        assert_eq!(
            must_error(try_receive_event(&events).map(|_| ())).to_string(),
            "popup guard DevTools connection closed"
        );
    }

    #[test]
    fn popup_guard_rejects_paused_initial_targets_and_failed_close() {
        assert!(ensure_initial_target_running(false).is_ok());
        assert_eq!(
            must_error(ensure_initial_target_running(true)).to_string(),
            "Chromium unexpectedly paused an existing browser target"
        );
        assert!(ensure_target_closed(true).is_ok());
        assert_eq!(
            must_error(ensure_target_closed(false)).to_string(),
            "Chromium refused to close a paused popup target"
        );
    }

    #[test]
    #[should_panic(expected = "expected popup guard error")]
    fn popup_guard_test_error_helper_rejects_success() {
        let _ = must_error(Ok(()));
    }
}
