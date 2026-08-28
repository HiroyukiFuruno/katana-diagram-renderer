use super::diagram_type::MermaidDiagramType;
use crate::markdown::diagram_js_runtime::DiagramRuntimeScript;

pub(super) struct MermaidRuntimeScripts;

impl MermaidRuntimeScripts {
    pub(super) fn build<'a>(
        bundle: &'a str,
        request_json: &str,
        diagram_type: MermaidDiagramType,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        Self::build_scripts(
            bundle,
            MERMAID_ZENUML_RUNTIME,
            request_json,
            diagram_type == MermaidDiagramType::Zenuml,
        )
    }

    fn build_scripts<'a>(
        bundle: &'a str,
        zenuml_bundle: &'a str,
        request_json: &str,
        uses_zenuml_runtime: bool,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        let mut scripts = vec![
            DiagramRuntimeScript::borrowed("mermaid-runtime.min.js", MERMAID_RUNTIME),
            DiagramRuntimeScript::borrowed("mermaid.min.js", bundle),
            DiagramRuntimeScript::borrowed("mermaid-zenuml.min.js", zenuml_bundle),
        ];
        if uses_zenuml_runtime {
            scripts.push(DiagramRuntimeScript::borrowed(
                "zenuml-runtime.min.js",
                ZENUML_RUNTIME,
            ));
        }
        scripts.push(DiagramRuntimeScript::owned(
            "render-mermaid.js",
            render_script(request_json),
        ));
        scripts
    }

    #[cfg(test)]
    fn build_with_zenuml<'a>(
        bundle: &'a str,
        zenuml_bundle: &'a str,
        request_json: &str,
    ) -> Vec<DiagramRuntimeScript<'a>> {
        Self::build_scripts(
            bundle,
            zenuml_bundle,
            request_json,
            request_uses_zenuml_runtime(request_json),
        )
    }
}

fn render_script(request_json: &str) -> String {
    format!("katanaRunMermaidRuntime({request_json});")
}

#[cfg(test)]
fn request_uses_zenuml_runtime(request_json: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(request_json) else {
        return false;
    };
    if value
        .get("diagramType")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|diagram_type| diagram_type.eq_ignore_ascii_case("zenuml"))
    {
        return true;
    }
    let Some(source) = value.get("source").and_then(serde_json::Value::as_str) else {
        return false;
    };
    MermaidDiagramType::from_source(source) == MermaidDiagramType::Zenuml
}

const MERMAID_RUNTIME: &str = include_str!("../diagram_runtime/generated/mermaid-runtime.min.js");
const MERMAID_ZENUML_RUNTIME: &str =
    include_str!(concat!(env!("OUT_DIR"), "/mermaid-zenuml.min.js"));
const ZENUML_RUNTIME: &str = include_str!("../diagram_runtime/generated/zenuml-runtime.min.js");

#[cfg(test)]
#[path = "js_runtime_scripts_class_tests.rs"]
mod class_tests;
#[cfg(test)]
#[path = "js_runtime_scripts_entrypoint_tests.rs"]
mod entrypoint_tests;

#[cfg(test)]
mod tests {
    use super::{MermaidDiagramType, MermaidRuntimeScripts};
    use crate::markdown::{diagram_js_runtime::DiagramV8Runtime, runtime_assets::RuntimeAsset};

