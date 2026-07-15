use katana_render_runtime::{
    DiagramKind, DrawioRenderer, HtmlRenderInput, HtmlRenderer, MermaidRenderer, RenderConfig,
    RenderContext, RenderDiagnostics, RenderError, RenderInput, RenderOutput, RenderPolicy,
    Renderer, RendererProfile, RuntimeVersion,
};
#[cfg(unix)]
use katana_render_runtime::{
    HTML_BROWSER_PROTOCOL_VERSION, HtmlBrowserInput, HtmlBrowserProcessConfig,
    HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime,
};
use std::path::PathBuf;

#[test]
fn mermaid_renderer_reports_missing_runtime_without_stub_svg()
-> Result<(), Box<dyn std::error::Error>> {
    let renderer = MermaidRenderer::with_runtime_path(PathBuf::from(
        "target/kdr-tests/missing-mermaid.min.js",
    ));
    let input = RenderInput {
        kind: DiagramKind::Mermaid,
        source: "graph TD; A-->B".to_string(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    };

    let error = match renderer.render(&input) {
        Ok(_) => return Err("missing Mermaid runtime returned rendered output".into()),
        Err(error) => error,
    };

    assert!(matches!(error, RenderError::NotInstalled { .. }));
    Ok(())
}

#[test]
fn renderer_rejects_mismatched_diagram_kind_without_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let renderer = MermaidRenderer::with_runtime_path(PathBuf::from(
        "target/kdr-tests/missing-mermaid.min.js",
    ));
    let input = RenderInput {
        kind: DiagramKind::Drawio,
        source: "<mxGraphModel><root /></mxGraphModel>".to_string(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    };

    let error = match renderer.render(&input) {
        Ok(_) => return Err("mismatched diagram kind returned rendered output".into()),
        Err(error) => error,
    };

    assert!(matches!(error, RenderError::UnsupportedKind));
    Ok(())
}

#[test]
fn render_input_keeps_katana_adapter_relevant_fields() -> Result<(), Box<dyn std::error::Error>> {
    let input = RenderInput {
        kind: DiagramKind::Mermaid,
        source: "graph TD; A-->B".to_string(),
        config: RenderConfig {
            vendor_config: serde_json::json!({ "theme": "dark" }),
        },
        policy: RenderPolicy {
            max_width: Some(1200),
            max_height: Some(800),
            padding: Some(8),
            background: Some("#ffffff".to_string()),
            cache_profile: Some("katana-preview".to_string()),
        },
        context: RenderContext {
            theme_fingerprint: Some("theme-v1".to_string()),
            document_id: Some("workspace-file".to_string()),
            theme: None,
        },
    };

    let json = serde_json::to_value(&input)?;

    assert_eq!(json["kind"], "Mermaid");
    assert_eq!(json["source"], "graph TD; A-->B");
    assert_eq!(json["config"]["vendor_config"]["theme"], "dark");
    assert_eq!(json["policy"]["max_width"], 1200);
    assert_eq!(json["context"]["document_id"], "workspace-file");
    Ok(())
}

#[test]
fn render_output_exposes_adapter_relevant_metadata() {
    let output = RenderOutput {
        svg: "<svg width=\"20\" height=\"10\"></svg>".to_string(),
        width: 20.0,
        height: 10.0,
        view_box: "0 0 20 10".to_string(),
        runtime: RuntimeVersion {
            name: "mermaid".to_string(),
            version: "3.3.1".to_string(),
            checksum: Some(
                "217b66ef4279c33c141b4afe22effad10a91c02558dc70917be2c0981e78ed87".to_string(),
            ),
        },
        profile: RendererProfile {
            id: "mermaid-js".to_string(),
            description: Some("official runtime".to_string()),
        },
        diagnostics: RenderDiagnostics {
            warnings: Vec::new(),
            errors: Vec::new(),
        },
        cache_fingerprint: "fingerprint".to_string(),
    };

    assert!(output.svg.contains("<svg"));
    assert_eq!(output.width, 20.0);
    assert_eq!(output.height, 10.0);
    assert_eq!(output.runtime.name, "mermaid");
    assert!(output.runtime.checksum.is_some());
    assert_eq!(output.profile.id, "mermaid-js");
    assert_eq!(output.cache_fingerprint, "fingerprint");
}

#[test]
fn drawio_renderer_reports_missing_runtime_without_stub_svg()
-> Result<(), Box<dyn std::error::Error>> {
    let renderer =
        DrawioRenderer::with_runtime_path(PathBuf::from("target/kdr-tests/missing-drawio.min.js"));
    let input = RenderInput {
        kind: DiagramKind::Drawio,
        source: "<mxGraphModel><root /></mxGraphModel>".to_string(),
        config: RenderConfig::default(),
        policy: RenderPolicy::default(),
        context: RenderContext::default(),
    };

    let error = match renderer.render(&input) {
        Ok(_) => return Err("missing Draw.io runtime returned rendered output".into()),
        Err(error) => error,
    };

    assert!(matches!(error, RenderError::NotInstalled { .. }));
    Ok(())
}

#[test]
fn html_renderer_static_export_contract_remains_separate() -> Result<(), Box<dyn std::error::Error>>
{
    let output = HtmlRenderer.render(&HtmlRenderInput {
        source: "<style>p { color: red; }</style><p id=state>Static</p>".to_string(),
    })?;

    assert!(
        output
            .content
            .contains(r#"<p id="state" style="color: red">Static</p>"#),
        "{}",
        output.content
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn html_runtime_public_session_is_browser_page_session() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = HtmlRuntime.open(browser_source()?, viewport()?, &shell_config())?;

    assert_eq!(session.state(), HtmlBrowserSessionState::Active);
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(1)
    );
    assert_eq!(
        session.take_frame_update().map(|frame| frame.generation),
        Some(1)
    );

    session.dispatch_input(HtmlBrowserInput::Text {
        text: "typed".to_string(),
    })?;
    assert_eq!(
        session.take_frame_update().map(|frame| frame.generation),
        Some(2)
    );
    session.close()?;
    Ok(())
}

#[cfg(unix)]
fn shell_config() -> HtmlBrowserProcessConfig {
    let mut config = HtmlBrowserProcessConfig::new(PathBuf::from("/bin/sh"));
    config.args = vec!["-c".to_string(), browser_session_script()];
    config
}

#[cfg(unix)]
fn browser_source() -> Result<HtmlBrowserSource, Box<dyn std::error::Error>> {
    Ok(HtmlBrowserSource::new(
        "<button id=action>Run</button>",
        "https://example.test/index.html",
    )?)
}

#[cfg(unix)]
fn viewport() -> Result<HtmlBrowserViewport, Box<dyn std::error::Error>> {
    Ok(HtmlBrowserViewport::new(2, 2, 1.0)?)
}

#[cfg(unix)]
fn browser_session_script() -> String {
    format!(
        r#"count=0
while IFS= read -r request; do
  case "$request" in
    *'"command":"close"'*)
      printf '%s\n' '{{"result":"closed","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION}}}'
      exit 0
      ;;
  esac
  count=$((count + 1))
  printf '%s\n' '{{"result":"frame","protocol_version":{HTML_BROWSER_PROTOCOL_VERSION},"frame":{{"generation":'"$count"',"origin":"https://example.test/index.html","viewport":{{"width":2,"height":2,"device_scale_factor":1.0}},"pixel_format":"Rgba8","pixels":[0,0,0,255,0,0,0,255,0,0,0,255,0,0,0,255]}}}}'
done"#
    )
}
