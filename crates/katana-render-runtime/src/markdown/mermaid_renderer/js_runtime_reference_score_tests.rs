use super::MermaidJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;

const GIT_GRAPH_SOURCE: &str = "gitGraph
    commit id: \"base\"
    branch feature
    checkout feature
    commit id: \"rust-js\"
    checkout main
    merge feature";

const C4_DYNAMIC_SOURCE: &str = "C4Dynamic
    title Dynamic diagram for API Application
    Container(spa, \"Single Page Application\", \"javascript and react\")
    Container(api, \"API Application\", \"Java and Spring Boot\")
    Rel(spa, api, \"Uses\", \"JSON/HTTPS\")";

const C4_DYNAMIC_SOURCE_JA: &str = "C4Dynamic
    title APIアプリケーションのダイナミック図
    Container(spa, \"シングルページアプリケーション\", \"JavaScript と React\")
    Container(api, \"APIアプリケーション\", \"Java と Spring Boot\")
    Rel(spa, api, \"利用する\", \"JSON/HTTPS\")";

const C4_DEPLOYMENT_SOURCE: &str = "C4Deployment
    title Deployment diagram for Internet Banking System
    Deployment_Node(mob, \"Customer's mobile device\", \"Apple iOS or Android\") {
        Container(mobile, \"Mobile App\", \"Xamarin\")
    }";

const ARCHITECTURE_SERVICE_ICONS_SOURCE: &str = "architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service disk1(disk)[Storage] in api
    service disk2(disk)[Storage] in api
    service server(server)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db";

const ARCHITECTURE_SIMPLE_EN_SOURCE: &str = "architecture-beta
    group app(cloud)[KatanA]
    service markdown(server)[Markdown] in app
    service renderer(server)[Renderer] in app
    service svg(database)[SVG cache] in app
    markdown:R -- L:renderer
    renderer:R -- L:svg";

const ARCHITECTURE_SIMPLE_JA_SOURCE: &str = "architecture-beta
    group app(cloud)[KatanA]
    service markdown(server)[Markdown] in app
    service renderer(server)[レンダラー] in app
    service svg(database)[SVGキャッシュ] in app
    markdown:R -- L:renderer
    renderer:R -- L:svg";

#[test]
fn runtime_normalizes_git_graph_browser_bounds() {
    let rendered = render_mermaid_svg(GIT_GRAPH_SOURCE);
    assert_svg_contains(
        &rendered,
        r#"viewBox="-114.078125 -21.5 272.078125 161.97125244140625""#,
    );
    assert_svg_contains(&rendered, r#"style="max-width: 272.078125px;""#);
}

#[test]
fn runtime_normalizes_c4_dynamic_browser_bounds() {
    let rendered = render_mermaid_svg(C4_DYNAMIC_SOURCE);
    assert_svg_contains(&rendered, r#"viewBox="0 -70 832 412""#);
}

#[test]
fn runtime_normalizes_c4_dynamic_relation_positions() {
    let rendered = render_mermaid_svg(C4_DYNAMIC_SOURCE);
    assert_svg_contains(
        &rendered,
        r#"<line x1="366" y1="200.998008" x2="466" y2="200.865751""#,
    );
    assert_svg_contains(&rendered, r#"<text x="438" y="200.931879""#);
    assert_svg_contains(&rendered, r#"<text x="457.5" y="217.931879""#);
    assert_svg_contains(&rendered, r#"transform="translate(258, 202.625366)""#);
    assert_svg_contains(
        &rendered,
        r#"x="-108" y="-35.625366" width="216" height="71.250732421875""#,
    );
    assert_svg_contains(&rendered, ">Single</tspan>");
    assert_svg_contains(&rendered, "> Page</tspan>");
    assert_svg_contains(&rendered, "> Application</tspan>");
    assert_svg_contains(&rendered, "> Java</tspan>");
    assert_svg_contains(&rendered, "> Spring</tspan>");
    assert_svg_contains(&rendered, "> Boot]</tspan>");
}

#[test]
fn runtime_normalizes_c4_dynamic_relation_positions_ja_x_offsets() {
    let rendered = render_mermaid_svg(C4_DYNAMIC_SOURCE_JA);
    assert_svg_contains(&rendered, r#"transform="translate(258, 210.325366)""#);
    assert_svg_contains(&rendered, r#"width="216" height="86.650732421875""#);
    assert_svg_contains(&rendered, r#"transform="translate(574, 202.625366)""#);
    assert_svg_contains(&rendered, "x1=\"368\"");
    assert_svg_contains(&rendered, "x2=\"468\"");
    assert_svg_contains(&rendered, r#"<text x="448" y="204.781879""#);
    assert_svg_contains(&rendered, r#"<text x="459.5" y="221.781879""#);
    assert_svg_contains(&rendered, "シングルページアプリケー");
    assert_svg_contains(&rendered, ">ション</tspan>");
    assert_svg_contains(&rendered, "> JavaScript</tspan>");
    assert_svg_contains(&rendered, "> React]</tspan>");
    assert_svg_contains(&rendered, r#"<text x="166" y="20""#);
    assert_svg_contains(&rendered, "APIアプリケーションのダイナミック図");
    assert_svg_contains(&rendered, "APIアプリケーション");
    assert_svg_contains(&rendered, "> Spring</tspan>");
    assert_svg_contains(&rendered, "> Boot]</tspan>");
}

#[test]
fn runtime_normalizes_c4_deployment_browser_position_corrections() {
    let rendered = render_mermaid_svg(C4_DEPLOYMENT_SOURCE);
    assert_svg_contains(&rendered, r#"<rect x="150" y="122""#);
    assert_svg_contains(&rendered, r#"<text x="308" y="130""#);
    assert_svg_contains(&rendered, r#"<text x="308" y="153""#);
    assert_svg_contains(&rendered, r#"transform="translate(308, 259.625366)""#);
    assert_svg_contains(&rendered, ">Mobile</tspan>");
    assert_svg_contains(&rendered, "> App</tspan>");
    assert_svg_contains(&rendered, ">[Container:</tspan>");
    assert_svg_contains(&rendered, "> Xamarin]</tspan>");
}

#[test]
fn runtime_restores_architecture_service_icons_from_source_types() {
    let rendered = render_mermaid_svg(ARCHITECTURE_SERVICE_ICONS_SOURCE);
    assert_svg_contains(&rendered, r#"rx="20" ry="7.14""#);
    assert_svg_contains(&rendered, "l-4.83,13.22");
    assert_svg_contains(
        &rendered,
        r#"viewBox="-183.41357421875 -165.96131896972656 446.8271484375 462.922607421875""#,
    );
}

#[test]
fn runtime_normalizes_architecture_simple_renderer_coordinates_en() {
    let rendered = render_mermaid_svg(ARCHITECTURE_SIMPLE_EN_SOURCE);
    assert_svg_contains(
        &rendered,
        r#"viewBox="-282.6865234375 -65.5 647.373046875 262""#,
    );
    assert_svg_contains(
        &rendered,
        r#"transform="translate(-200.43652784647549,17)""#,
    );
    assert_svg_contains(&rendered, r#"transform="translate(0.25,17)""#);
    assert_svg_contains(&rendered, r#"transform="translate(200.93652784647549,17)""#);
    assert_svg_contains(&rendered, r#"<rect id="katana-mermaid-svg-"#);
    assert_svg_contains(&rendered, r#"class="text-outer-tspan row""#);
    assert_svg_contains(
        &rendered,
        r#"<path d="M -120.43652784647549,57 L -60.09326392323774,57 L0.25,57 ""#,
    );
    assert_svg_contains(
        &rendered,
        r#"<path d="M 80.25,57 L 140.59326392323774,57 L200.93652784647549,57 ""#,
    );
}

#[test]
fn runtime_normalizes_architecture_simple_renderer_coordinates_ja() {
    let rendered = render_mermaid_svg(ARCHITECTURE_SIMPLE_JA_SOURCE);
    assert_svg_contains(
        &rendered,
        r#"viewBox="-283.1865234375 -65.5 646.373046875 262""#,
    );
    assert_svg_contains(
        &rendered,
        r#"transform="translate(-200.68652784647549,17)""#,
    );
    assert_svg_contains(&rendered, r#"transform="translate(0,17)""#);
    assert_svg_contains(&rendered, r#"transform="translate(200.68652784647549,17)""#);
    assert_svg_contains(&rendered, r#"class="text-outer-tspan row""#);
    assert_svg_contains(
        &rendered,
        r#"<path d="M -120.68652784647549,57 L -60.34326392323774,57 L0,57 ""#,
    );
    assert_svg_contains(
        &rendered,
        r#"<path d="M 80,57 L 140.34326392323774,57 L200.68652784647549,57 ""#,
    );
}

fn render_mermaid_svg(source: &str) -> Result<String, String> {
    let mermaid = RuntimeAsset::mermaid();
    mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(source, &mermaid_js, DiagramColorPreset::dark())
        })
}

fn assert_svg_contains(rendered: &Result<String, String>, expected: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(expected)),
        "missing {expected:?} in {rendered:?}"
    );
}
