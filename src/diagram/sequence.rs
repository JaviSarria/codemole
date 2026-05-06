/// Generates a Mermaid sequence diagram from the call graph.
use std::collections::{HashMap, HashSet, VecDeque};
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
// Mermaid text output
// ---------------------------------------------------------------------------

pub fn sequence_mermaid(graph: &CallGraph) -> String {
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

    let mut out = String::from("sequenceDiagram\n");

    for (i, name) in seen.iter().enumerate() {
        out.push_str(&format!("    participant \"{}\" as p{}\n", name, i));
    }
    out.push('\n');

    // Entry-point is activated before any events
    let entry_class = graph.nodes.first().map(|n| n.class.as_str()).unwrap_or("");
    if !entry_class.is_empty() {
        out.push_str(&format!("    activate {}\n", alias(entry_class)));
    }

    for event in &build_events(graph) {
        match event {
            SeqEvent::Call { from, to, label } => {
                out.push_str(&format!(
                    "    {}->>{}: {}()\n    activate {}\n",
                    alias(from), alias(to), label, alias(to)
                ));
            }
            SeqEvent::Return { from, to, label } => {
                out.push_str(&format!(
                    "    {}-->>{}: {}\n    deactivate {}\n",
                    alias(from), alias(to), label, alias(from)
                ));
            }
        }
    }

    if !entry_class.is_empty() {
        out.push_str(&format!("    deactivate {}\n", alias(entry_class)));
    }

    out
}

// =============================================================================
// Structured sequence model (conditions + loops)
// =============================================================================

pub struct SeqMessage {
    pub from:      String,
    pub to:        String,
    pub label:     String,
    pub ret_label: String,
    pub body:      Vec<SeqNode>,
}

pub enum SeqNode {
    Message(SeqMessage),
    Alt {
        condition:   String,
        then_branch: Vec<SeqNode>,
        else_branch: Vec<SeqNode>,
    },
    Loop {
        label: String,
        body:  Vec<SeqNode>,
    },
}

// =============================================================================
// CFG-based structured sequence builder
// =============================================================================

use crate::parser::java_ir::{JavaIndex, JCfg};

/// Build a structured sequence tree starting from `(entry_class, entry_method)`.
/// Returns an empty `Vec` if the JAR index has no CFG data for the entry.
pub fn build_seq_nodes_java(
    entry_class:  &str,
    entry_method: &str,
    index:        &JavaIndex,
) -> Vec<SeqNode> {
    let mut visited: HashSet<(String, String)> = HashSet::new();
    walk_function(entry_class, entry_method, index, &mut visited, 0)
}

fn walk_function(
    class:   &str,
    method:  &str,
    index:   &JavaIndex,
    visited: &mut HashSet<(String, String)>,
    depth:   usize,
) -> Vec<SeqNode> {
    if depth > 20 { return vec![]; }
    let key = (class.to_string(), method.to_string());
    if visited.contains(&key) { return vec![]; }
    visited.insert(key);

    if let Some(cfg) = index.get_cfg(class, method) {
        let entry = cfg.entry_block.clone();
        let exit  = cfg.exit_block.clone();
        walk_cfg_block(cfg, &entry, &exit, class, index, visited, &mut HashSet::new(), depth)
    } else {
        vec![]
    }
}

