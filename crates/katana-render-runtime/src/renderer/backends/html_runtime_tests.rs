#[path = "html_runtime_tests_execution.rs"]
mod execution;
#[path = "html_runtime_tests_interaction.rs"]
mod interaction;
use crate::renderer::backends::{HtmlRenderInput, HtmlRenderer};

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

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}