    #[test]
    fn build_includes_bundle_and_render_script() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml("bundle", "plugin", "{}");
        assert!(scripts.iter().any(|it| it.name == "mermaid-runtime.min.js"));
        assert!(scripts.iter().any(|it| it.name == "mermaid.min.js"));
        assert!(scripts.iter().any(|it| it.name == "mermaid-zenuml.min.js"));
        assert!(scripts.iter().any(|it| it.name == "render-mermaid.js"));
    }

    #[test]
    fn invalid_request_json_does_not_load_zenuml_runtime() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml("bundle", "plugin", "not-json");

        assert!(!scripts.iter().any(|it| it.name == "zenuml-runtime.min.js"));
    }

    #[test]
    fn zenuml_registration_runs_before_render() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid(),
            fake_zenuml(),
            r##"{"source":"zenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"zenuml"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("registered")),
            "{rendered:?}"
        );
    }

    #[test]
    fn zenuml_directive_source_registers_external_diagram_without_request_hint() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid(),
            fake_zenuml(),
            r##"{"source":"%%{init: { \"theme\": \"dark\" }}%%\n%% comment\nzenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("registered")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_provides_constructable_style_sheet() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_style_sheet(),
            "",
            r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains("font-family: sans-serif")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_preserves_c4_description_with_explicit_line_break() {
        let scripts = MermaidRuntimeScripts::build(
            mermaid_test_bundle(),
            r##"{"source":"C4Context\nPerson(customer, \"顧客\", \"1行目<br/> 2行目\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
            MermaidDiagramType::Other,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|svg| {
                svg.contains(">1行目</tspan>") && svg.contains(">2行目</tspan>")
            }),
            "{rendered:?}"
        );
    }

    fn mermaid_test_bundle() -> &'static str {
        must_utf8(std::str::from_utf8(must_asset_bytes(
            RuntimeAsset::mermaid().bytes(),
        )))
    }

    #[test]
    #[should_panic(expected = "unexpected test error: boom")]
    fn must_asset_bytes_reports_unexpected_test_errors() {
        let _ = must_asset_bytes(Err("boom".to_string()));
    }

    #[test]
    #[should_panic(expected = "invalid utf-8")]
    fn must_utf8_reports_unexpected_test_errors() {
        let _ = must_utf8(std::str::from_utf8(&[u8::MAX]));
    }

    fn must_asset_bytes(result: Result<&'static [u8], String>) -> &'static [u8] {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn must_utf8(result: Result<&'static str, std::str::Utf8Error>) -> &'static str {
        match result {
            Ok(value) => value,
            Err(error) => fail(format!("unexpected test error: {error}")),
        }
    }

    fn fail(message: String) -> ! {
        std::panic::resume_unwind(Box::new(message))
    }

    #[test]
    fn runtime_measures_horizontal_svg_arcs_like_a_browser() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_horizontal_svg_arcs(),
            "",
            r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains("0:0:216:121.673")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_measures_vertical_svg_arcs_like_a_browser() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_vertical_svg_arcs(),
            "",
            r##"{"source":"C4Context\nSystemQueue(queue, \"Queue\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(
                |it| it.contains("-9.26887261930837:-73.650734:243.80661785792515:73.650734")
            ),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_measures_repeated_relative_curves_from_each_segment_origin() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_repeated_relative_curves(),
            "",
            r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("10:10:60:0")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_matches_browser_c4_canvas_text_width() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_c4_canvas_text(),
            "",
            r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("54.279")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_matches_browser_c4_wide_text_kerning() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_c4_wide_svg_text(),
            "",
            r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("154.78125")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_matches_browser_c4_relation_text_widths() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_c4_relation_svg_text(),
            "",
            r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains("27.340:46.001:133.384:48.000")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_respects_nearest_c4_font_weight() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_nearest_c4_font_weight(),
            "",
            r##"{"source":"C4Context\nSystemQueue(queue, \"Queue\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains(">normal<")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_applies_descendant_css_text_anchor_to_text_measurement() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_descendant_text_anchor(),
            "",
            r##"{"source":"block-beta\nA[\"Markdown\"]","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("<text>-")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_er_viewbox_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_er_viewbox(),
            "",
            r##"{"source":"erDiagram\nDOCUMENT ||--o{ SECTION : contains","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"viewBox="6.25 0 169.03125 527.5""#)
                    && it.contains(r#"transform="translate(84.515625, 64.625)""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_state_viewbox_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_state_viewbox(),
            "",
            r##"{"source":"stateDiagram-v2\n[*] --> Pending","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"style="max-width: 176.711px;""#)
                    && it.contains(r#"viewBox="0 0 176.711 287""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_requirement_viewbox_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_requirement_viewbox(),
            "",
            r##"{"source":"requirementDiagram\nrequirement test_req { id: 1 }","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"width="174""#)
                    && it.contains(r#"height="382""#)
                    && it.contains(r#"viewBox="0 0 173.737 382""#)
                    && it.contains(r#"max-width: 174px;"#)
                    && it.contains(r#"transform="translate(0, -10.5)""#)
                    && it.contains(r#"x="-47.7578125" y="-1" width="95.515625" height="23""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_class_fixture_layout_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_class_enumeration_layout(),
            "",
            r##"{"source":"classDiagram\nclass PreviewPane\nclass RenderedSection\nPreviewPane --> RenderedSection","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"max-width: 235.1796875px;"#)
                    && it.contains(r#"viewBox="0 0 235.1796875 377""#)
                    && it.contains("M117.59,146L117.59,150.167")
                    && it.contains(r#"transform="translate(117.58984375, 77)""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_mindmap_fixture_layout_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_mindmap_layout(),
            "",
            r##"{"source":"mindmap\n  root((mindmap))","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"max-width: 713.7322387695312px;"#)
                    && it.contains(r#"viewBox="5 5 713.7322387695312 372.634467""#)
                    && it.contains("M410.65,216.34245")
                    && it.contains(
                        "On effectiveness<tspan x=\"0\" dy=\"1.1em\">and features</tspan>"
                    )),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_simple_mindmap_fixture_layout_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_simple_mindmap_layout(),
            "",
            r##"{"source":"mindmap\n  root((Mermaid))","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"max-width: 539.5531005859375px;"#)
                    && it.contains(r#"viewBox="5 5 539.5531005859375 220.367651""#)
                    && it.contains("M266.435,117.5586")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_localized_requirement_layout_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_localized_requirement_layout(),
            "",
            r##"{"source":"requirementDiagram\nrequirement テスト要件 { id: 1 }","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"viewBox="0 0 161.21875 382""#)
                    && it.contains(r#"transform="translate(80.609375, 62.5)""#)
                    && it.contains(r#"M-69.0546875 -54.5"#)
                    && it.contains(r#"transform="translate(-21.2265625, -22)""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_japanese_sequence_activation_layout_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_japanese_sequence_activation_layout(),
            "",
            r##"{"source":"sequenceDiagram\n田中->>+鈴木: こんにちは鈴木さん、お元気ですか？","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"width="571""#)
                    && it.contains(r#"viewBox="-50 -10 571 355""#)
                    && it.contains(r#"x="391" y="111""#)
                    && it.contains(r#"x1="391" y1="203" x2="79" y2="203""#)
                    && it.contains(r#"x="232" y="80""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_english_sequence_activation_layout_to_browser_bounds() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_english_sequence_activation_layout(),
            "",
            r##"{"source":"sequenceDiagram\nAlice->>+John: Hello John, how are you?","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"width="501""#)
                    && it.contains(r#"viewBox="-50 -10 501 355""#)
                    && it.contains(r#"x="251" y="269""#)
                    && it.contains(r#"x1="326" y1="65" x2="326" y2="269""#)
                    && it.contains(r#"x="321" y="111""#)
                    && it.contains(r#"x1="321" y1="203" x2="79" y2="203""#)
                    && it.contains(r#"x2="318" y2="111""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_c4_review_keeps_svg_when_source_does_not_match() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_c4_review_svg(),
            "",
            r##"{"source":"graph TD\nA-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"style="max-width: 300px;""#)
                    && it.contains(r#"viewBox="0 0 300 200""#)
                    && !it.contains(r#"viewBox="0 -70 1148 441""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_normalizes_c4_review_changes_svg_when_source_matches() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_c4_review_svg(),
            "",
            r##"{"source":"C4Context\nPerson(a,\"A\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains(r#"style="max-width: 1148px;""#)
                    && it.contains(r#"viewBox="0 -70 1148 441""#)),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_reads_html_line_breaks_as_visible_text() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_with_html_line_break(),
            "",
            r##"{"source":"mindmap\n  root((mindmap))","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|it| it.contains("On effectiveness\nand features")),
            "{rendered:?}"
        );
    }

    #[test]
    fn runtime_measures_svg_without_viewbox_with_dom_metrics_helper() {
        let scripts = MermaidRuntimeScripts::build_with_zenuml(
            fake_mermaid_measuring_svg_without_viewbox(),
            "",
            r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
        );

        let rendered = DiagramV8Runtime::render(&scripts);

        assert!(
            rendered.as_ref().is_ok_and(|it| it.contains("800:600")),
            "{rendered:?}"
        );
    }

    fn fake_mermaid() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  registerExternalDiagrams: async (diagrams) => {
    globalThis.__registeredDiagram = diagrams[0].id;
  },
  render: async (id) => {
    const text = globalThis.__registeredDiagram ?? "missing";
    return { svg: `<svg id="${id}"><text>${text}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_svg_without_viewbox() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    globalThis.innerWidth = 800;
    globalThis.innerHeight = 600;
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    document.body.appendChild(svg);
    const box = svg.getBoundingClientRect();
    svg.remove();
    return { svg: `<svg id="${id}" viewBox="0 0 800 600"><text>${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_horizontal_svg_arcs() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M0,15.8358 a108,15.8358 0,0,0 216,0 a108,15.8358 0,0,0 -216,0 l0,90.0019 a108,15.8358 0,0,0 216,0 l0,-90.0019");
    const box = path.getBBox();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.y}:${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_vertical_svg_arcs() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M0,0 a9.26887261930837,36.825367 0,0,1 0,-73.650734 l225.26887261930838,0 a9.26887261930837,36.825367 0,0,1 0,73.650734 M225.26887261930838,-73.650734 a9.26887261930837,36.825367 0,0,0 0,73.650734 l-225.26887261930838,0");
    const box = path.getBBox();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.y}:${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_repeated_relative_curves() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M 10 10 c 10 0 20 0 30 0 c 10 0 20 0 30 0");
    const box = path.getBBox();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.y}:${box.width}:${box.height}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_c4_canvas_text() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const context = document.createElement("canvas").getContext("2d");
    context.font = "10.5px Open Sans, sans-serif";
    const width = context.measureText("[Container: ").width.toFixed(3);
    return { svg: `<svg id="${id}"><text>${width}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_c4_wide_svg_text() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    tspan.style.setProperty("font-size", "10.5px");
    tspan.style.setProperty("font-family", "Open Sans, sans-serif");
    tspan.textContent = "リレーショナルデータベーススキ";
    const width = tspan.getComputedTextLength();
    return { svg: `<svg id="${id}"><text>${width}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_c4_relation_svg_text() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const values = ["Uses", "[HTTPS]", "Reads from and writes to", "利用する"];
    const widths = values.map((value) => {
      const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
      tspan.style.setProperty("font-size", "12px");
      tspan.style.setProperty("font-family", "Open Sans, sans-serif");
      tspan.textContent = value;
      return tspan.getComputedTextLength().toFixed(3);
    });
    return { svg: `<svg id="${id}"><text>${widths.join(":")}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_measuring_nearest_c4_font_weight() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.style.setProperty("font-weight", "bold");
    const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
    const outer = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    const inner = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    outer.style.setProperty("font-size", "14px");
    outer.style.setProperty("font-family", "Open Sans, sans-serif");
    inner.setAttribute("font-weight", "normal");
    inner.textContent = "Banking System F Queue";
    outer.appendChild(inner);
    text.appendChild(outer);
    group.appendChild(text);
    const weight = outer.getComputedTextLength() < 165 ? "normal" : "bold";
    return { svg: `<svg id="${id}"><text>${weight}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_with_style_sheet() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const sheet = new CSSStyleSheet();
    const index = sheet.insertRule(".style-built { fill: red; }", sheet.cssRules.length);
    sheet.cssRules[index].style.setProperty("font-family", "sans-serif");
    if (sheet.cssRules[index].style.length !== 2) throw new Error("missing CSS declarations");
    const theme = new CSSStyleSheet();
    theme.replaceSync(".theme-built { stroke: blue; }");
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    tspan.setAttribute("class", "style-built");
    svg.appendChild(tspan);
    if (svg.querySelectorAll("tspan.style-built").length !== 1) throw new Error("compound selector failed");
    const css = [...sheet.cssRules, ...theme.cssRules].map((rule) => rule.cssText).join("\n");
    return { svg: `<svg id="${id}"><style>${css}</style><text class="style-built">ok</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_with_descendant_text_anchor() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("id", id);
    const style = document.createElementNS("http://www.w3.org/2000/svg", "style");
    style.textContent = `#${id} .flowchart-label text { text-anchor: middle; }`;
    svg.appendChild(style);
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.setAttribute("class", "flowchart-label");
    const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
    const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
    tspan.setAttribute("x", "0");
    tspan.textContent = "Markdown";
    text.appendChild(tspan);
    group.appendChild(text);
    svg.appendChild(group);
    document.body.appendChild(svg);
    const box = text.getBBox();
    svg.remove();
    return { svg: `<svg id="${id}"><text>${box.x}:${box.width}</text></svg>` };
  }
};
"#
    }

    fn fake_mermaid_with_er_viewbox() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="erDiagram" style="max-width: 164.525px;" viewBox="-8 -8 164.525 535.5" role="graphics-document document" aria-roledescription="er"><g><g class="root"><g class="nodes"><g class="node default " id="${id}-entity-DOCUMENT-0" transform="translate(78.2625, 64.625)"><g class="outer-path" style=""><path d="M-64.9805 -56.625 L64.9805 -56.625 L64.9805 56.625 L-64.9805 56.625"></path></g></g><g class="node default " id="${id}-entity-SECTION-1" transform="translate(78.2625, 280.875)"><g class="outer-path" style=""><path d="M-70.2625 -56.625 L70.2625 -56.625 L70.2625 56.625 L-70.2625 56.625"></path></g></g><g class="node default " id="${id}-entity-DIAGRAM-2" transform="translate(78.2625, 480)"><rect class="basic label-container" x="-52.3135" y="-39.5" width="104.627" height="79"></rect></g></g></g></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_state_viewbox() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" style="max-width: 184.711px;" viewBox="-8 -8 184.711 295" role="graphics-document document" aria-roledescription="stateDiagram"><g class="node statediagram-state "><g class="label" style="" transform="translate(1, -9.5)"><text y="-10.1" style="">Pending</text></g></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_requirement_viewbox() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="requirementDiagram" style="max-width: 181.737px;" viewBox="-8 -8 181.737 390" role="graphics-document document" aria-roledescription="requirement"><g class="label" data-id="edge" transform="translate(0, -9.5)"><rect class="background" style="" x="-47.75500000000001" y="-2" width="95.51000000000002" height="23"></rect></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_localized_requirement_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="162" class="requirementDiagram" style="max-width: 162px;" viewBox="0 0 161.212 382" role="graphics-document document" aria-roledescription="requirement"><g class="node default " transform="translate(80.60600000000001, 62.5)"><path d="M-69.0515 -54.5 L69.0515 -54.5 L69.0515 54.5 L-69.0515 54.5"></path><g class="label" transform="translate(-20.486517499999998, -22)"><text>テスト要件</text></g></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_class_enumeration_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="classDiagram" style="max-width: 231.55450000000002px;" viewBox="0 0 231.55450000000002 377" role="graphics-document document" aria-roledescription="classDiagram"><g class="root" transform="translate(1, 1)"><g transform="translate(2, 2)"><text>PreviewPane RenderedSection «enumeration»</text></g><path d="M0"></path><path d="M1"></path><path d="M2"></path><path d="M3"></path><path d="M4"></path><path d="M5"></path><path d="M6"></path><path d="M7"></path><path d="M8"></path><path d="M9"></path><path d="M10"></path><path d="M11"></path><path d="M12"></path><path d="M13"></path><path d="M14"></path></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_mindmap_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const labels = ["mindmap", "Origins", "Research", "Tools", ...Array.from({ length: 16 }, (_, index) => `item-${index}`), "wrapped"];
    const texts = labels.map((label) => `<text>${label}</text>`).join("");
    return { svg: `<svg id="${id}" width="100%" class="mindmap" style="max-width: 900px;" viewBox="0 0 900 900" role="graphics-document document" aria-roledescription="mindmap"><g transform="translate(1, 2)"></g><path d="M0"></path><path d="M1"></path><path d="M2"></path><path d="M3"></path>${texts}</svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_simple_mindmap_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" class="mindmap" style="max-width: 900px;" viewBox="0 0 900 900" role="graphics-document document" aria-roledescription="mindmap"><g transform="translate(1, 2)"></g><path d="M0"></path><path d="M1"></path><path d="M2"></path><path d="M3"></path><text>Mermaid</text><text>Runtime</text><text>Rasterize</text><text>Quality</text></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_japanese_sequence_activation_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="595" height="355" viewBox="-50 -10 595 355" role="graphics-document document" aria-roledescription="sequence"><g><rect x="415" y="113" width="10" height="144" class="activation0"></rect></g><g><rect x="420" y="163" width="10" height="46" class="activation1"></rect></g><text x="244" y="80" class="messageText">こんにちは鈴木さん、お元気ですか？</text><line x1="76" y1="111" x2="412" y2="111" data-et="message"></line><text x="247" y="172" class="messageText">こんにちは田中さん、聞こえますよ！</text><line x1="415" y1="205" x2="79" y2="205" data-et="message"></line><line x1="415" y1="253" x2="79" y2="253" data-et="message"></line></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_english_sequence_activation_layout() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="500" height="355" viewBox="-50 -10 500 355" role="graphics-document document" aria-roledescription="sequence"><g><rect x="250" y="269" width="150" height="65" class="actor actor-bottom"></rect><line id="actor1" x1="325" y1="65" x2="325" y2="269"></line><rect x="250" y="0" width="150" height="65" class="actor actor-top"></rect></g><g><rect x="320" y="113" width="10" height="144" class="activation0"></rect></g><g><rect x="325" y="163" width="10" height="46" class="activation1"></rect></g><text x="197" y="80" class="messageText">Hello John, how are you?</text><line x1="76" y1="111" x2="317" y2="111" data-et="message"></line><text x="200" y="172" class="messageText">Hi Alice, I can hear you!</text><line x1="320" y1="205" x2="79" y2="205" data-et="message"></line><line x1="320" y1="253" x2="79" y2="253" data-et="message"></line></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_c4_review_svg() -> &'static str {
        r##"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    return { svg: `<svg id="${id}" width="100%" style="max-width: 300px;" viewBox="0 0 300 200" role="graphics-document document" aria-roledescription="c4"><g><text>C4 Review</text></g></svg>` };
  }
};
"##
    }

    fn fake_mermaid_with_html_line_break() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id) => {
    const node = document.createElement("div");
    node.innerHTML = "On effectiveness<br/>and features";
    return { svg: `<svg id="${id}"><text>${node.innerText}</text></svg>` };
  }
};
"#
    }

    fn fake_zenuml() -> &'static str {
        r#"globalThis["mermaid-zenuml"] = { id: "registered" };"#
    }
}
