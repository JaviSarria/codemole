/// Generates a Graphviz DOT digraph (Java) or DOT for call-flow (Python/Go).
use std::collections::{HashMap, HashSet};
use crate::parser::CallGraph;

pub fn classflow_dot(lang: &str, graph: &CallGraph) -> String {
    match lang {
        "java" => class_dot(graph),
        _ => flow_dot(graph),
    }
}

// ---------------------------------------------------------------------------
// Class diagram (Java) → Graphviz DOT with HTML-like record labels
// ---------------------------------------------------------------------------

fn class_dot(graph: &CallGraph) -> String {
    // Group methods by class (insertion order)
    let mut classes: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut class_seen: HashMap<&str, usize> = HashMap::new();
    for node in &graph.nodes {
        if let Some(&idx) = class_seen.get(node.class.as_str()) {
            if !classes[idx].1.contains(&node.method.as_str()) {
                classes[idx].1.push(&node.method);
            }
        } else {
            class_seen.insert(&node.class, classes.len());
            classes.push((&node.class, vec![&node.method]));
        }
    }

    let mut out = String::new();
    out.push_str("digraph classflow {\n");
    out.push_str("  graph [rankdir=TB, bgcolor=\"white\", splines=ortho, ");
    out.push_str("nodesep=0.8, ranksep=1.4, fontname=\"Helvetica\"];\n");
    out.push_str("  node [shape=none, margin=0, fontname=\"Helvetica\", fontsize=11];\n");
    out.push_str("  edge [arrowhead=open, color=\"#555555\", penwidth=1.2];\n\n");

    // Node definitions with HTML table labels
    for (name, methods) in &classes {
        out.push_str(&format!("  {} [id={}, label=<\n", dot_id(name), dot_id(name)));
        out.push_str("    <TABLE BORDER=\"1\" CELLBORDER=\"0\" CELLSPACING=\"0\" ");
        out.push_str("CELLPADDING=\"5\" BGCOLOR=\"#f0f7ff\" STYLE=\"ROUNDED\">\n");
        out.push_str(&format!(
            "      <TR><TD BGCOLOR=\"#4a90d9\"><FONT COLOR=\"white\"><B>{}</B></FONT></TD></TR>\n",
            html_escape(name)
        ));
        for m in methods {
            out.push_str(&format!(
                "      <TR><TD ALIGN=\"LEFT\">+ {}()</TD></TR>\n",
                html_escape(m)
            ));
        }
        out.push_str("    </TABLE>>];\n");
    }
    out.push('\n');

    // Edges — one per unique class pair
    let node_map: HashMap<&str, &crate::parser::Node> =
        graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut rels: HashSet<(String, String)> = HashSet::new();
    for edge in &graph.edges {
        let fc = node_map.get(edge.from.as_str()).map(|n| n.class.as_str()).unwrap_or("?");
        let tc = node_map.get(edge.to.as_str()).map(|n| n.class.as_str()).unwrap_or("?");
        if fc != tc {
            rels.insert((fc.to_string(), tc.to_string()));
        }
    }
    let mut rels: Vec<_> = rels.into_iter().collect();
    rels.sort();
    for (f, t) in rels {
        out.push_str(&format!("  {} -> {};\n", dot_id(&f), dot_id(&t)));
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Call-flow (Python / Go) → Graphviz DOT with simple box nodes
// ---------------------------------------------------------------------------

fn flow_dot(graph: &CallGraph) -> String {
    let mut out = String::new();
    out.push_str("digraph callflow {\n");
    out.push_str("  graph [rankdir=TB, bgcolor=\"white\", splines=ortho, ");
    out.push_str("nodesep=0.6, ranksep=1.0, fontname=\"Helvetica\"];\n");
    out.push_str("  node [shape=box, style=\"rounded,filled\", fillcolor=\"#e8f4fd\", ");
    out.push_str("color=\"#4a90d9\", fontname=\"Helvetica\", fontsize=10];\n");
    out.push_str("  edge [arrowhead=open, color=\"#555555\", fontsize=9, fontname=\"Helvetica\"];\n\n");

    let id_map: HashMap<&str, String> = graph.nodes.iter().enumerate()
        .map(|(i, n)| (n.id.as_str(), format!("n{i}")))
        .collect();

    for node in &graph.nodes {
        let sid = &id_map[node.id.as_str()];
        let label = format!("{}.{}()", node.class, node.method);
        out.push_str(&format!("  {} [label=\"{}\"];\n", sid, dot_escape(&label)));
    }
    out.push('\n');

    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    for edge in &graph.edges {
        let key = (edge.from.as_str(), edge.to.as_str());
        if seen.insert(key) {
            if let (Some(fi), Some(ti)) = (id_map.get(edge.from.as_str()), id_map.get(edge.to.as_str())) {
                out.push_str(&format!("  {} -> {} [label=\"{}\"];\n",
                    fi, ti, dot_escape(&edge.label)));
            }
        }
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// DOT helpers
// ---------------------------------------------------------------------------

/// Quoted DOT identifier safe for all node names.
fn dot_id(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\"")
                       .replace('\\', "\\\\"))
}

/// Escape a string for use inside a DOT double-quoted label.
fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
}

/// Escape HTML special characters inside a DOT HTML label.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
}

