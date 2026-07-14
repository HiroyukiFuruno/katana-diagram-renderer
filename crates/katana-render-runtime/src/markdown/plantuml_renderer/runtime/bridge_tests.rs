use super::super::super::resolve::PlantUmlRuntimePaths;
use super::super::PlantUmlJvmRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::plantuml_renderer::asset::PlantUmlJarAssetOps;
use crate::markdown::plantuml_renderer::theme::{
    PlantUmlRenderStyle, PlantUmlThemeConfig, PlantUmlThemeOps,
};
use jni::objects::JObject;
use std::path::{Path, PathBuf};

#[test]
fn bridge_renders_light_and_dark_svg_with_pinned_local_plantuml_runtime() -> Result<(), String> {
    let Some((jar_path, jvm_path)) = local_runtime_paths() else {
        return Ok(());
    };
    let paths = PlantUmlRuntimePaths { jar_path, jvm_path };
    let light_style =
        PlantUmlThemeOps::style(DiagramColorPreset::light(), &PlantUmlThemeConfig::default());

    let svg = render_alice_sequence(&paths, &light_style)?;
    assert!(svg.contains("<svg"));
    assert!(svg.contains("Alice"));
    let svg = render_alice_sequence(&paths, &themed_light_style()?)?;
    assert!(svg.contains("<svg"));
    let dark_style =
        PlantUmlThemeOps::style(DiagramColorPreset::dark(), &PlantUmlThemeConfig::default());
    let svg = render_alice_sequence(&paths, &dark_style)?;

    assert!(svg.contains("<svg"));
    Ok(())
}

#[test]
fn bridge_reports_invalid_diagram_descriptions() -> Result<(), String> {
    let Some((jar_path, jvm_path)) = local_runtime_paths() else {
        return Ok(());
    };
    let paths = PlantUmlRuntimePaths { jar_path, jvm_path };
    let style =
        PlantUmlThemeOps::style(DiagramColorPreset::light(), &PlantUmlThemeConfig::default());

    let result = PlantUmlJvmRuntimeOps::render_svg(
        "@startuml\n!error forced test failure\n@enduml",
        &paths,
        &style,
    );

    assert!(matches!(result, Err(message) if message.contains("PlantUML render failed")));
    Ok(())
}

#[test]
fn missing_diagram_description_reports_render_error() {
    assert_eq!(
        super::PlantUmlJvmBridgeOps::missing_description(&JObject::null()),
        Some("error: PlantUML did not return a diagram description".to_string())
    );
}

#[test]
fn jni_error_message_preserves_display_text() {
    assert_eq!(
        super::jni_error_message(jni::errors::Error::NullPtr("bridge test")),
        "Null pointer in bridge test"
    );
}

fn local_runtime_paths() -> Option<(PathBuf, PathBuf)> {
    let jar_path = PlantUmlJarAssetOps::cache_path(None);
    let jvm_path = Path::new(
        "/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home/lib/server/libjvm.dylib",
    )
    .to_path_buf();
    (jar_path.exists() && jvm_path.exists()).then_some((jar_path, jvm_path))
}

fn render_alice_sequence(
    paths: &PlantUmlRuntimePaths,
    style: &PlantUmlRenderStyle,
) -> Result<String, String> {
    PlantUmlJvmRuntimeOps::render_svg("@startuml\nAlice -> Bob\n@enduml", paths, style)
}

fn themed_light_style() -> Result<PlantUmlRenderStyle, String> {
    let themed_config = PlantUmlThemeConfig::from_value(&serde_json::json!({
        "plantuml_theme": "cyborg",
        "plantuml_theme_mode": "light",
    }))?;
    Ok(PlantUmlThemeOps::style(
        DiagramColorPreset::light(),
        &themed_config,
    ))
}
