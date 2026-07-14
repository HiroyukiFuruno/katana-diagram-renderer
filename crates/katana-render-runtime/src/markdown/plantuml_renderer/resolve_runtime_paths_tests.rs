use super::super::PlantUmlRuntimePathOps;
use super::temp_path;
use std::path::PathBuf;

#[test]
fn runtime_paths_for_candidates_builds_paths() -> Result<(), String> {
    let jar_path = PathBuf::from("plantuml.jar");
    let jvm_path = temp_path("runtime-libjvm.dylib");
    std::fs::write(&jvm_path, b"jvm").map_err(|error| error.to_string())?;

    let paths = PlantUmlRuntimePathOps::runtime_paths_for_candidates(
        jar_path.clone(),
        vec![jvm_path.clone()],
    )
    .map_err(|warning| warning.message())?;
    let _ = std::fs::remove_file(&jvm_path);

    assert_eq!(paths.jar_path, jar_path);
    assert_eq!(paths.jvm_path, jvm_path);
    Ok(())
}
