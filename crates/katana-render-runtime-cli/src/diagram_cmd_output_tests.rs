use super::DiagramCommand;
use katana_render_runtime::{RenderDiagnostics, RenderOutput, RendererProfile, RuntimeVersion};

#[test]
fn write_render_output_handles_file_and_stdout_outputs() -> Result<(), Box<dyn std::error::Error>> {
    let output = render_output();
    let path = std::env::temp_dir().join(format!("krr-cli-output-{}.svg", std::process::id()));

    DiagramCommand::write_render_output(Some(path.clone()), &output)?;
    DiagramCommand::write_render_output(None, &output)?;

    assert_eq!(std::fs::read_to_string(&path)?, "<svg/>");
    std::fs::remove_file(path)?;
    Ok(())
}

fn render_output() -> RenderOutput {
    RenderOutput {
        svg: "<svg/>".to_string(),
        width: 1.0,
        height: 1.0,
        view_box: "0 0 1 1".to_string(),
        runtime: RuntimeVersion {
            name: "test".to_string(),
            version: "0".to_string(),
            checksum: None,
        },
        profile: RendererProfile {
            id: "test".to_string(),
            description: None,
        },
        diagnostics: RenderDiagnostics {
            warnings: vec!["test warning".to_string()],
            errors: Vec::new(),
        },
        cache_fingerprint: "test".to_string(),
    }
}
