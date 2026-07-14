use super::html_runtime::{HtmlRuntimeError, StaticHtmlRuntime};

/// Input for the HTML DOM runtime.
#[derive(Debug, Clone)]
pub struct HtmlRenderInput {
    pub source: String,
}

/// HTML content prepared for a neutral document viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlRenderOutput {
    /// Visible document content with CSS resolved into inline styles.
    pub content: String,
}

/// Static HTML export renderer for neutral document viewer conversion.
#[derive(Debug, Clone, Default)]
pub struct HtmlRenderer;

impl HtmlRenderer {
    pub fn render(&self, input: &HtmlRenderInput) -> Result<HtmlRenderOutput, HtmlRuntimeError> {
        StaticHtmlRuntime
            .render(&input.source)
            .map(|content| HtmlRenderOutput { content })
    }
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
