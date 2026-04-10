mod svg;

use std::fs;
use std::path::Path;
use std::process::Command;
use crate::parser::CallGraph;
use crate::diagram::{sequence_plantuml, classflow_dot};

const VIEWER_TEMPLATE: &str = include_str!("../../viewer/viewer.html");
const SVGPANZOOM_JS: &str   = include_str!("../../viewer/svg-pan-zoom.min.js");

/// Write all output files into `out_dir`:
///   sequence.puml  — PlantUML source
///   classflow.dot  — Graphviz DOT source
///   sequence.svg   — rendered by `plantuml`; native renderer as fallback
///   classflow.svg  — rendered by `dot`; native renderer as fallback
///   sequenceViewer.html  — self-contained viewer with embedded SVG
///   classflowViewer.html — self-contained viewer with embedded SVG
///   svg-pan-zoom.min.js  — pan/zoom library for the viewers
pub fn write_diagrams(lang: &str, endpoint: &str, graph: &CallGraph, out_dir: &str) {
    fs::create_dir_all(out_dir).unwrap_or_default();

    let seq_puml = sequence_plantuml(graph);
    let cf_dot   = classflow_dot(lang, graph);

    write_file(out_dir, "sequence.puml", &seq_puml);
    write_file(out_dir, "classflow.dot",  &cf_dot);

    // sequence.svg — prefer plantuml, fall back to native renderer
    let seq_svg_path = Path::new(out_dir).join("sequence.svg");
    if render_plantuml(out_dir, "sequence.puml") && seq_svg_path.exists() {
        println!("  rendered sequence.svg via plantuml");
    } else {
        eprintln!("  plantuml unavailable or failed — using native SVG renderer");
        write_file(out_dir, "sequence.svg", &svg::sequence_svg(
            &format!("Sequence Diagram — {}", endpoint), graph));
    }

    // classflow.svg — prefer dot, fall back to native renderer
    if render_dot(out_dir, "classflow.dot", "classflow.svg") {
        println!("  rendered classflow.svg via dot (Graphviz)");
    } else {
        eprintln!("  dot unavailable or failed — using native SVG renderer");
        write_file(out_dir, "classflow.svg", &svg::classflow_svg(
            &format!("Class Flow — {}", endpoint), lang, graph));
    }

    // Write the pan/zoom JS library once
    write_file(out_dir, "svg-pan-zoom.min.js", SVGPANZOOM_JS);

    // Generate self-contained viewer HTML files
    write_viewer(out_dir, "sequence.svg",  "sequenceViewer.html");
    write_viewer(out_dir, "classflow.svg", "classflowViewer.html");
}

/// Calls `plantuml -tsvg -o <abs_dir> <abs_puml>`.
/// On Windows, delegates via `cmd /C <path>` because plantuml is typically
/// installed as a `.cmd` batch wrapper which CreateProcessW cannot run directly.
/// Returns `true` if the process exits successfully.
fn render_plantuml(out_dir: &str, puml_name: &str) -> bool {
    let abs_dir = match Path::new(out_dir).canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let puml_path = abs_dir.join(puml_name);
    let puml_str  = puml_path.to_str().unwrap_or("");
    let dir_str   = abs_dir.to_str().unwrap_or(".");

    let mut cmd = plantuml_command();
    cmd.args(["-tsvg", "-o", dir_str, puml_str]);

    match cmd.output() {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.is_empty() {
                eprintln!("  plantuml stderr: {}", stderr.trim());
            }
            false
        }
        Err(e) => {
            eprintln!("  plantuml not found or could not start: {}", e);
            false
        }
    }
}

/// Builds a `Command` that invokes plantuml.
///
/// Search order:
///   1. Next to the current executable (supports placing plantuml.bat/cmd/jar
///      alongside code-mole.exe so both can be deployed as a bundle).
///   2. System PATH (global installation).
fn plantuml_command() -> Command {
    // --- look next to the running executable ---
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        #[cfg(windows)]
        let candidates: &[&str] = &["plantuml.cmd", "plantuml.bat", "plantuml.exe"];
        #[cfg(not(windows))]
        let candidates: &[&str] = &["plantuml"];

        for name in candidates {
            let candidate = exe_dir.join(name);
            if candidate.exists() {
                // .cmd / .bat files must go through cmd /C on Windows
                #[cfg(windows)]
                if name.ends_with(".cmd") || name.ends_with(".bat") {
                    let mut c = Command::new("cmd");
                    c.args([std::ffi::OsStr::new("/C"),  candidate.as_os_str()]);
                    return c;
                }
                return Command::new(&candidate);
            }
        }
    }

    // --- fall back to PATH ---
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.args(["/C", "plantuml"]);
        c
    }
    #[cfg(not(windows))]
    Command::new("plantuml")
}

/// Calls `dot -Tsvg -o <abs_svg> <abs_dot>`.
/// Returns `true` if process exits 0 and the SVG file exists afterwards.
fn render_dot(out_dir: &str, dot_name: &str, svg_name: &str) -> bool {
    let abs_dir = match Path::new(out_dir).canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let dot_path = abs_dir.join(dot_name);
    let svg_path = abs_dir.join(svg_name);
    let ok = Command::new("dot")
        .args([
            "-Tsvg",
            "-o",
            svg_path.to_str().unwrap_or(""),
            dot_path.to_str().unwrap_or(""),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok && svg_path.exists()
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
    let html = VIEWER_TEMPLATE.replace("<!-- SVG_PLACEHOLDER -->", &svg_content);
    write_file(out_dir, viewer_name, &html);
}

