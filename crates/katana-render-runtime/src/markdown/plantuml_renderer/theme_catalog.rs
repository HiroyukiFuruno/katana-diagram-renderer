pub struct PlantUmlThemeCatalog;

impl PlantUmlThemeCatalog {
    pub const HELP_TEXT: &'static str = concat!(
        "PlantUML official theme name. Available themes: ",
        "_none_, amiga, aws-orange, black-knight, bluegray, blueprint, carbon-gray, ",
        "cerulean, cerulean-outline, cloudscape-design, crt-amber, crt-green, cyborg, ",
        "cyborg-outline, hacker, lightgray, mars, materia, materia-outline, metal, ",
        "mimeograph, minty, mono, plain, reddress-darkblue, reddress-darkgreen, ",
        "reddress-darkorange, reddress-darkred, reddress-lightblue, reddress-lightgreen, ",
        "reddress-lightorange, reddress-lightred, sandstone, silver, sketchy, ",
        "sketchy-outline, spacelab, spacelab-white, sunlust, superhero, superhero-outline, ",
        "toy, united, vibrant."
    );

    pub const NAMES: &'static [&'static str] = &[
        "_none_",
        "amiga",
        "aws-orange",
        "black-knight",
        "bluegray",
        "blueprint",
        "carbon-gray",
        "cerulean",
        "cerulean-outline",
        "cloudscape-design",
        "crt-amber",
        "crt-green",
        "cyborg",
        "cyborg-outline",
        "hacker",
        "lightgray",
        "mars",
        "materia",
        "materia-outline",
        "metal",
        "mimeograph",
        "minty",
        "mono",
        "plain",
        "reddress-darkblue",
        "reddress-darkgreen",
        "reddress-darkorange",
        "reddress-darkred",
        "reddress-lightblue",
        "reddress-lightgreen",
        "reddress-lightorange",
        "reddress-lightred",
        "sandstone",
        "silver",
        "sketchy",
        "sketchy-outline",
        "spacelab",
        "spacelab-white",
        "sunlust",
        "superhero",
        "superhero-outline",
        "toy",
        "united",
        "vibrant",
    ];

    pub fn names() -> &'static [&'static str] {
        Self::NAMES
    }

    pub fn help_text() -> String {
        Self::HELP_TEXT.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::PlantUmlThemeCatalog;

    #[test]
    fn catalog_contains_available_plantuml_themes() {
        let names = PlantUmlThemeCatalog::names();

        assert!(names.contains(&"cyborg"));
        assert!(names.contains(&"black-knight"));
        assert!(names.contains(&"spacelab"));
    }

    #[test]
    fn help_text_contains_every_catalog_theme() {
        let help_text = PlantUmlThemeCatalog::help_text();

        for theme in PlantUmlThemeCatalog::names() {
            assert!(help_text.contains(theme), "{theme}");
        }
    }
}
