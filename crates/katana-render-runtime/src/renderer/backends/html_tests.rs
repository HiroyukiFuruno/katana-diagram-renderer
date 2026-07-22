use super::{HtmlRenderInput, HtmlRenderer, HtmlRuntimeError};

type TestResult<T = ()> = Result<T, String>;

#[test]
fn renders_static_css_and_excludes_document_metadata() -> TestResult {
    let output = render(
        r#"<!doctype html>
<html><head><title>Hidden metadata</title><style>
body { color: red; }
.note { font-weight: bold; }
#card { color: blue; font-style: italic; }
</style><script>window.bad = true;</script></head>
<body><p class="note">Visible</p><p id="card" style="color: green">Card</p></body></html>"#,
    )?;

    assert!(
        output.contains(r#"<p class="note" style="color: red; font-weight: bold">Visible</p>"#)
    );
    assert!(output.contains(r#"<p id="card" style="color: green; font-style: italic">Card</p>"#));
    for hidden in ["Hidden metadata", "window.bad", "<script", "<style"] {
        assert!(!output.contains(hidden), "{output}");
    }
    Ok(())
}

#[test]
fn keeps_more_specific_css_when_a_later_rule_is_weaker() -> TestResult {
    let output = render(
        "<style>#card { color: blue; } .note { color: red; }</style><p id=card class=note>Card</p>",
    )?;

    assert!(output.contains(r#"style="color: blue""#), "{output}");
    Ok(())
}

#[test]
fn parses_malformed_html_and_normalizes_tables() -> TestResult {
    let output = render(
        "<main><p class=note>One<p>Two</p><table><tr><th>A</th><th>B</th></tr><tr><td>x</td><td>y</td></tr></table></main>",
    )?;

    assert!(output.contains("<p class=\"note\">One</p>"), "{output}");
    assert!(output.contains("<p>Two</p>"), "{output}");
    assert!(
        output.contains("| A | B |\n| --- | --- |\n| x | y |"),
        "{output}"
    );
    Ok(())
}

#[test]
fn evaluates_inline_scripts_against_the_html5_dom() -> TestResult {
    let output = render(
        "<style>#state { color: red; }</style><p id=state>Static</p><script>const state = document.getElementById('state'); state.textContent = 'mutated'; state.style.color = 'blue';</script>",
    )?;

    assert!(output.contains("mutated"), "{output}");
    assert!(output.contains(r#"style="color: blue""#), "{output}");
    Ok(())
}

#[test]
fn evaluates_inline_scripts_in_document_order() -> TestResult {
    let output = render(
        "<p id=state>Initial</p><script>document.getElementById('state').textContent = 'First';</script><script>const state = document.getElementById('state'); if (state.textContent !== 'First') throw new Error('order'); state.textContent = 'Second';</script>",
    )?;

    assert!(output.contains("Second"), "{output}");
    assert!(!output.contains("First"), "{output}");
    Ok(())
}

#[test]
fn evaluates_dom_creation_query_and_removal() -> TestResult {
    let output = render(
        "<p id=root></p><p id=obsolete>old</p><style>.note { font-weight: bold; }</style><script>const root = document.getElementById('root'); const note = document.createElement('span'); note.className = 'note'; note.textContent = 'new'; root.appendChild(note); document.querySelector('.note').style.color = 'purple'; document.getElementById('obsolete').remove();</script>",
    )?;

    assert!(
        output
            .contains(r#"<span class="note" style="font-weight: bold; color: purple">new</span>"#),
        "{output}"
    );
    assert!(!output.contains("obsolete"), "{output}");
    assert!(!output.contains(">old<"), "{output}");
    Ok(())
}

#[test]
fn evaluates_dom_getters_and_missing_selectors() -> TestResult {
    let output = render(
        "<p id=state class=original style=\"color: blue\">Visible</p><script>const state = document.getElementById('state'); if (state.textContent !== 'Visible') throw new Error('text'); if (state.getAttribute('class') !== 'original') throw new Error('class'); if (state.style.color !== 'blue') throw new Error('style'); if (document.querySelector('.missing') !== null) throw new Error('selector'); state.setAttribute('title', 'updated'); state.style.backgroundColor = 'black';</script>",
    )?;

    assert!(
        output.contains(r#"style="color: blue; background-color: black""#),
        "{output}"
    );
    assert!(output.contains(r#"title="updated""#), "{output}");
    Ok(())
}

#[test]
fn evaluates_supported_query_selector_forms_and_empty_selectors() -> TestResult {
    let output = render(
        "<p id=state class=note>Visible</p><script>const byId = document.querySelector('#state'); const byTag = document.querySelector('p'); const byTagClass = document.querySelector('p.note'); if (!byId || !byTag || !byTagClass) throw new Error('selector'); if (document.querySelector('') !== null) throw new Error('empty selector'); byTagClass.textContent = byId.textContent + byTag.textContent;</script>",
    )?;

    assert!(output.contains("VisibleVisible"), "{output}");
    Ok(())
}

#[test]
fn rejects_invalid_dynamic_element_names() {
    assert_dom_bridge_error(
        "document.createElement('bad-tag')",
        "unsupported element name",
    );
}

#[test]
fn rejects_empty_dynamic_attribute_names() {
    assert_dom_bridge_error(
        "document.createElement('span').setAttribute('', 'value')",
        "attribute name is empty",
    );
}

#[test]
fn overwrites_existing_style_properties_using_dom_property_names() -> TestResult {
    let output = render(
        "<p id=state>Visible</p><script>const state = document.getElementById('state'); state.style.Color = 'red'; state.style.Color = 'green'; if (state.style.Color !== 'green') throw new Error('style');</script>",
    )?;

    assert!(output.contains(r#"style="color: green""#), "{output}");
    Ok(())
}

#[test]
fn applies_inner_html_without_executing_dynamically_inserted_scripts() -> TestResult {
    let output = render(
        "<div id=card>old</div><script>const card = document.getElementById('card'); card.innerHTML = '<span class=\"note\">new</span><script>throw new Error(\"dynamic\")<\\/script>'; if (!card.innerHTML.includes('new')) throw new Error('innerHTML');</script>",
    )?;

    assert!(
        output.contains(r#"<span class="note">new</span>"#),
        "{output}"
    );
    assert!(!output.contains("dynamic"), "{output}");
    assert!(!output.contains("old"), "{output}");
    Ok(())
}

#[test]
fn applies_inner_html_from_a_document_body_fragment() -> TestResult {
    let output = render(
        "<div id=card>old</div><script>document.getElementById('card').innerHTML = '<body><span class=note>replacement</span></body>';</script>",
    )?;

    assert!(
        output.contains(r#"<span class="note">replacement</span>"#),
        "{output}"
    );
    assert!(!output.contains("old"), "{output}");
    Ok(())
}

#[test]
fn rejects_invalid_native_dom_operations() {
    assert_dom_bridge_error("__krr_dom('unknownOperation')", "unknownOperation");
    assert_dom_bridge_error("__krr_dom('eventPath', 'invalid')", "invalid HTML node id");
    assert_dom_bridge_error("__krr_dom('getElementById')", "missing DOM argument");
    assert_dom_bridge_error(
        "__krr_dom('textContent', 'invalid')",
        "invalid HTML node id",
    );
}

#[test]
fn rejects_scripts_that_cannot_be_evaluated_without_a_partial_snapshot() {
    let external = HtmlRenderer.render(&HtmlRenderInput {
        source: "<p>visible</p><script src=\"https://example.invalid/app.js\"></script>"
            .to_string(),
    });
    assert!(
        matches!(external, Err(HtmlRuntimeError::ExternalScript(source)) if source.contains("example.invalid"))
    );

    let exception = HtmlRenderer.render(&HtmlRenderInput {
        source: "<p>visible</p><script>throw new Error('boom')</script>".to_string(),
    });
    assert!(
        matches!(exception, Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("boom"))
    );
}

#[test]
fn ignores_unsupported_css_and_keeps_void_elements() -> TestResult {
    let output = render(
        r#"<style>
/* supported comments are discarded */
p span { color: blue; }
* { color: red; }
p. { color: red; }
.note.note { color: red; }
.note { border: 1px; text-align: ; }
</style><img alt="Preview" src="cover.png"><p class="note">Visible</p>"#,
    )?;

    assert!(
        output.contains(r#"<img alt="Preview" src="cover.png">"#),
        "{output}"
    );
    assert!(
        output.contains(r#"<p class="note">Visible</p>"#),
        "{output}"
    );
    Ok(())
}

#[test]
fn ignores_unclosed_css_comments_and_normalizes_empty_tables() -> TestResult {
    let output = render("<style>/* unfinished</style><table></table><p>Visible</p>")?;

    assert_eq!(output, "<p>Visible</p>");
    Ok(())
}

#[test]
fn pads_uneven_html_table_rows() -> TestResult {
    let output =
        render("<table><tr><th>Feature</th><th>Status</th></tr><tr><td>HTML</td></tr></table>")?;

    assert_eq!(output, "| Feature | Status |\n| --- | --- |\n| HTML |  |");
    Ok(())
}

fn render(source: &str) -> TestResult<String> {
    HtmlRenderer
        .render(&HtmlRenderInput {
            source: source.to_string(),
        })
        .map(|output| output.content)
        .map_err(|error| format!("HTML runtime must render test fixture: {error}"))
}

fn assert_dom_bridge_error(script: &str, message: &str) {
    let result = HtmlRenderer.render(&HtmlRenderInput {
        source: format!("<p>visible</p><script>{script}</script>"),
    });
    assert!(matches!(result, Err(HtmlRuntimeError::DomBridge(error)) if error.contains(message)));
}
