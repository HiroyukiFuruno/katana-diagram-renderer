use super::super::html_document::HtmlDocumentNode;
use super::constants::{
    CONTROL_HEIGHT, MIN_LAYOUT_WIDTH, TABLE_CELL_CONTENT_INSET, TABLE_CELL_PADDING,
};
use super::document::{TableCell, node_text, table_rows, wrap_text_with_style};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::types::TableCellLayout;

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
        let lines = wrap_text_with_style(
            &node_text(&cell.children),
            layout.width - TABLE_CELL_CONTENT_INSET,
            &style,
        );
        self.paint_text_lines(
            &lines,
            layout.x + TABLE_CELL_PADDING,
            layout.width - TABLE_CELL_CONTENT_INSET,
            layout.y + TABLE_CELL_PADDING + style.font_size,
            &style,
        );
    }
}

fn table_row_height(row: &[TableCell], column_width: f32, style: &CssStyle) -> f32 {
    row.iter()
        .map(|cell| table_cell_height(cell, column_width, style))
        .fold(CONTROL_HEIGHT, f32::max)
}

fn table_cell_height(cell: &TableCell, column_width: f32, style: &CssStyle) -> f32 {
    wrap_text_with_style(
        &node_text(&cell.children),
        column_width - TABLE_CELL_CONTENT_INSET,
        style,
    )
    .len() as f32
        * style.line_height
        + TABLE_CELL_CONTENT_INSET
}

fn table_cell_style(cell: &TableCell, row_index: usize, style: &CssStyle) -> CssStyle {
    let is_header = row_index == 0 && cell.tag == "th";
    let mut style = CssStyle::from_element(&cell.tag, &cell.attributes, style);
    if !style.explicit_background && is_header {
        style.background = Some("#0b74c7".to_string());
    }
    if !style.explicit_color && is_header {
        style.color = "#ffffff".to_string();
    }
    if !style.has_any_border() {
        style.border = Some("#c8cdd2".to_string());
        style.border_width = 1.0;
    }
    style
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, TableCell, table_cell_style};

    #[test]
    fn table_cell_uses_cascade_style_before_browser_fallbacks() {
        let header = TableCell {
            tag: "th".to_string(),
            attributes: vec![(
                "style".to_string(),
                "background:#dbeafe;color:#173f5f;border:2px solid #112233".to_string(),
            )],
            children: Vec::new(),
        };
        let style = table_cell_style(&header, 0, &CssStyle::browser_default());

        assert_eq!(style.background.as_deref(), Some("#dbeafe"));
        assert_eq!(style.color, "#173f5f");
        assert_eq!(style.border.as_deref(), Some("#112233"));
        assert_eq!(style.border_width, 2.0);

        let fallback = TableCell {
            tag: "th".to_string(),
            attributes: Vec::new(),
            children: Vec::new(),
        };
        let fallback = table_cell_style(&fallback, 0, &CssStyle::browser_default());
        assert_eq!(fallback.background.as_deref(), Some("#0b74c7"));
        assert_eq!(fallback.color, "#ffffff");
    }
}
