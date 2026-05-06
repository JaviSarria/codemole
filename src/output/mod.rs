mod svg;

use std::fs;
use std::path::Path;
use std::process::Command;
use crate::parser::{CallGraph, invoke_java_parser, JavaIndex};
use crate::diagram::{
    sequence_mermaid, build_seq_nodes_java, sequence_mermaid_structured,
    classflow_dot, build_component_diagram, component_dot,
    build_dependency_diagram, dependency_dot, dependency_metrics_text,
};

const VIEWER_TEMPLATE: &str = include_str!("../../viewer/viewer.html");
const SVGPANZOOM_JS: &str   = include_str!("../../viewer/svg-pan-zoom.min.js");

/// Write all output files into `out_dir`:
///   diagrams/sequence.md    — Mermaid sequence diagram source
///   diagrams/classflow.dot  — Graphviz DOT source
///   diagrams/component.dot  — Graphviz DOT source for component/module diagram
///   diagrams/dependency.dot — Graphviz DOT source for package dependency diagram
///   sequence.svg            — rendered by the built-in SVG renderer
///   classflow.svg           — rendered by `dot` (Graphviz); native renderer as fallback
///   component.svg           — rendered by `dot` (Graphviz); native renderer as fallback
///   dependency.svg          — rendered by `dot` (Graphviz); native renderer as fallback
///   sequenceViewer.html     — self-contained viewer with embedded SVG
///   classflowViewer.html    — self-contained viewer with embedded SVG
///   componentViewer.html    — self-contained viewer with embedded SVG
///   dependencyViewer.html   — self-contained viewer with embedded SVG
///   svg-pan-zoom.min.js     — pan/zoom library for the viewers
pub fn write_diagrams(
    lang:     &str,
    endpoint: &str,
    src_root: &str,
    graph:    &CallGraph,
    out_dir:  &str,
    jar_path: &str,
) {
    fs::create_dir_all(out_dir).unwrap_or_default();

    // Diagram sources go into a sub-folder
    let diag_dir = Path::new(out_dir).join("diagrams");
    fs::create_dir_all(&diag_dir).unwrap_or_default();
    let diag_dir = diag_dir.to_string_lossy().into_owned();

    // For Java, try CFG-enriched sequence (alt/loop blocks) via the JAR.
    // Falls back to the flat renderer when the JAR is unavailable or fails.
    let seq_md = if lang == "java" && !jar_path.is_empty()
        && Path::new(jar_path).exists()
    {
        build_structured_sequence(graph, src_root, jar_path)
            .unwrap_or_else(|| sequence_mermaid(graph))
    } else {
        sequence_mermaid(graph)
    };
    let cf_dot     = classflow_dot(lang, graph);
    let comp_diag  = build_component_diagram(lang, graph);
    let comp_dot   = component_dot(&comp_diag);
    let dep_diag        = build_dependency_diagram(lang, src_root, graph);
    let dep_dot_src     = dependency_dot(&dep_diag);
    let dep_metrics_txt = dependency_metrics_text(&dep_diag);

    // ── Diagram sources → diagrams/ ─────────────────────────────────────────
    write_file(&diag_dir, "sequence.md",           &seq_md);
    write_file(&diag_dir, "classflow.dot",          &cf_dot);
    write_file(&diag_dir, "component.dot",          &comp_dot);
    write_file(&diag_dir, "dependency.dot",         &dep_dot_src);
    write_file(&diag_dir, "dependency_metrics.txt", &dep_metrics_txt);

    // ── SVG renders → out_dir/ ──────────────────────────────────────────────

    // sequence.svg — built-in native renderer
    let seq_svg = svg::sequence_svg(&format!("Sequence Diagram — {}", endpoint), graph);
    write_file(out_dir, "sequence.svg", &seq_svg);

    // classflow.svg — prefer dot (Graphviz), fall back to native renderer
    if render_dot(&diag_dir, out_dir, "classflow.dot", "classflow.svg") {
        println!("  rendered classflow.svg via dot (Graphviz)");
    } else {
        eprintln!("  dot unavailable or failed — using native SVG renderer for classflow");
        write_file(out_dir, "classflow.svg", &svg::classflow_svg(
            &format!("Class Flow — {}", endpoint), lang, graph));
    }

    // component.svg — prefer dot (Graphviz), fall back to native renderer
    if render_dot(&diag_dir, out_dir, "component.dot", "component.svg") {
        println!("  rendered component.svg via dot (Graphviz)");
    } else {
        eprintln!("  dot unavailable or failed — using native SVG renderer for component");
        write_file(out_dir, "component.svg", &svg::component_svg(
            &format!("Component Diagram — {}", endpoint), &comp_diag));
    }

    // dependency.svg — prefer dot (Graphviz), fall back to native renderer
    if render_dot(&diag_dir, out_dir, "dependency.dot", "dependency.svg") {
        println!("  rendered dependency.svg via dot (Graphviz)");
    } else {
        eprintln!("  dot unavailable or failed — using native SVG renderer for dependency");
        write_file(out_dir, "dependency.svg", &svg::dependency_svg(
            &format!("Dependency Diagram — {}", endpoint), &dep_diag));
    }

    // ── Viewer files → out_dir/ ─────────────────────────────────────────────
    write_file(out_dir, "svg-pan-zoom.min.js", SVGPANZOOM_JS);

    write_viewer(out_dir, "sequence.svg",   "sequenceViewer.html");
    write_viewer(out_dir, "classflow.svg",  "classflowViewer.html");
    write_viewer(out_dir, "component.svg",  "componentViewer.html");
    write_viewer(out_dir, "dependency.svg", "dependencyViewer.html");
}

