use super::StaticHtmlRuntime;
use super::{HtmlNodeId, HtmlRenderInput, HtmlRenderer, HtmlRuntimeError, HtmlRuntimeEvent};

type TestResult<T = ()> = Result<T, String>;

const HTML_RUNTIME_HOST_BINDING_SOURCE: &str = concat!(
    include_str!("html_runtime/bridge.rs"),
    include_str!("html_runtime/script.rs"),
);

#[test]
fn rejects_execution_timeouts_without_a_partial_snapshot() {
    let timeout = HtmlRenderer.render(&HtmlRenderInput {
        source: "<p>visible</p><script>for (;;) {}</script>".to_string(),
    });

    assert!(matches!(timeout, Err(HtmlRuntimeError::ExecutionTimeout)));
}

#[test]
fn reports_javascript_compile_errors() {
    assert!(matches!(
        render("<p>visible</p><script>const = ;</script>"),
        Err(message) if message.contains("JavaScript exception")
    ));
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
fn rejects_unsupported_event_listener_contracts() {
    assert!(matches!(
        render("<button id=action>Run</button><script>document.getElementById('action').addEventListener('mouseover', () => {});</script>"),
        Err(message) if message.contains("Unsupported event listener")
    ));
    assert!(matches!(
        render("<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', 'not a function');</script>"),
        Err(message) if message.contains("Unsupported event listener")
    ));
}

#[test]
fn reports_dom_bridge_errors_from_native_callbacks() {
    assert!(matches!(
        render("<div id=host></div><script>document.getElementById('host').appendChild({});</script>"),
        Err(message) if message.contains("invalid HTML node id")
    ));
    assert!(matches!(
        render("<button id=action>Run</button><script>__krr_dom('addEventListener', 'not-a-node', 'click', () => {});</script>"),
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