fn walk_cfg_block(
    cfg:           &JCfg,
    block_id:      &str,
    exit_id:       &str,
    caller_class:  &str,
    index:         &JavaIndex,
    node_visited:  &mut HashSet<(String, String)>,
    block_visited: &mut HashSet<String>,
    depth:         usize,
) -> Vec<SeqNode> {
    if block_id == exit_id || block_visited.contains(block_id) {
        return vec![];
    }
    block_visited.insert(block_id.to_string());

    let block = match cfg.blocks.iter().find(|b| b.id == block_id) {
        Some(b) => b,
        None    => return vec![],
    };

    let mut nodes = Vec::<SeqNode>::new();

    // ── Step 1: process all statements ────────────────────────────────────────
    let has_loop_stmt = block.statements.iter().any(|s| s.kind == "loop");
    let mut condition_expr: Option<String> = None;

    for stmt in &block.statements {
        match stmt.kind.as_str() {
            "call" => {
                if let Some(cid) = &stmt.call_id {
                    if let Some((callee_class, callee_method)) = index.callee_of(cid) {
                        let ret  = index.return_label_of(callee_class, callee_method);
                        let body = walk_function(
                            callee_class, callee_method, index, node_visited, depth + 1,
                        );
                        nodes.push(SeqNode::Message(SeqMessage {
                            from:      caller_class.to_string(),
                            to:        callee_class.to_string(),
                            label:     callee_method.to_string(),
                            ret_label: ret,
                            body,
                        }));
                    }
                }
            }
            "condition" => {
                condition_expr = stmt.expression.clone()
                    .or_else(|| Some("cond".to_string()));
            }
            _ => {}
        }
    }

    // ── Step 2: analyse edges for control-flow structure ──────────────────────
    let true_edge  = block.edges.iter().find(|e| e.edge_type == "true");
    let false_edge = block.edges.iter().find(|e| e.edge_type == "false");

    if has_loop_stmt {
        // ── Loop ──────────────────────────────────────────────────────────────
        if let (Some(te), Some(fe)) = (true_edge, false_edge) {
            let body_id  = te.to.clone();
            let after_id = fe.to.clone();

            let body_nodes = walk_cfg_block(
                cfg, &body_id, exit_id, caller_class,
                index, node_visited, &mut block_visited.clone(), depth,
            );
            nodes.push(SeqNode::Loop { label: "loop".to_string(), body: body_nodes });

            let rest = walk_cfg_block(
                cfg, &after_id, exit_id, caller_class,
                index, node_visited, block_visited, depth,
            );
            nodes.extend(rest);
        }
    } else if let Some(cond) = condition_expr {
        // ── If / Alt ──────────────────────────────────────────────────────────
        if let (Some(te), Some(fe)) = (true_edge, false_edge) {
            let then_id = te.to.clone();
            let else_id = fe.to.clone();

            let merge_id   = find_merge(cfg, &then_id, &else_id);
            let merge_exit = merge_id.as_deref().unwrap_or(exit_id);

            let then_branch = walk_cfg_block(
                cfg, &then_id, merge_exit, caller_class,
                index, node_visited, &mut block_visited.clone(), depth,
            );
            let else_branch = walk_cfg_block(
                cfg, &else_id, merge_exit, caller_class,
                index, node_visited, &mut block_visited.clone(), depth,
            );

            nodes.push(SeqNode::Alt { condition: cond, then_branch, else_branch });

            if let Some(mid) = merge_id {
                let rest = walk_cfg_block(
                    cfg, &mid, exit_id, caller_class,
                    index, node_visited, block_visited, depth,
                );
                nodes.extend(rest);
            }
        }
    } else {
        // ── Normal continuation ───────────────────────────────────────────────
        for edge in &block.edges {
            if edge.edge_type == "normal" && edge.to != exit_id {
                let rest = walk_cfg_block(
                    cfg, &edge.to, exit_id, caller_class,
                    index, node_visited, block_visited, depth,
                );
                nodes.extend(rest);
            }
        }
    }

    nodes
}

/// Find the merge block after a two-way branch: the first block in CFG order
/// that is reachable from BOTH `left_id` and `right_id`.
fn find_merge(cfg: &JCfg, left_id: &str, right_id: &str) -> Option<String> {
    let left_reach  = reachable_blocks(cfg, left_id);
    let right_reach = reachable_blocks(cfg, right_id);
    cfg.blocks.iter()
        .find(|b| left_reach.contains(&b.id) && right_reach.contains(&b.id))
        .map(|b| b.id.clone())
}

