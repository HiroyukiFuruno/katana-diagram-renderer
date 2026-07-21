mod document;
mod policy;
#[cfg(test)]
mod tests;
mod transport;

use super::html_browser::HtmlBrowserSource;
use super::html_document::HtmlDocument;
pub(super) use policy::HtmlSubresourcePolicy;

pub(super) struct HtmlDocumentResources {
    pub(super) stylesheets: std::collections::HashMap<String, String>,
    pub(super) scripts: Vec<String>,
}

pub(super) struct HtmlSubresourceLoader {
    policy: HtmlSubresourcePolicy,
    document_origin: String,
}

impl HtmlSubresourceLoader {
    pub(super) fn new(source: &HtmlBrowserSource) -> Self {
        Self {
            policy: HtmlSubresourcePolicy::from_source(source),
            document_origin: source.origin.as_str().to_owned(),
        }
    }

    pub(super) fn load(
        &self,
        document: &mut HtmlDocument,
    ) -> Result<HtmlDocumentResources, String> {
        document::load_document_resources(self, document)
    }

    pub(super) fn load_text(&self, reference: &str) -> Result<String, String> {
        let url = self.policy.resolve_subresource(reference)?;
        transport::load_text(&url)
    }

    pub(super) fn load_image_data_url(&self, reference: &str) -> Result<String, String> {
        let url = self.policy.resolve_subresource(reference)?;
        transport::load_image_data_url(&url)
    }

    pub(super) fn document_origin(&self) -> &str {
        &self.document_origin
    }
}
