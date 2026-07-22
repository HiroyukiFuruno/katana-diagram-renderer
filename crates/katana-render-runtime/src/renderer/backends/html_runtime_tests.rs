use super::StaticHtmlRuntime;
use super::html_runtime::HtmlRuntimeDispatch;
use super::{HtmlNodeId, HtmlRenderInput, HtmlRenderer, HtmlRuntimeError, HtmlRuntimeEvent};

type TestResult<T = ()> = Result<T, String>;

const HTML_RUNTIME_HOST_BINDING_SOURCE: &str = concat!(
    include_str!("html_runtime/bridge.rs"),
    include_str!("html_runtime/script.rs"),
    include_str!("html_runtime/dom_bootstrap.js"),
);
const DOM_EVENT_CONTRACT_DOCUMENT: &str = r#"<div id=parent><a id=link href="/next" onclick="document.getElementById('status').textContent += 'inline|'">Open</a></div><p id=status></p><script>
const parent = document.getElementById('parent');
const link = document.getElementById('link');
const status = document.getElementById('status');
const removed = () => { status.textContent += 'removed|'; };
window.addEventListener('click', (event) => { status.textContent += `window:${event.eventPhase}:${event.currentTarget === window}|`; }, true);
document.addEventListener('click', (event) => { status.textContent += `document:${event.eventPhase}:${event.currentTarget === document}|`; }, true);
parent.addEventListener('click', (event) => { status.textContent += `parent:${event.eventPhase}:${event.target === link}|`; }, true);
link.addEventListener('click', removed);
link.removeEventListener('click', removed);
link.addEventListener('click', () => { status.textContent += 'once|'; }, { once: true });
link.addEventListener('click', (event) => {
  status.textContent += `target:${event.eventPhase}:${event.target === link}:${event.currentTarget === link}|`;
  event.preventDefault();
  event.stopImmediatePropagation();
});
link.addEventListener('click', () => { status.textContent += 'after-immediate|'; });
parent.addEventListener('click', () => { status.textContent += 'bubble|'; });
</script>"#;

#[test]
fn rejects_execution_timeouts_without_a_partial_snapshot() {
    let timeout = HtmlRenderer.render(&HtmlRenderInput {
        source: "<p>visible</p><script>for (;;) {}</script>".to_string(),
    });

    assert!(matches!(timeout, Err(HtmlRuntimeError::ExecutionTimeout)));
}

#[test]
fn reports_javascript_compile_errors() -> TestResult {
    let error = render("<p>visible</p><script>const = ;</script>")
        .err()
        .ok_or_else(|| "invalid JavaScript rendered unexpectedly".to_string())?;
    assert!(error.contains("JavaScript exception"), "{error}");
    assert!(error.contains("inline-script:1:"), "{error}");
    assert!(error.contains("const = ;"), "{error}");
    Ok(())
}

#[test]
fn keeps_host_io_capabilities_unavailable_to_html_scripts() -> TestResult {
    let output = render(
        "<p>visible</p><script>for (const capability of ['fetch', 'XMLHttpRequest', 'WebSocket', 'require', 'process', 'Deno', 'Bun']) { if (globalThis[capability] !== undefined) throw new Error(capability); }</script>",
    )?;

    assert_eq!(output, "<p>visible</p>");
    for binding in [
        "fetch",
        "XMLHttpRequest",
        "WebSocket",
        "require",
        "process",
        "Deno",
        "Bun",
    ] {
        assert!(
            !HTML_RUNTIME_HOST_BINDING_SOURCE.contains(binding),
            "HTML runtime source exposes host binding: {binding}"
        );
    }
    Ok(())
}

#[test]
fn accepts_standard_event_names_and_rejects_non_listener_values() {
    assert!(
        render("<button id=action>Run</button><script>document.getElementById('action').addEventListener('mouseover', () => {});</script>")
            .is_ok()
    );
    assert!(matches!(
        render("<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', 'not a function');</script>"),
        Err(message) if message.contains("Event listener must be a function or EventListener object")
    ));
}

#[test]
fn reports_dom_bridge_errors_from_native_callbacks() {
    assert!(matches!(
        render("<div id=host></div><script>document.getElementById('host').appendChild({});</script>"),
        Err(message) if message.contains("invalid HTML node id")
    ));
    assert!(matches!(
        render("<button id=action>Run</button><script>__krr_dom('eventPath', 'not-a-node');</script>"),
        Err(message) if message.contains("invalid HTML node id: not-a-node")
    ));
}

#[test]
fn invalid_click_target_reports_dom_bridge_error() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start("<button id=action>Run</button>")
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;

    let dispatch = session.dispatch(HtmlRuntimeEvent::Click {
        target: HtmlNodeId(9999),
    });

    assert!(matches!(dispatch, Err(HtmlRuntimeError::DomBridge(_))));
    Ok(())
}

