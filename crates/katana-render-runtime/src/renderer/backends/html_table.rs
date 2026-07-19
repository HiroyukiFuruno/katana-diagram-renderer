use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn render_table(table: &Handle) -> String {
    let rows = descendant_elements(table, "tr")
        .into_iter()
        .map(|row| table_cells(&row))
        .filter(|cells| !cells.is_empty())
        .collect::<Vec<_>>();
    MarkdownTable::new(rows).to_markdown()
}

struct MarkdownTable {
    rows: Vec<Vec<String>>,
}

impl MarkdownTable {
    fn new(rows: Vec<Vec<String>>) -> Self {
        Self { rows }
    }

    fn to_markdown(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }
        let width = self.rows.iter().map(Vec::len).max().unwrap_or(1);
        let header = Self::row(self.rows.first().map(Vec::as_slice).unwrap_or(&[]), width);
        let divider = Self::divider(width);
        let body = self
            .rows
            .iter()
            .skip(1)
            .map(|row| Self::row(row, width))
            .collect::<Vec<_>>();
        [vec![header, divider], body].concat().join("\n")
    }

    fn divider(width: usize) -> String {
        Self::pipe_row(&(0..width).map(|_| "---").collect::<Vec<_>>())
    }

    fn row(cells: &[String], width: usize) -> String {
        let mut normalized = cells.iter().map(String::as_str).collect::<Vec<_>>();
        while normalized.len() < width {
            normalized.push("");
        }
        Self::pipe_row(&normalized)
    }

    fn pipe_row(cells: &[&str]) -> String {
        format!("| {} |", cells.join(" | "))
    }
}

fn table_cells(row: &Handle) -> Vec<String> {
    descendant_elements(row, "td")
        .into_iter()
        .chain(descendant_elements(row, "th"))
        .map(|cell| normalize_cell_text(&text_content(&cell)))
        .collect()
}

fn descendant_elements(node: &Handle, name: &str) -> Vec<Handle> {
    let mut matches = Vec::new();
    collect_descendants(node, name, &mut matches);
    matches
}

fn collect_descendants(node: &Handle, name: &str, matches: &mut Vec<Handle>) {
    for child in node.children.borrow().iter() {
        if element_name(child).is_some_and(|candidate| candidate == name) {
            matches.push(child.clone());
        }
        collect_descendants(child, name, matches);
    }
}

fn element_name(node: &Handle) -> Option<String> {
    match &node.data {
        NodeData::Element { name, .. } => Some(name.local.to_string().to_ascii_lowercase()),
        _ => None,
    }
}

fn text_content(node: &Handle) -> String {
    let own = match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => String::new(),
    };
    node.children.borrow().iter().fold(own, |mut text, child| {
        text.push_str(&text_content(child));
        text
    })
}

fn normalize_cell_text(value: &str) -> String {
    value
        .replace('|', "\\|")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
