use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::types::{ElementRenderContext, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn render_tag(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        match element.tag {
            "html" | "body" | "main" => self.render_container_element(element.children, layout),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" => {
                self.render_label_element(element, layout)
            }
            "label" => self.render_label_container_element(element, layout),
            "a" => self.render_link_element(element, layout),
            "button" => self.render_button(element, layout),
            "img" => self.render_image(
                element.attributes,
                layout.x,
                layout.y,
                layout.width,
                layout.style,
            ),
            "input" | "textarea" => self.render_input_element(element, layout),
            "details" => self.render_details(element, layout),
            "summary" => self.render_summary(element, layout),
            _ => self.render_structural_tag(element, layout),
        }
    }

    fn render_structural_tag(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        match element.tag {
            "table" => self.render_table_element(element.children, layout),
            "hr" => self.render_rule_element(layout),
            "br" => layout.y + layout.style.line_height,
            "ul" | "ol" => self.render_list_element(element.children, layout, element.tag == "ol"),
            "li" => self.render_list_item_element(element.children, layout),
            _ => self.render_container_element(element.children, layout),
        }
    }

    fn render_container_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_container(
            children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            layout.details_node_id,
        )
    }

    fn render_label_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_label(
            element.tag,
            element.children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
        )
    }

    fn render_label_container_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let style = layout.style.clone().for_tag("label");
        self.render_container(
            element.children,
            layout.x,
            layout.y,
            layout.width,
            &style,
            layout.details_node_id,
        )
    }

    fn render_link_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_link(
            element.node_id,
            element.children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
        )
    }

    fn render_input_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_input(
            element.node_id,
            element.attributes,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
        )
    }

    fn render_table_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_table(children, layout.x, layout.y, layout.width, layout.style)
    }

    fn render_rule_element(&mut self, layout: LayoutContext<'_>) -> f32 {
        self.render_rule(layout.x, layout.y, layout.width, layout.style)
    }

    fn render_list_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        ordered: bool,
    ) -> f32 {
        self.render_list(
            children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            ordered,
        )
    }

    fn render_list_item_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_list_item(children, layout.x, layout.y, layout.width, layout.style)
    }
}
