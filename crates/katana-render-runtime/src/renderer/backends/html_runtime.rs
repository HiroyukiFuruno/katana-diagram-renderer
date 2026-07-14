mod bridge;
mod dom_state;
mod execution;
#[cfg(test)]
mod interaction;
mod script;
mod session;
mod style;
mod types;

pub(crate) use session::StaticHtmlRuntime;
pub use types::HtmlRuntimeError;
#[cfg(test)]
pub(crate) use types::{HtmlNodeId, HtmlRuntimeEvent};
