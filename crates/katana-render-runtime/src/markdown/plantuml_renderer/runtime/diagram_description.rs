use jni::{
    jni_sig, jni_str,
    objects::{JObject, JString, JValue},
};

pub(super) struct PlantUmlDiagramDescriptionOps;

const PLANTUML_ERROR_STATUS_MIN: i32 = 400;

impl PlantUmlDiagramDescriptionOps {
    pub(super) fn error<'local>(
        env: &mut jni::Env<'local>,
        description: &JObject<'local>,
    ) -> jni::errors::Result<Option<String>> {
        let image_data = Self::image_data(env, description)?;
        let status = Self::image_data_status(env, &image_data)?;
        if status < PLANTUML_ERROR_STATUS_MIN {
            return Ok(None);
        }
        let detail = Self::text(env, description)?;
        Ok(Some(format!(
            "error: PlantUML reported status {status}: {detail}"
        )))
    }

    pub(super) fn missing(description: &JObject<'_>) -> Option<String> {
        description
            .is_null()
            .then(|| "error: PlantUML did not return a diagram description".to_string())
    }

    pub(super) fn text<'local>(
        env: &mut jni::Env<'local>,
        description: &JObject<'local>,
    ) -> jni::errors::Result<String> {
        let arguments: &[JValue<'_>] = &[];
        let method = jni_str!("getDescription");
        let signature = jni_sig!("()Ljava/lang/String;");
        let value = env.call_method(description, method, signature, arguments)?;
        let text = value.l()?;
        let java_text = env.cast_local::<JString>(text)?;
        java_text.try_to_string(env)
    }

    fn image_data<'local>(
        env: &mut jni::Env<'local>,
        description: &JObject<'local>,
    ) -> jni::errors::Result<JObject<'local>> {
        let arguments: &[JValue<'_>] = &[];
        let method = jni_str!("getImageData");
        let signature = jni_sig!("()Lnet/sourceforge/plantuml/core/ImageData;");
        let value = env.call_method(description, method, signature, arguments)?;
        value.l()
    }

    fn image_data_status(
        env: &mut jni::Env<'_>,
        image_data: &JObject<'_>,
    ) -> jni::errors::Result<i32> {
        let arguments: &[JValue<'_>] = &[];
        let method = jni_str!("getStatus");
        let signature = jni_sig!("()I");
        let value = env.call_method(image_data, method, signature, arguments)?;
        value.i()
    }
}

#[cfg(test)]
mod tests {
    use super::PlantUmlDiagramDescriptionOps;
    use jni::objects::JObject;

    #[test]
    fn missing_diagram_description_reports_render_error() {
        assert_eq!(
            PlantUmlDiagramDescriptionOps::missing(&JObject::null()),
            Some("error: PlantUML did not return a diagram description".to_string())
        );
    }
}
