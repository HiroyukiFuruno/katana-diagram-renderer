use super::{HtmlDomBridgeState, argument, node_id};
use crate::renderer::backends::html_runtime::style::{
    kebab_case, property as style_property, set_property as set_style_property,
};
use crate::renderer::backends::html_runtime::types::DomValue;

impl HtmlDomBridgeState {
    pub(super) fn mutate_tree(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        match operation {
            "appendChild" => {
                let parent = node_id(argument(arguments, 0)?)?;
                let child = node_id(argument(arguments, 1)?)?;
                document.append_child(parent, child)?;
                Ok(DomValue::Undefined)
            }
            "remove" => {
                document.remove(node_id(argument(arguments, 0)?)?)?;
                Ok(DomValue::Undefined)
            }
            _ => Err(format!("unsupported HTML mutation operation: {operation}")),
        }
    }

    pub(super) fn mutate_content(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        let node_id = node_id(argument(arguments, 0)?)?;
        let value = argument(arguments, 1)?;
        match operation {
            "setTextContent" => document.set_text_content(node_id, value)?,
            "setInnerHTML" => document.set_inner_html(node_id, value)?,
            _ => return Err(format!("unsupported HTML content operation: {operation}")),
        }
        Ok(DomValue::Undefined)
    }

    pub(super) fn set_attribute(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        let node_id = node_id(argument(arguments, 0)?)?;
        let name = argument(arguments, 1)?;
        match operation {
            "setAttribute" => {
                self.document
                    .borrow_mut()
                    .set_attribute(node_id, name, argument(arguments, 2)?)?
            }
            "removeAttribute" => self.document.borrow_mut().remove_attribute(node_id, name)?,
            _ => return Err(format!("unsupported HTML attribute operation: {operation}")),
        }
        Ok(DomValue::Undefined)
    }

    pub(super) fn style(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        match operation {
            "styleGet" => {
                let node_id = node_id(argument(arguments, 0)?)?;
                let property = argument(arguments, 1)?;
                Ok(document
                    .attribute(node_id, "style")?
                    .and_then(|style| style_property(&style, property))
                    .map(DomValue::String)
                    .unwrap_or(DomValue::Null))
            }
            "styleSet" => {
                let node_id = node_id(argument(arguments, 0)?)?;
                let property = kebab_case(argument(arguments, 1)?);
                let value = argument(arguments, 2)?;
                let current = document
                    .attribute(node_id, "style")?
                    .unwrap_or_else(String::new);
                let style = set_style_property(&current, &property, value);
                document.set_attribute(node_id, "style", &style)?;
                Ok(DomValue::Undefined)
            }
            _ => Err(format!("unsupported HTML style operation: {operation}")),
        }
    }
}
