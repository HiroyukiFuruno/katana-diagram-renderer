use super::CssDeclaration;

const BOX_SHORTHAND_VALUE_COUNT: usize = 4;

pub(super) fn expand_box_shorthand(
    name: &str,
    value: &str,
    important: bool,
) -> Option<Vec<CssDeclaration>> {
    let sides = match name {
        "padding" | "margin" => parse_box_sides(value)?,
        _ => return None,
    };

    Some(
        ["top", "right", "bottom", "left"]
            .into_iter()
            .zip(sides)
            .map(|(direction, value)| CssDeclaration {
                name: format!("{name}-{direction}"),
                value,
                important,
            })
            .collect(),
    )
}

fn parse_box_sides(value: &str) -> Option<[String; BOX_SHORTHAND_VALUE_COUNT]> {
    let values = value
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [all] => Some([all.clone(), all.clone(), all.clone(), all.clone()]),
        [vertical, horizontal] => Some([
            vertical.clone(),
            horizontal.clone(),
            vertical.clone(),
            horizontal.clone(),
        ]),
        [top, horizontal, bottom] => Some([
            top.clone(),
            horizontal.clone(),
            bottom.clone(),
            horizontal.clone(),
        ]),
        [top, right, bottom, left] => {
            Some([top.clone(), right.clone(), bottom.clone(), left.clone()])
        }
        _ => None,
    }
}