fn reachable_blocks(cfg: &JCfg, start: &str) -> HashSet<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue:   VecDeque<String> = VecDeque::new();
    queue.push_back(start.to_string());
    while let Some(id) = queue.pop_front() {
        if visited.contains(&id) { continue; }
        visited.insert(id.clone());
        if let Some(block) = cfg.blocks.iter().find(|b| b.id == id) {
            for edge in &block.edges {
                // Do not follow back-edges or exit edges when computing merge
                if edge.edge_type != "loop"
                    && edge.edge_type != "return"
                    && edge.edge_type != "exception"
                {
                    queue.push_back(edge.to.clone());
                }
            }
        }
    }
    visited
}

// =============================================================================
// Participant collection
// =============================================================================

pub fn collect_participants(nodes: &[SeqNode], out: &mut Vec<String>) {
    for node in nodes {
        match node {
            SeqNode::Message(msg) => {
                if !out.contains(&msg.from) { out.push(msg.from.clone()); }
                if !out.contains(&msg.to)   { out.push(msg.to.clone()); }
                collect_participants(&msg.body, out);
            }
            SeqNode::Alt { then_branch, else_branch, .. } => {
                collect_participants(then_branch, out);
                collect_participants(else_branch, out);
            }
            SeqNode::Loop { body, .. } => {
                collect_participants(body, out);
            }
        }
    }
}

// =============================================================================
// Structured Mermaid renderer
// =============================================================================

/// Render a Mermaid `sequenceDiagram` with `alt` / `loop` blocks built from CFG data.
pub fn sequence_mermaid_structured(entry_class: &str, nodes: &[SeqNode]) -> String {
    let mut participants: Vec<String> = vec![entry_class.to_string()];
    collect_participants(nodes, &mut participants);

    let mut out = String::from("sequenceDiagram\n");

    for (i, name) in participants.iter().enumerate() {
        out.push_str(&format!("    participant \"{}\" as p{}\n", name, i));
    }
    out.push('\n');

    let alias = |name: &str| -> String {
        participants.iter().position(|s| s == name)
            .map(|i| format!("p{i}"))
            .unwrap_or_else(|| "px".to_string())
    };

    let entry_alias = alias(entry_class);
    out.push_str(&format!("    activate {entry_alias}\n"));
    out.push_str(&render_nodes(nodes, &alias, 0));
    out.push_str(&format!("    deactivate {entry_alias}\n"));
    out
}

fn render_nodes(
    nodes: &[SeqNode],
    alias: &dyn Fn(&str) -> String,
    depth: usize,
) -> String {
    let pad = "  ".repeat(depth);
    let mut out = String::new();

    for node in nodes {
        match node {
            SeqNode::Message(msg) => {
                let from = alias(&msg.from);
                let to   = alias(&msg.to);
                out.push_str(&format!(
                    "{pad}    {}->>{}: {}()\n{pad}    activate {to}\n",
                    from, to, msg.label,
                ));
                out.push_str(&render_nodes(&msg.body, alias, depth + 1));
                out.push_str(&format!(
                    "{pad}    {}-->>{}: {}\n{pad}    deactivate {to}\n",
                    to, from, msg.ret_label,
                ));
            }
            SeqNode::Alt { condition, then_branch, else_branch } => {
                out.push_str(&format!("{pad}    alt {condition}\n"));
                out.push_str(&render_nodes(then_branch, alias, depth + 1));
                if !else_branch.is_empty() {
                    out.push_str(&format!("{pad}    else\n"));
                    out.push_str(&render_nodes(else_branch, alias, depth + 1));
                }
                out.push_str(&format!("{pad}    end\n"));
            }
            SeqNode::Loop { label, body } => {
                out.push_str(&format!("{pad}    loop {label}\n"));
                out.push_str(&render_nodes(body, alias, depth + 1));
                out.push_str(&format!("{pad}    end\n"));
            }
        }
    }
    out
}
