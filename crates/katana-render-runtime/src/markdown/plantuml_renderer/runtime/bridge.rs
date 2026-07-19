use super::super::theme::PlantUmlRenderStyle;
use super::diagram_description::PlantUmlDiagramDescriptionOps;
use jni::{
    JavaVM, jni_sig, jni_str,
    objects::{JByteArray, JObject, JString, JValue},
};

pub(crate) struct PlantUmlJvmBridgeOps;

impl PlantUmlJvmBridgeOps {
    pub(crate) fn render(
        java_vm: &JavaVM,
        source: &str,
        style: &PlantUmlRenderStyle,
    ) -> Result<String, String> {
        let (svg, description) = java_vm
            .attach_current_thread(|env| -> jni::errors::Result<(String, String)> {
                let source = env.new_string(source)?;
                let reader = Self::source_string_reader(env, &source, style)?;
                let output_stream = Self::byte_array_output_stream(env)?;
                let description = Self::render_to_stream(env, &reader, &output_stream, style)?;
                let svg = Self::svg_from_stream(env, &output_stream)?;
                Ok((svg, description))
            })
            .map_err(jni_error_message)?;
        if description.to_ascii_lowercase().contains("error") {
            return Err(format!("PlantUML render failed: {description}"));
        }
        Ok(svg)
    }

    fn source_string_reader<'local>(
        env: &mut jni::Env<'local>,
        source: &JString<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<JObject<'local>> {
        let defines = Self::empty_defines(env)?;
        let config = Self::config_list(env, style)?;
        let signature = jni_sig!(
            "(Lnet/sourceforge/plantuml/preproc/Defines;Ljava/lang/String;Ljava/util/List;)V"
        );
        let arguments = [
            JValue::Object(&defines),
            JValue::Object(source),
            JValue::Object(&config),
        ];
        let class = jni_str!("net/sourceforge/plantuml/SourceStringReader");
        env.new_object(class, signature, &arguments)
    }

    fn empty_defines<'local>(env: &mut jni::Env<'local>) -> jni::errors::Result<JObject<'local>> {
        let arguments: &[JValue<'_>] = &[];
        let class = jni_str!("net/sourceforge/plantuml/preproc/Defines");
        let signature = jni_sig!("()Lnet/sourceforge/plantuml/preproc/Defines;");
        let value = env.call_static_method(class, jni_str!("createEmpty"), signature, arguments)?;
        value.l()
    }

    fn config_list<'local>(
        env: &mut jni::Env<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<JObject<'local>> {
        let list = env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[])?;
        for line in style.config_lines() {
            let java_line = env.new_string(line)?;
            let arguments = [JValue::Object(&java_line)];
            let method = jni_str!("add");
            let signature = jni_sig!("(Ljava/lang/Object;)Z");
            env.call_method(&list, method, signature, &arguments)?;
        }
        Ok(list)
    }

    fn byte_array_output_stream<'local>(
        env: &mut jni::Env<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        env.new_object(
            jni_str!("java/io/ByteArrayOutputStream"),
            jni_sig!("()V"),
            &[],
        )
    }

    fn render_to_stream<'local>(
        env: &mut jni::Env<'local>,
        reader: &JObject<'local>,
        output_stream: &JObject<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<String> {
        let format_option = Self::svg_format_option(env, style)?;
        let description = Self::call_output_image(env, reader, output_stream, &format_option)?;
        if let Some(error) = PlantUmlDiagramDescriptionOps::error(env, &description)? {
            return Ok(error);
        }
        PlantUmlDiagramDescriptionOps::missing(&description)
            .map(Ok)
            .unwrap_or_else(|| PlantUmlDiagramDescriptionOps::text(env, &description))
    }

    fn svg_format_option<'local>(
        env: &mut jni::Env<'local>,
        style: &PlantUmlRenderStyle,
    ) -> jni::errors::Result<JObject<'local>> {
        let class = jni_str!("net/sourceforge/plantuml/FileFormat");
        let signature = jni_sig!("Lnet/sourceforge/plantuml/FileFormat;");
        let value = env.get_static_field(class, jni_str!("SVG"), signature)?;
        let svg_format = value.l()?;
        let signature = jni_sig!("(Lnet/sourceforge/plantuml/FileFormat;)V");
        let arguments = [JValue::Object(&svg_format)];
        let class = jni_str!("net/sourceforge/plantuml/FileFormatOption");
        let format_option = env.new_object(class, signature, &arguments)?;
        if style.dark_mode() {
            return Self::dark_format_option(env, &format_option);
        }
        Ok(format_option)
    }

    fn dark_format_option<'local>(
        env: &mut jni::Env<'local>,
        format_option: &JObject<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        let class = jni_str!("net/sourceforge/plantuml/klimt/color/ColorMapper");
        let signature = jni_sig!("Lnet/sourceforge/plantuml/klimt/color/ColorMapper;");
        let value = env.get_static_field(class, jni_str!("DARK_MODE"), signature)?;
        let dark_mapper = value.l()?;
        let signature = jni_sig!(
            "(Lnet/sourceforge/plantuml/klimt/color/ColorMapper;)Lnet/sourceforge/plantuml/FileFormatOption;"
        );
        let arguments = [JValue::Object(&dark_mapper)];
        let method = jni_str!("withColorMapper");
        let value = env.call_method(format_option, method, signature, &arguments)?;
        value.l()
    }

    fn call_output_image<'local>(
        env: &mut jni::Env<'local>,
        reader: &JObject<'local>,
        output_stream: &JObject<'local>,
        format_option: &JObject<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        let signature = jni_sig!(
            "(Ljava/io/OutputStream;Lnet/sourceforge/plantuml/FileFormatOption;)Lnet/sourceforge/plantuml/core/DiagramDescription;"
        );
        let arguments = [JValue::Object(output_stream), JValue::Object(format_option)];
        let value = env.call_method(reader, jni_str!("outputImage"), signature, &arguments)?;
        value.l()
    }

    fn svg_from_stream(
        env: &mut jni::Env<'_>,
        output_stream: &JObject<'_>,
    ) -> jni::errors::Result<String> {
        let arguments: &[JValue<'_>] = &[];
        let method = jni_str!("toByteArray");
        let signature = jni_sig!("()[B");
        let value = env.call_method(output_stream, method, signature, arguments)?;
        let bytes_object = value.l()?;
        let bytes = env.cast_local::<JByteArray>(bytes_object)?;
        let svg_bytes = env.convert_byte_array(&bytes)?;
        Ok(String::from_utf8_lossy(&svg_bytes).to_string())
    }
}

fn jni_error_message(error: jni::errors::Error) -> String {
    error.to_string()
}

#[cfg(test)]
#[path = "bridge_tests.rs"]
mod tests;
