use super::super::html_dom_helpers::collect_scripts;
use super::HtmlDocument;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use std::collections::HashMap;

impl HtmlDocument {
    pub(crate) fn parse(source: &str) -> Self {
        let parsed = parse_document(RcDom::default(), Default::default()).one(source.to_string());
        let mut document = Self {
            document: parsed.document,
            nodes: HashMap::new(),
            node_ids: HashMap::new(),
            next_node_id: 1,
        };
        document.register_subtree(&document.document.clone());
        document
    }

    pub(crate) fn render(&self) -> String {
        super::super::html_snapshot::render_document(&self.document)
    }

    pub(crate) fn inline_scripts(&self) -> Result<Vec<String>, String> {
        let mut scripts = Vec::new();
        collect_scripts(&self.document, &mut scripts)?;
        Ok(scripts)
    }
}