/// Calls `dot -Tsvg -o <svg_dir/svg_name> <dot_dir/dot_name>`.
/// Returns `true` if the SVG file was produced with content.
fn render_dot(dot_dir: &str, svg_dir: &str, dot_name: &str, svg_name: &str) -> bool {
    let abs_dot_dir = match Path::new(dot_dir).canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let abs_svg_dir = match Path::new(svg_dir).canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let dot_path = abs_dot_dir.join(dot_name);
    let svg_path = abs_svg_dir.join(svg_name);
    let _ = Command::new("dot")
        .args([
            "-Tsvg",
            "-o",
            svg_path.to_str().unwrap_or(""),
            dot_path.to_str().unwrap_or(""),
        ])
        .status();
    // Accept the output as long as a non-empty SVG was written, even when dot
    // exits non-zero due to non-fatal warnings.
    svg_path.metadata().map(|m| m.len() > 0).unwrap_or(false)
}

fn write_file(dir: &str, name: &str, content: &str) {
    let path = Path::new(dir).join(name);
    match fs::write(&path, content) {
        Ok(_) => println!("  wrote {}", path.display()),
        Err(e) => eprintln!("  error writing {}: {}", path.display(), e),
    }
}

fn write_viewer(out_dir: &str, svg_name: &str, viewer_name: &str) {
    let svg_path = Path::new(out_dir).join(svg_name);
    let svg_content = match fs::read_to_string(&svg_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("  error reading {} for viewer: {}", svg_path.display(), e);
            return;
        }
    };
    let html = VIEWER_TEMPLATE
        .replace("<!-- SVG_PLACEHOLDER -->", &svg_content)
        .replace("<!-- SVGPANZOOM_PLACEHOLDER -->", SVGPANZOOM_JS);
    write_file(out_dir, viewer_name, &html);
}

// ---------------------------------------------------------------------------
// Java CFG-enriched sequence builder
// ---------------------------------------------------------------------------

/// Invoke the Java parser JAR and build a structured PlantUML with `alt`/`loop`
/// blocks. Returns `None` when the JAR fails or produces no useful data.
fn build_structured_sequence(
    graph:    &CallGraph,
    src_root: &str,
    jar_path: &str,
) -> Option<String> {
    let prog  = invoke_java_parser(src_root, jar_path)?;
    let index = JavaIndex::build(prog);

    let entry = graph.nodes.first()?;
    let nodes = build_seq_nodes_java(&entry.class, &entry.method, &index);

    if nodes.is_empty() { return None; }

    Some(sequence_mermaid_structured(&entry.class, &nodes))
}
