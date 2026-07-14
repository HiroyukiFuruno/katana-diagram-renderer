mod bridge;
mod diagram_description;

use super::resolve::PlantUmlRuntimePaths;
use super::theme::PlantUmlRenderStyle;
use bridge::PlantUmlJvmBridgeOps;
use jni::{InitArgsBuilder, JNIVersion, JavaVM};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static PLANTUML_JVM: Mutex<Option<PlantUmlJvm>> = Mutex::new(None);

pub(crate) struct PlantUmlJvmRuntimeOps;

struct PlantUmlJvm {
    java_vm: JavaVM,
    jar_path: PathBuf,
}

impl PlantUmlJvmRuntimeOps {
    pub(crate) fn render_svg(
        source: &str,
        paths: &PlantUmlRuntimePaths,
        style: &PlantUmlRenderStyle,
    ) -> Result<String, String> {
        let mut guard = Self::jvm_guard();
        if guard.is_none() {
            *guard = Some(Self::create_jvm(paths)?);
        }
        let jvm = guard
            .as_ref()
            .ok_or("PlantUML JVM is not initialized".to_string())?;
        ensure_matching_jar(&jvm.jar_path, &paths.jar_path)?;
        PlantUmlJvmBridgeOps::render(&jvm.java_vm, source, style)
    }

    fn create_jvm(paths: &PlantUmlRuntimePaths) -> Result<PlantUmlJvm, String> {
        let args = Self::jvm_args(&paths.jar_path)?;
        let java_vm =
            JavaVM::with_libjvm(args, || Ok(paths.jvm_path.clone())).map_err(string_error)?;
        Ok(PlantUmlJvm {
            java_vm,
            jar_path: paths.jar_path.clone(),
        })
    }

    fn jvm_args(jar_path: &std::path::Path) -> Result<jni::InitArgs<'_>, String> {
        let class_path = format!("-Djava.class.path={}", jar_path.display());
        InitArgsBuilder::new()
            .version(JNIVersion::V1_8)
            .option(class_path)
            .option("-Djava.awt.headless=true")
            .build()
            .map_err(string_error)
    }

    fn jvm_guard() -> MutexGuard<'static, Option<PlantUmlJvm>> {
        match PLANTUML_JVM.lock() {
            Ok(guard) => guard,
            Err(error) => error.into_inner(),
        }
    }
}

fn string_error(error: impl ToString) -> String {
    error.to_string()
}

fn ensure_matching_jar(
    active: &std::path::Path,
    requested: &std::path::Path,
) -> Result<(), String> {
    if active == requested {
        return Ok(());
    }
    Err("PlantUML JVM is already initialized with another JAR".to_string())
}

#[cfg(test)]
mod tests {
    use super::PlantUmlJvmRuntimeOps;

    #[test]
    fn jvm_arguments_accept_jar_path_with_spaces() {
        let arguments = PlantUmlJvmRuntimeOps::jvm_args("target/krr tests/plantuml.jar".as_ref());

        assert!(arguments.is_ok());
    }

    #[test]
    fn rejects_a_second_plantuml_jar_after_jvm_initialization() {
        let result =
            super::ensure_matching_jar("target/first.jar".as_ref(), "target/second.jar".as_ref());

        assert!(matches!(result, Err(message) if message.contains("another JAR")));
    }

    #[test]
    fn string_error_preserves_display_message() {
        assert_eq!(
            super::string_error("PlantUML JVM failed"),
            "PlantUML JVM failed"
        );
    }

    #[test]
    fn recovers_from_a_poisoned_jvm_lock() {
        let poisoner = std::thread::spawn(|| {
            let _guard = PlantUmlJvmRuntimeOps::jvm_guard();
            std::panic::resume_unwind(Box::new("poison PlantUML JVM lock"));
        });

        assert!(poisoner.join().is_err());
        let guard = PlantUmlJvmRuntimeOps::jvm_guard();
        drop(guard);
    }
}
