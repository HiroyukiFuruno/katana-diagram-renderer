use super::super::super::resolve::{PlantUmlRuntimePathOps, PlantUmlRuntimePaths};
use super::super::PlantUmlJvmRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::plantuml_renderer::asset::{PLANTUML_ENV_LOCK, PlantUmlJarAssetOps};
use crate::markdown::plantuml_renderer::theme::{
    PlantUmlRenderStyle, PlantUmlThemeConfig, PlantUmlThemeOps,
};
use std::ffi::OsString;
use std::sync::MutexGuard;

#[test]
fn bridge_renders_light_and_dark_svg_with_pinned_local_plantuml_runtime() -> Result<(), String> {
    let _guard = env_guard()?;
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let Some(paths) = local_runtime_paths() else {
        return Ok(());
    };
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
    let _guard = env_guard()?;
    let _krr_jvm = EnvOverride::unset("KRR_PLANTUML_JVM");
    let _kdr_jvm = EnvOverride::unset("KDR_PLANTUML_JVM");
    let Some(paths) = local_runtime_paths() else {
        return Ok(());
    };
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
fn jni_error_message_preserves_display_text() {
    assert_eq!(
        super::jni_error_message(jni::errors::Error::NullPtr("bridge test")),
        "Null pointer in bridge test"
    );
}

fn local_runtime_paths() -> Option<PlantUmlRuntimePaths> {
    let jar_path = PlantUmlJarAssetOps::cache_path(None);
    PlantUmlRuntimePathOps::resolve_paths(&jar_path, None).ok()
}

struct EnvOverride {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvOverride {
    fn unset(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        unsafe { std::env::remove_var(key) };
        Self { key, original }
    }
}

impl Drop for EnvOverride {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn env_guard() -> Result<MutexGuard<'static, ()>, String> {
    PLANTUML_ENV_LOCK.lock().map_err(|error| error.to_string())
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
