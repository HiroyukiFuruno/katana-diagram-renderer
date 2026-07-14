#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlRuntimeError {
    ExternalScript(String),
    JavaScriptException(String),
    DomBridge(String),
    ExecutionTimeout,
}

impl std::fmt::Display for HtmlRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExternalScript(source) => {
                write!(formatter, "external script is not supported: {source}")
            }
            Self::JavaScriptException(message) => {
                write!(formatter, "JavaScript exception: {message}")
            }
            Self::DomBridge(message) => write!(formatter, "HTML DOM bridge error: {message}"),
            Self::ExecutionTimeout => write!(formatter, "JavaScript execution timed out"),
        }
    }
}

impl std::error::Error for HtmlRuntimeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg(test)]
pub struct HtmlNodeId(pub(crate) u64);

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct HtmlNavigationIntent {
    pub href: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub enum HtmlRuntimeEvent {
    Click { target: HtmlNodeId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct HtmlRuntimeDispatch {
    pub content: String,
    pub navigation: Option<HtmlNavigationIntent>,
}

pub(super) enum DomValue {
    Undefined,
    Null,
    String(String),
    NodeId(u64),
}
