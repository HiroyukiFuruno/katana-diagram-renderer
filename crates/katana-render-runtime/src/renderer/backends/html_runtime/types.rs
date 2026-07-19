#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlRuntimeError {
    ExternalScript(String),
    Subresource(String),
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
            Self::Subresource(message) => write!(formatter, "HTML subresource error: {message}"),
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
pub struct HtmlNodeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HtmlRuntimeEventKind {
    Click,
    Input,
    Toggle,
}

impl HtmlRuntimeEventKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "click" => Some(Self::Click),
            "input" => Some(Self::Input),
            "toggle" => Some(Self::Toggle),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::Input => "input",
            Self::Toggle => "toggle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlNavigationIntent {
    pub href: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlRuntimeEvent {
    Click { target: HtmlNodeId },
    Input { target: HtmlNodeId },
    Toggle { target: HtmlNodeId },
}

impl HtmlRuntimeEvent {
    pub(crate) fn kind(self) -> HtmlRuntimeEventKind {
        match self {
            Self::Click { .. } => HtmlRuntimeEventKind::Click,
            Self::Input { .. } => HtmlRuntimeEventKind::Input,
            Self::Toggle { .. } => HtmlRuntimeEventKind::Toggle,
        }
    }

    pub(crate) fn target(self) -> HtmlNodeId {
        match self {
            Self::Click { target } | Self::Input { target } | Self::Toggle { target } => target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
