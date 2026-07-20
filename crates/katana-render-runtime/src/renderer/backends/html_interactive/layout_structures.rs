use super::super::html_document::HtmlDocumentNode;
use super::constants::{
    LIST_MARKER_WIDTH, MIN_LAYOUT_WIDTH, RULE_VERTICAL_INSET, TABLE_CELL_CONTENT_INSET,
    TABLE_CELL_PADDING,
};
use super::document::{TableCell, node_text, table_rows, wrap_text};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::svg::escape_xml;
use super::types::{DetailsContext, TableCellLayout};

impl HtmlLayoutRenderer {
    pub(super) fn render_table(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let rows = table_rows(children);
        if rows.is_empty() {
            return y;
        }
        let x = x + style.margin_left;
        let width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let column_width = width / columns as f32;
        let bottom = self.render_table_rows(&rows, x, y + style.margin_top, column_width, style);
        bottom + style.margin_bottom
    }

    fn render_table_rows(
        &mut self,
        rows: &[Vec<TableCell>],
        x: f32,
        mut current_y: f32,
        column_width: f32,
        style: &CssStyle,
    ) -> f32 {
        for (row_index, row) in rows.iter().enumerate() {
            let height = table_row_height(row, column_width, style);
            self.render_table_row(
                row,
                TableCellLayout {
                    row_index,
                    x,
                    y: current_y,
                    width: column_width,
                    height,
                    style,
                },
            );
            current_y += height;
        }
        current_y
    }

    pub(super) fn render_rule(&mut self, x: f32, y: f32, width: f32, style: &CssStyle) -> f32 {
        let line_y = y + style.margin_top + RULE_VERTICAL_INSET;
        let x = x + style.margin_left;
        let width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        self.svg.push_str(&format!(
            r#"<line x1="{x}" y1="{line_y}" x2="{}" y2="{line_y}" stroke="{}" stroke-width="1"/>"#,
            x + width,
            escape_xml(style.border.as_deref().unwrap_or("#c8cdd2"))
        ));
        line_y + RULE_VERTICAL_INSET + style.margin_bottom
    }

    pub(super) fn render_list(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
        ordered: bool,
    ) -> f32 {
        let mut current = y + style.margin_top;
        let x = x + style.margin_left;
        let width = (width - style.margin_left - style.margin_right).max(MIN_LAYOUT_WIDTH);
        let mut index = 1usize;
        for child in children {
            if let Some(items) = list_item_children(child) {
                self.paint_list_marker(ordered, index, x, current, style);
                current = self.render_nodes(
                    items,
                    x + LIST_MARKER_WIDTH,
                    current,
                    width - LIST_MARKER_WIDTH,
                    style,
                    DetailsContext::NONE,
                );
                index += 1;
            }
        }
        current + style.margin_bottom
    }

    pub(super) fn render_list_item(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        self.paint_text_lines(&["•".to_string()], x, y + style.font_size, style);
        self.render_nodes(
            children,
            x + LIST_MARKER_WIDTH,
            y,
            width - LIST_MARKER_WIDTH,
            style,
            DetailsContext::NONE,
        )
    }

    fn render_table_row(&mut self, row: &[TableCell], layout: TableCellLayout<'_>) {
        for (column_index, cell) in row.iter().enumerate() {
            self.render_table_cell(
                cell,
                TableCellLayout {
                    x: layout.x + column_index as f32 * layout.width,
                    ..layout
                },
            );
        }
    }

    fn render_table_cell(&mut self, cell: &TableCell, layout: TableCellLayout<'_>) {
        let style = table_cell_style(cell, layout.row_index, layout.style);
        self.paint_box(layout.x, layout.y, layout.width, layout.height, &style);
        let lines = wrap_text(
            &node_text(&cell.children),
            layout.width - TABLE_CELL_CONTENT_INSET,
            style.font_size,
        );
        self.paint_text_lines(
            &lines,
            layout.x + TABLE_CELL_PADDING,
            layout.y + TABLE_CELL_PADDING + style.font_size,
            &style,
        );
    }

    fn paint_list_marker(&mut self, ordered: bool, index: usize, x: f32, y: f32, style: &CssStyle) {
        self.paint_text_lines(
            &[list_marker(ordered, index)],
            x,
            y + style.font_size,
            style,
        );
    }
}

fn table_row_height(row: &[TableCell], column_width: f32, style: &CssStyle) -> f32 {
    row.iter()
        .map(|cell| table_cell_height(cell, column_width, style))
        .fold(super::constants::CONTROL_HEIGHT, f32::max)
}

fn table_cell_height(cell: &TableCell, column_width: f32, style: &CssStyle) -> f32 {
    wrap_text(
        &node_text(&cell.children),
        column_width - TABLE_CELL_CONTENT_INSET,
        style.font_size,
    )
    .len() as f32
        * style.line_height
        + TABLE_CELL_CONTENT_INSET
}

fn table_cell_style(cell: &TableCell, row_index: usize, style: &CssStyle) -> CssStyle {
    let is_header = row_index == 0 && cell.tag == "th";
    let mut style = style.clone();
    style.background = is_header.then(|| "#0b74c7".to_string());
    style.color = if is_header { "#ffffff" } else { &style.color }.to_string();
    style.border = Some("#c8cdd2".to_string());
    style
}

fn list_item_children(node: &HtmlDocumentNode) -> Option<&[HtmlDocumentNode]> {
    let HtmlDocumentNode::Element { tag, children, .. } = node else {
        return None;
    };
    (tag == "li").then_some(children)
}

fn list_marker(ordered: bool, index: usize) -> String {
    if ordered {
        format!("{index}.")
    } else {
        "•".to_string()
    }
}
