/// Generates a Mermaid `sequenceDiagram` from the call graph.
use std::collections::{HashMap, HashSet};
use crate::parser::CallGraph;

// ---------------------------------------------------------------------------
// DFS sequence events (also consumed by the SVG renderer)
// ---------------------------------------------------------------------------

pub enum SeqEvent {
    Call   { from: String, to: String, label: String },
    Return { from: String, to: String, label: String },
}

/// Traverse the call graph in DFS order and produce interleaved
/// Call + Return events, preserving proper activation nesting.
pub fn build_events(graph: &CallGraph) -> Vec<SeqEvent> {
    if graph.nodes.is_empty() {
        return vec![];
    }

    // adjacency: from_id -> [(to_id, label)]
    let mut adj: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
    for edge in &graph.edges {
        adj.entry(edge.from.as_str())
            .or_default()
            .push((edge.to.as_str(), edge.label.as_str()));
    }

    // id -> class name
    let node_map: HashMap<&str, &str> = graph
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.class.as_str()))
        .collect();
    let class_of = |id: &str| node_map.get(id).copied().unwrap_or("?");

    // id -> return expression (for return-arrow labels)
    let return_map: HashMap<&str, (&str, &str)> = graph
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), (n.return_type.as_str(), n.return_expr.as_str())))
        .collect();
    // Priority: declared return type > return expression > method name
    let return_label_of = |id: &str, method_name: &str| -> String {
        let (rtype, expr) = return_map.get(id).copied().unwrap_or(("", ""));
        if !rtype.is_empty() { return rtype.to_string(); }
        if !expr.is_empty()  { return expr.to_string(); }
        method_name.to_string()
    };

    // Iterative DFS.
    // Stack item: (to_id, from_class, label, is_return)
    let mut stack: Vec<(String, String, String, bool)> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut events: Vec<SeqEvent> = Vec::new();

    visited.insert(graph.entry.clone());
    let entry_class = class_of(&graph.entry).to_string();

    if let Some(children) = adj.get(graph.entry.as_str()) {
        for (cid, clabel) in children.iter().rev() {
            stack.push((cid.to_string(), entry_class.clone(), clabel.to_string(), false));
        }
    }

    while let Some((to_id, from_class, label, is_return)) = stack.pop() {
        let to_class = class_of(&to_id).to_string();
        if is_return {
            events.push(SeqEvent::Return { from: to_class, to: from_class, label });
        } else {
            events.push(SeqEvent::Call {
                from: from_class.clone(),
                to: to_class.clone(),
                label: label.clone(),
            });
            // Schedule the matching return (processed after all of to_id's children)
            let ret_label = return_label_of(&to_id, &label);
            stack.push((to_id.clone(), from_class, ret_label, true));
            if !visited.contains(&to_id) {
                visited.insert(to_id.clone());
                if let Some(children) = adj.get(to_id.as_str()) {
                    for (cid, clabel) in children.iter().rev() {
                        stack.push((
                            cid.to_string(),
                            to_class.clone(),
                            clabel.to_string(),
                            false,
                        ));
                    }
                }
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// PlantUML text output
// ---------------------------------------------------------------------------

pub fn sequence_plantuml(graph: &CallGraph) -> String {
    // Participants in BFS / insertion order, aliased as p0, p1, ...
    let mut seen: Vec<String> = Vec::new();
    for node in &graph.nodes {
        if !seen.contains(&node.class) {
            seen.push(node.class.clone());
        }
    }

    let alias = |name: &str| -> String {
        seen.iter()
            .position(|s| s == name)
            .map(|i| format!("p{i}"))
            .unwrap_or_else(|| "px".to_string())
    };

    let mut out = String::from(
        "@startuml\n\
         skinparam responseMessageBelowArrow true\n\
         skinparam sequenceArrowThickness 1.5\n\
         skinparam roundcorner 5\n\
         skinparam maxMessageSize 200\n\
         skinparam sequenceGroupBodyBackgroundColor transparent\n\n"
    );

    for (i, name) in seen.iter().enumerate() {
        out.push_str(&format!("participant \"{}\" as p{}\n", name, i));
    }
    out.push('\n');

    // Entry-point is activated before any events
    let entry_class = graph.nodes.first().map(|n| n.class.as_str()).unwrap_or("");
    if !entry_class.is_empty() {
        out.push_str(&format!("activate {}\n", alias(entry_class)));
    }

    for event in &build_events(graph) {
        match event {
            SeqEvent::Call { from, to, label } => {
                out.push_str(&format!(
                    "{} -> {} : {}()\nactivate {}\n",
                    alias(from), alias(to), label, alias(to)
                ));
            }
            SeqEvent::Return { from, to, label } => {
                out.push_str(&format!(
                    "{} --> {} : {}\ndeactivate {}\n",
                    alias(from), alias(to), label, alias(from)
                ));
            }
        }
    }

    if !entry_class.is_empty() {
        out.push_str(&format!("deactivate {}\n", alias(entry_class)));
    }

    out.push_str("@enduml\n");
    out
}