#[test]
fn returns_navigation_intent_without_a_click_handler() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start("<a id=link href=\"/next\">Open</a>")
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let link = session
        .node_for_element_id("link")
        .ok_or_else(|| "link must have a stable node id".to_string())?;

    let dispatch = session
        .dispatch(HtmlRuntimeEvent::Click { target: link })
        .map_err(|error| format!("click must dispatch: {error}"))?;

    assert_eq!(
        dispatch.navigation.map(|intent| intent.href),
        Some("/next".to_string())
    );
    Ok(())
}

#[test]
fn dom_event_options_order_cancellation_and_default_action_follow_the_event_path() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start(DOM_EVENT_CONTRACT_DOCUMENT)
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let link = session
        .node_for_element_id("link")
        .ok_or_else(|| "link must have a stable node id".to_string())?;

    let first = session
        .dispatch(HtmlRuntimeEvent::Click { target: link })
        .map_err(|error| format!("first click must dispatch: {error}"))?;
    let second = session
        .dispatch(HtmlRuntimeEvent::Click { target: link })
        .map_err(|error| format!("second click must dispatch: {error}"))?;
    let snapshot = session.snapshot().map_err(|error| error.to_string())?;
    assert_dom_event_contract(&first, &second, &snapshot);
    Ok(())
}

fn assert_dom_event_contract(
    first: &HtmlRuntimeDispatch,
    second: &HtmlRuntimeDispatch,
    snapshot: &str,
) {
    assert!(first.navigation.is_none());
    assert!(second.navigation.is_none());
    let expected = "window:1:true|document:1:true|parent:1:true|once|target:2:true:true|window:1:true|document:1:true|parent:1:true|target:2:true:true|";
    assert!(
        snapshot.contains(&format!(r#"<p id="status">{expected}</p>"#)),
        "{snapshot}"
    );
    for forbidden in ["removed|", "after-immediate|", "bubble|"] {
        assert!(!snapshot.contains(forbidden), "{snapshot}");
    }
}

#[test]
fn returns_v8_exceptions_from_click_handlers() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start(
            "<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', () => { throw new Error('listener'); });</script>",
        )
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let action = session
        .node_for_element_id("action")
        .ok_or_else(|| "action must have a stable node id".to_string())?;

    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });

    assert!(
        matches!(dispatch, Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("listener"))
    );
    Ok(())
}

#[test]
fn returns_v8_exceptions_from_inline_click_handlers() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start("<button id=action onclick=\"throw new Error('inline')\">Run</button>")
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let action = session
        .node_for_element_id("action")
        .ok_or_else(|| "action must have a stable node id".to_string())?;

    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });

    assert!(
        matches!(dispatch, Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("inline"))
    );
    Ok(())
}

#[test]
fn inline_click_handler_timeout_discards_the_v8_session() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start("<button id=action onclick=\"for (;;) {}\">Run</button>")
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let action = session
        .node_for_element_id("action")
        .ok_or_else(|| "action must have a stable node id".to_string())?;

    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });

    assert!(matches!(dispatch, Err(HtmlRuntimeError::ExecutionTimeout)));
    assert!(session.node_for_element_id("action").is_none());
    assert!(
        matches!(session.snapshot(), Err(HtmlRuntimeError::DomBridge(message)) if message.contains("discarded"))
    );
    Ok(())
}

#[test]
fn discards_the_v8_session_after_event_timeout() -> TestResult {
    let mut session = StaticHtmlRuntime
        .start(
            "<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', () => { for (;;) {} });</script>",
        )
        .map_err(|error| format!("HTML runtime session must start: {error}"))?;
    let action = session
        .node_for_element_id("action")
        .ok_or_else(|| "action must have a stable node id".to_string())?;

    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });

    assert!(matches!(dispatch, Err(HtmlRuntimeError::ExecutionTimeout)));
    assert!(session.node_for_element_id("action").is_none());
    assert!(
        matches!(session.snapshot(), Err(HtmlRuntimeError::DomBridge(message)) if message.contains("discarded"))
    );
    Ok(())
}

#[test]
fn formats_each_html_runtime_error() {
    assert_eq!(
        HtmlRuntimeError::ExternalScript("app.js".to_string()).to_string(),
        "external script is not supported: app.js"
    );
    assert_eq!(
        HtmlRuntimeError::JavaScriptException("boom".to_string()).to_string(),
        "JavaScript exception: boom"
    );
    assert_eq!(
        HtmlRuntimeError::DomBridge("invalid node".to_string()).to_string(),
        "HTML DOM bridge error: invalid node"
    );
    assert_eq!(
        HtmlRuntimeError::ExecutionTimeout.to_string(),
        "JavaScript execution timed out"
    );
}

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
