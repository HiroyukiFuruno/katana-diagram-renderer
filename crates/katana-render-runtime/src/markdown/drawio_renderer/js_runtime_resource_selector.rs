use std::collections::BTreeSet;

pub(super) struct DrawioResourceSelector<'a> {
    source: &'a str,
    groups: BTreeSet<String>,
    uses_drawio_shape: bool,
}

impl<'a> DrawioResourceSelector<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        let groups = extract_resource_groups(source);
        let uses_drawio_shape = source.contains("shape=mxgraph.");
        Self {
            source,
            groups,
            uses_drawio_shape,
        }
    }

    pub(super) fn includes(&self, path: &str) -> bool {
        path == "stencils/basic.xml"
            || self.includes_stencil(path)
            || self.includes_shape_script(path)
            || self.includes_image(path)
    }

    fn includes_stencil(&self, path: &str) -> bool {
        let Some(relative_path) = path.strip_prefix("stencils/") else {
            return false;
        };
        if !relative_path.ends_with(".xml") {
            return false;
        }
        self.groups.iter().any(|group| {
            relative_path == format!("{group}.xml")
                || relative_path.starts_with(&format!("{group}/"))
        })
    }

    fn includes_shape_script(&self, path: &str) -> bool {
        path.starts_with("shapes/") && self.uses_drawio_shape
    }

    fn includes_image(&self, path: &str) -> bool {
        is_image_resource(path) && self.source_references(path)
    }

    fn source_references(&self, path: &str) -> bool {
        self.source.contains(path) || self.source.contains(&format!("/{path}"))
    }
}

fn is_image_resource(path: &str) -> bool {
    path.ends_with(".svg")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".gif")
}

pub(super) fn extract_resource_groups(source: &str) -> BTreeSet<String> {
    let mut groups = BTreeSet::new();
    let mut remaining = source;
    while let Some(index) = remaining.find("shape=mxgraph.") {
        remaining = &remaining[index + "shape=mxgraph.".len()..];
        groups.extend(
            drawio_prefix(remaining)
                .into_iter()
                .flat_map(resource_groups),
        );
    }
    groups
}

pub(super) fn drawio_prefix(value: &str) -> Option<&str> {
    let prefix = value.split(['.', ';', '"', '\'', '&', ' ']).next()?;
    if prefix.is_empty() {
        return None;
    }
    Some(prefix)
}

pub(super) fn resource_groups(prefix: &str) -> Vec<String> {
    match prefix.to_ascii_lowercase().as_str() {
        "arrows2" => vec!["arrows".to_string()],
        "ios" | "ios7" | "ios7ui" => vec!["ios7".to_string()],
        "pid2misc" | "pid2valves" => vec!["pid2".to_string()],
        "rackgeneral" => vec!["rack".to_string()],
        "veeam2" => vec!["veeam".to_string()],
        other => vec![other.to_string()],
    }
}
