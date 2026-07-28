use super::super::html_css_selector::CssSelector;
use super::{HtmlDocument, selector};

impl HtmlDocument {
    pub(crate) fn get_element_by_id(&mut self, id: &str) -> Option<u64> {
        let handle =
            super::super::html_dom_helpers::find_element(&self.document, |tag, attributes| {
                tag == "*"
                    || super::super::html_dom_helpers::attribute_value(attributes, "id") == Some(id)
            })?;
        Some(self.register_subtree(&handle))
    }

    pub(crate) fn query_selector(&mut self, selector: &str) -> Option<u64> {
        let selector = CssSelector::parse(selector)?;
        let handle = selector::find_selector(&self.document, &selector, &[])?;
        Some(self.register_subtree(&handle))
    }

    pub(crate) fn query_selector_all(&mut self, selector: &str) -> Vec<u64> {
        let Some(selector) = CssSelector::parse(selector) else {
            return Vec::new();
        };
        let mut handles = Vec::new();
        selector::collect_selectors(&self.document, &selector, &[], &mut handles);
        handles
            .iter()
            .map(|handle| self.register_subtree(handle))
            .collect()
    }

    pub(crate) fn query_selector_from(
        &mut self,
        node_id: u64,
        selector: &str,
    ) -> Result<Option<u64>, String> {
        let node = self.node(node_id)?;
        let Some(selector) = CssSelector::parse(selector) else {
            return Ok(None);
        };
        Ok(selector::find_descendant_selector(&node, &selector)
            .map(|handle| self.register_subtree(&handle)))
    }

    pub(crate) fn query_selector_all_from(
        &mut self,
        node_id: u64,
        selector: &str,
    ) -> Result<Vec<u64>, String> {
        let node = self.node(node_id)?;
        let Some(selector) = CssSelector::parse(selector) else {
            return Ok(Vec::new());
        };
        let mut handles = Vec::new();
        selector::collect_descendant_selectors(&node, &selector, &mut handles);
        Ok(handles
            .iter()
            .map(|handle| self.register_subtree(handle))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;

    #[test]
    fn query_selector_returns_none_for_invalid_selector_syntax() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<main><p id=target>Item</p></main>");
        let node = document
            .get_element_by_id("target")
            .ok_or("target node must exist")?;

        assert!(document.query_selector("main +").is_none());
        assert!(document.query_selector_from(node, "main +")?.is_none());
        assert!(document.query_selector_all("main +").is_empty());
        Ok(())
    }
}
