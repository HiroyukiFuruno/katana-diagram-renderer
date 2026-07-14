pub(super) fn property(style: &str, property: &str) -> Option<String> {
    let property = kebab_case(property);
    style
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .find_map(|(name, value)| (name.trim() == property).then(|| value.trim().to_string()))
}

pub(super) fn set_property(style: &str, property: &str, value: &str) -> String {
    let mut declarations = style
        .split(';')
        .filter_map(|declaration| declaration.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect::<Vec<_>>();
    if let Some((_, existing)) = declarations.iter_mut().find(|(name, _)| name == property) {
        *existing = value.to_string();
    } else {
        declarations.push((property.to_string(), value.to_string()));
    }
    declarations
        .into_iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn kebab_case(value: &str) -> String {
    value
        .chars()
        .enumerate()
        .flat_map(|(index, character)| {
            if character.is_ascii_uppercase() {
                if index == 0 {
                    vec![character.to_ascii_lowercase()]
                } else {
                    vec!['-', character.to_ascii_lowercase()]
                }
            } else {
                vec![character]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{kebab_case, property, set_property};

    #[test]
    fn style_property_round_trip_preserves_existing_declarations() {
        let style = set_property(
            "color: blue; font-weight: bold",
            "background-color",
            "black",
        );

        assert_eq!(property(&style, "color"), Some("blue".to_string()));
        assert_eq!(
            property(&style, "backgroundColor"),
            Some("black".to_string())
        );
        assert_eq!(kebab_case("fontWeight"), "font-weight");
    }
}
