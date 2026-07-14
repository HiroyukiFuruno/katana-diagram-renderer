use super::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserNavigationEvent, HtmlBrowserSource,
    HtmlBrowserViewport,
};
use serde::{Deserialize, Serialize};

pub const HTML_BROWSER_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum HtmlBrowserCommand {
    Load {
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    },
    Frame,
    Resize {
        viewport: HtmlBrowserViewport,
    },
    Input {
        input: HtmlBrowserInput,
    },
    Close,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HtmlBrowserRequest {
    pub protocol_version: u32,
    pub command: HtmlBrowserCommand,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum HtmlBrowserResponse {
    Frame {
        protocol_version: u32,
        frame: HtmlBrowserFrame,
    },
    Closed {
        protocol_version: u32,
    },
    Navigation {
        protocol_version: u32,
        navigation: HtmlBrowserNavigationEvent,
    },
    Error {
        protocol_version: u32,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HtmlBrowserSessionState {
    Active,
    Closed,
}
