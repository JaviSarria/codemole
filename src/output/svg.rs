/// Native SVG renderer — zero external dependencies.
///
/// Implements three renderers:
///   SequenceSVG   — sequence diagram
///   FlowchartSVG  — for Python / Go (flowchart TD)
///   ClassSVG      — for Java (classDiagram)
use std::collections::HashMap;
use crate::parser::{CallGraph, Node};
use crate::diagram::{SeqEvent, build_events};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn sequence_svg(title: &str, graph: &CallGraph) -> String {
    render_sequence(title, graph)
}

pub fn classflow_svg(title: &str, lang: &str, graph: &CallGraph) -> String {
    if lang == "java" {
        render_class(title, graph)
    } else {
        render_flowchart(title, graph)
    }
}

// ---------------------------------------------------------------------------
// Shared constants / helpers
// ---------------------------------------------------------------------------

const FONT_SIZE: f64 = 13.0;
const FONT_W: f64 = 7.8; // approximate mono char width at FONT_SIZE
const PAD: f64 = 10.0;
const TITLE_H: f64 = 30.0;

fn text_width(s: &str) -> f64 {
    s.len() as f64 * FONT_W
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn with_title(title: &str, w: f64, h: f64, body: &str) -> String {
    let header = svg_header(w, h + TITLE_H);
    let esc = xml_escape(title);
    let cx = w / 2.0;
    let ty = TITLE_H / 2.0 + FONT_SIZE / 2.0 - 1.0;
    let th = TITLE_H;
    let rpad = w - 20.0;
    format!(
        "{header}  <title>{esc}</title>\n  \
         <text x=\"{cx:.1}\" y=\"{ty:.1}\" text-anchor=\"middle\" \
         style=\"font-weight:bold;font-size:15px;fill:#333\">{esc}</text>\n  \
         <line x1=\"20\" y1=\"{th:.1}\" x2=\"{rpad:.1}\" y2=\"{th:.1}\" \
         stroke=\"#ddd\" stroke-width=\"1\"/>\n  \
         <g transform=\"translate(0,{th:.1})\">\n{body}  </g>\n</svg>"
    )
}

fn svg_header(w: f64, h: f64) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
  <defs>
    <style>
      text {{ font-family: monospace; font-size: {fs}px; fill: #222; }}
      .part-box  {{ fill: #4a90d9; stroke: #2c5f8a; stroke-width:1.5; rx:4; }}
      .part-lab  {{ fill: white; font-weight: bold; }}
      .life-line {{ stroke: #999; stroke-width:1; stroke-dasharray:5,4; }}
      .arrow      {{ stroke: #333; stroke-width:1.5; fill: none; }}
      .arrow-head {{ fill: #333; }}
      .ret-arrow  {{ stroke: #666; stroke-width:1.5; fill: none; stroke-dasharray:6,3; }}
      .activation {{ fill: #c5e3ff; stroke: #4a90d9; stroke-width:1; }}
      .msg-box    {{ fill: #fffde7; stroke: #e0c800; stroke-width:1; rx:3; }}
      .msg-text   {{ fill: #333; font-size: 11px; }}
      .ret-text   {{ fill: #666; font-size: 11px; font-style: italic; }}
      .node-box   {{ fill: #e8f4fd; stroke: #4a90d9; stroke-width:1.5; rx:6; }}
      .class-box  {{ fill: #f0f7ff; stroke: #4a90d9; stroke-width:1.5; rx:4; }}
      .class-hdr  {{ fill: #4a90d9; rx:4; }}
      .class-htxt {{ fill: white; font-weight: bold; }}
      .edge-line  {{ stroke: #555; stroke-width:1.5; fill:none; }}
      .edge-lab   {{ fill: #555; font-size: 11px; }}
    </style>
    <marker id="arr" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
      <polygon points="0 0, 8 3, 0 6" class="arrow-head"/>
    </marker>
    <marker id="ret-arr" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
      <polyline points="0 0, 8 3, 0 6" fill="none" stroke="gray" stroke-width="1.2"/>
    </marker>
  </defs>
"#,
        w = w,
        h = h,
        fs = FONT_SIZE
    )
}

// ---------------------------------------------------------------------------
// Sequence diagram
// ---------------------------------------------------------------------------

fn render_sequence(title: &str, graph: &CallGraph) -> String {
    // ---- participants (BFS / insertion order) ----
    let mut participants: Vec<&Node> = Vec::new();
    let mut seen_classes: Vec<&str> = Vec::new();
    for node in &graph.nodes {
        if !seen_classes.contains(&node.class.as_str()) {
            seen_classes.push(&node.class);
            participants.push(node);
        }
    }
    if participants.is_empty() {
        return "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"60\"><text x=\"10\" y=\"30\">No nodes</text></svg>".to_string();
    }

    // ---- column geometry ----
    let part_h    = 44.0;
    let top_margin = 10.0;
    let col_pad   = 40.0; // extra horizontal padding per column
    let call_h    = 55.0; // vertical spacing per event row
    let act_w     = 12.0; // activation-bar width

    let col_widths: Vec<f64> = participants
        .iter()
        .map(|n| f64::max(140.0, text_width(&n.class) + col_pad))
        .collect();

    let mut col_x: Vec<f64> = Vec::with_capacity(participants.len());
    let mut x_acc = 20.0_f64;
    for w in &col_widths {
        col_x.push(x_acc + w / 2.0);
        x_acc += w;
    }
    let total_w = x_acc + 20.0;

    // map class name -> column index
    let col_idx: HashMap<&str, usize> = participants
        .iter()
        .enumerate()
        .map(|(i, n)| (n.class.as_str(), i))
        .collect();

    // ---- DFS events ----
    let events = build_events(graph);
    let n_events = events.len();
    let life_top = top_margin + part_h;
    let total_h  = life_top + (n_events as f64 + 1.5) * call_h + 20.0;

    // Assign a Y to every event
    let event_ys: Vec<f64> = (0..n_events)
        .map(|i| life_top + (i as f64 + 1.0) * call_h)
        .collect();

    // ---- First pass: compute activation bars ----
    // act_starts[class] = stack of (y_start) for open activations
    let mut act_starts: HashMap<String, Vec<f64>> = HashMap::new();
    // collected bars: (class, y_start, y_end)
    let mut act_bars: Vec<(String, f64, f64)> = Vec::new();

    for (ev, &y) in events.iter().zip(event_ys.iter()) {
        match ev {
            SeqEvent::Call   { to, .. }   => { act_starts.entry(to.clone()).or_default().push(y); }
            SeqEvent::Return { from, .. } => {
                if let Some(stack) = act_starts.get_mut(from.as_str()) {
                    if let Some(y_start) = stack.pop() {
                        act_bars.push((from.clone(), y_start, y));
                    }
                }
            }
        }
    }
    // Flush any activations never explicitly returned (e.g., entry point)
    for (class, stack) in &act_starts {
        for &y_start in stack {
            act_bars.push((class.clone(), y_start, total_h - 20.0));
        }
    }

    let mut body = String::new();

    // ---- Participant boxes ----
    for (i, node) in participants.iter().enumerate() {
        let cx = col_x[i];
        let bw = col_widths[i] - 4.0;
        let bx = cx - bw / 2.0;
        body.push_str(&format!(
            "  <rect x=\"{bx:.1}\" y=\"{by:.1}\" width=\"{bw:.1}\" height=\"{ph:.1}\" rx=\"4\" class=\"part-box\"/>\n\
               <text x=\"{cx:.1}\" y=\"{ty:.1}\" text-anchor=\"middle\" class=\"part-lab\">{label}</text>\n",
            bx = bx, by = top_margin, bw = bw, ph = part_h,
            cx = cx, ty = top_margin + part_h / 2.0 + FONT_SIZE / 2.0 - 2.0,
            label = xml_escape(&node.class)
        ));
    }

    // ---- Lifelines ----
    let life_bot = total_h - 20.0;
    for (i, _) in participants.iter().enumerate() {
        let cx = col_x[i];
        body.push_str(&format!(
            "  <line x1=\"{cx:.1}\" y1=\"{lt:.1}\" x2=\"{cx:.1}\" y2=\"{lb:.1}\" class=\"life-line\"/>\n",
            cx = cx, lt = life_top, lb = life_bot
        ));
    }

    // ---- Activation bars (drawn before arrows so arrows appear on top) ----
    for (class, y_start, y_end) in &act_bars {
        if let Some(&ci) = col_idx.get(class.as_str()) {
            let cx = col_x[ci];
            let bar_h = (y_end - y_start).max(4.0);
            body.push_str(&format!(
                "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"2\" class=\"activation\"/>\n",
                x = cx - act_w / 2.0, y = y_start, w = act_w, h = bar_h
            ));
        }
    }

    // ---- Arrows ----
    for (ev, &y) in events.iter().zip(event_ys.iter()) {
        match ev {
            SeqEvent::Call { from, to, label } => {
                let fi = col_idx.get(from.as_str()).copied().unwrap_or(0);
                let ti = col_idx.get(to.as_str()).copied().unwrap_or(0);
                let x1 = col_x[fi];
                let x2 = col_x[ti];
                let call_label = format!("{}()", label);
                let lw = text_width(&call_label) + 10.0;
                let lh = 20.0;

                if fi == ti {
                    // Self-call loop
                    let loop_w = 55.0;
                    let loop_h = 36.0;
                    let rx = x1 + act_w / 2.0;
                    body.push_str(&format!(
                        "  <path d=\"M {x1:.1},{y:.1} H {r:.1} V {yb:.1} H {x2:.1}\" \
                            class=\"arrow\" marker-end=\"url(#arr)\"/>\n\
                           <rect x=\"{lx:.1}\" y=\"{ly:.1}\" width=\"{lw:.1}\" height=\"{lh:.1}\" rx=\"3\" class=\"msg-box\"/>\n\
                           <text x=\"{tx:.1}\" y=\"{ty:.1}\" text-anchor=\"middle\" class=\"msg-text\">{lab}</text>\n",
                        x1 = x1, y = y, r = rx + loop_w, yb = y + loop_h, x2 = x1,
                        lx = rx + loop_w / 2.0 - lw / 2.0, ly = y - lh - 2.0,
                        lw = lw, lh = lh,
                        tx = rx + loop_w / 2.0, ty = y - lh / 2.0 - 2.0 + FONT_SIZE / 2.0,
                        lab = xml_escape(&call_label)
                    ));
                } else {
                    let mid_x = (x1 + x2) / 2.0;
                    body.push_str(&format!(
                        "  <line x1=\"{x1:.1}\" y1=\"{y:.1}\" x2=\"{x2:.1}\" y2=\"{y:.1}\" \
                            class=\"arrow\" marker-end=\"url(#arr)\"/>\n\
                           <rect x=\"{lx:.1}\" y=\"{ly:.1}\" width=\"{lw:.1}\" height=\"{lh:.1}\" rx=\"3\" class=\"msg-box\"/>\n\
                           <text x=\"{tx:.1}\" y=\"{ty:.1}\" text-anchor=\"middle\" class=\"msg-text\">{lab}</text>\n",
                        x1 = x1, y = y, x2 = x2,
                        lx = mid_x - lw / 2.0, ly = y - lh - 2.0,
                        lw = lw, lh = lh,
                        tx = mid_x, ty = y - lh / 2.0 - 2.0 + FONT_SIZE / 2.0,
                        lab = xml_escape(&call_label)
                    ));
                }
            }

            SeqEvent::Return { from, to, label } => {
                let fi = col_idx.get(from.as_str()).copied().unwrap_or(0);
                let ti = col_idx.get(to.as_str()).copied().unwrap_or(0);
                let x1 = col_x[fi];
                let x2 = col_x[ti];
                let ret_label = label.as_str();

                if fi == ti {
                    // Self-return (same column) — skip, already closed by the self-call loop
                } else {
                    let mid_x = (x1 + x2) / 2.0;
                    body.push_str(&format!(
                        "  <line x1=\"{x1:.1}\" y1=\"{y:.1}\" x2=\"{x2:.1}\" y2=\"{y:.1}\" \
                            class=\"ret-arrow\" marker-end=\"url(#ret-arr)\"/>\n\
                           <text x=\"{tx:.1}\" y=\"{ty:.1}\" text-anchor=\"middle\" class=\"ret-text\">{lab}</text>\n",
                        x1 = x1, y = y, x2 = x2,
                        tx = mid_x, ty = y - 4.0,
                        lab = xml_escape(ret_label)
                    ));
                }
            }
        }
    }

    with_title(title, total_w, total_h, &body)
}

// ---------------------------------------------------------------------------
// Flowchart (Go / Python)
// ---------------------------------------------------------------------------

fn render_flowchart(title: &str, graph: &CallGraph) -> String {
    if graph.nodes.is_empty() {
        return "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"60\"><text x=\"10\" y=\"30\">No nodes</text></svg>".to_string();
    }

    // BFS level assignment
    let (levels, max_level) = bfs_levels(graph);

    // Group nodes by level
    let mut by_level: Vec<Vec<usize>> = vec![vec![]; max_level + 1];
    for (i, &lvl) in levels.iter().enumerate() {
        by_level[lvl].push(i);
    }

    let node_w = 180.0;
    let node_h = 40.0;
    let h_gap = 30.0;
    let v_gap = 70.0;
    let margin = 30.0;

    // Canvas size
    let max_in_row = by_level.iter().map(|l| l.len()).max().unwrap_or(1) as f64;
    let total_w = max_in_row * (node_w + h_gap) + 2.0 * margin;
    let total_h = (max_level as f64 + 1.0) * (node_h + v_gap) + 2.0 * margin;

    // Node positions
    let mut pos: Vec<(f64, f64)> = vec![(0.0, 0.0); graph.nodes.len()];
    for (level, indices) in by_level.iter().enumerate() {
        let count = indices.len() as f64;
        let row_w = count * (node_w + h_gap) - h_gap;
        let start_x = (total_w - row_w) / 2.0;
        let y = margin + level as f64 * (node_h + v_gap);
        for (j, &idx) in indices.iter().enumerate() {
            pos[idx] = (start_x + j as f64 * (node_w + h_gap), y);
        }
    }

    let mut body = String::new();

    // Edges (drawn first, behind nodes)
    let id_map: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    for edge in &graph.edges {
        let fi = match id_map.get(edge.from.as_str()) { Some(&i) => i, None => continue };
        let ti = match id_map.get(edge.to.as_str()) { Some(&i) => i, None => continue };

        let (fx, fy) = pos[fi];
        let (tx, ty) = pos[ti];
        let x1 = fx + node_w / 2.0;
        let y1 = fy + node_h;
        let x2 = tx + node_w / 2.0;
        let y2 = ty;

        let mx = (x1 + x2) / 2.0;
        let my = (y1 + y2) / 2.0;
        let label_w = text_width(&edge.label) + 8.0;

        body.push_str(&format!(
            r#"  <line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" class="edge-line" marker-end="url(#arr)"/>
  <text x="{mx:.1}" y="{my:.1}" text-anchor="middle" class="edge-lab">{label}</text>
"#,
            x1 = x1, y1 = y1, x2 = x2, y2 = y2,
            mx = mx, my = my - 3.0,
            label = xml_escape(&edge.label)
        ));
        let _ = label_w;
    }

    // Nodes
    for (i, node) in graph.nodes.iter().enumerate() {
        let (x, y) = pos[i];
        let label = format!("{}.{}", node.class, node.method);
        let cx = x + node_w / 2.0;
        body.push_str(&format!(
            r#"  <rect x="{x:.1}" y="{y:.1}" width="{nw:.1}" height="{nh:.1}" rx="6" class="node-box"/>
  <text x="{cx:.1}" y="{ty:.1}" text-anchor="middle">{label}</text>
"#,
            x = x, y = y, nw = node_w, nh = node_h,
            cx = cx,
            ty = y + node_h / 2.0 + FONT_SIZE / 2.0 - 2.0,
            label = xml_escape(&label)
        ));
    }

    with_title(title, total_w, total_h, &body)
}

// ---------------------------------------------------------------------------
// Class diagram (Java)
// ---------------------------------------------------------------------------

fn render_class(title: &str, graph: &CallGraph) -> String {
    use std::collections::HashSet;

    // ── 1. Group methods by class ─────────────────────────────────────────────
    let mut classes: Vec<(String, Vec<String>)> = Vec::new();
    let mut class_idx: HashMap<String, usize> = HashMap::new();
    for node in &graph.nodes {
        if let Some(&i) = class_idx.get(&node.class) {
            if !classes[i].1.contains(&node.method) { classes[i].1.push(node.method.clone()); }
        } else {
            class_idx.insert(node.class.clone(), classes.len());
            classes.push((node.class.clone(), vec![node.method.clone()]));
        }
    }
    if classes.is_empty() {
        return "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"60\"><text x=\"10\" y=\"30\">No nodes</text></svg>".to_string();
    }
    let nc = classes.len();

    // ── 2. Deduped class-level edges ─────────────────────────────────────────
    let node_by_id: HashMap<&str, &Node> =
        graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut edge_set: HashSet<(usize, usize)> = HashSet::new();
    let mut class_edges: Vec<(usize, usize)> = Vec::new();
    for edge in &graph.edges {
        let fc = node_by_id.get(edge.from.as_str()).map(|n| n.class.as_str()).unwrap_or("?");
        let tc = node_by_id.get(edge.to.as_str()).map(|n| n.class.as_str()).unwrap_or("?");
        if fc == tc { continue; }
        if let (Some(&fi), Some(&ti)) = (class_idx.get(fc), class_idx.get(tc)) {
            if edge_set.insert((fi, ti)) { class_edges.push((fi, ti)); }
        }
    }

    // ── 3. BFS level per class ────────────────────────────────────────────────
    let (node_levels, _) = bfs_levels(graph);
    let mut class_level = vec![usize::MAX; nc];
    for (i, node) in graph.nodes.iter().enumerate() {
        if let Some(&ci) = class_idx.get(&node.class) {
            class_level[ci] = class_level[ci].min(node_levels[i]);
        }
    }
    for l in &mut class_level { if *l == usize::MAX { *l = 0; } }
    let max_level = *class_level.iter().max().unwrap_or(&0);
    let class_row = class_level.clone();

    // ── 4. Row grouping ───────────────────────────────────────────────────────
    let mut rows: Vec<Vec<usize>> = vec![vec![]; max_level + 1];
    for ci in 0..nc { rows[class_row[ci]].push(ci); }

    // ── 5. Layout constants ───────────────────────────────────────────────────
    let hdr_h      = 34.0_f64;
    let row_h      = 22.0_f64;
    let h_gap      = 60.0_f64;
    let base_v_gap = 90.0_f64;   // minimum gap; may be expanded per zone
    let margin     = 40.0_f64;
    let min_bw     = 150.0_f64;
    let text_pad   = 20.0_f64;

    // ── 6. Per-class box dimensions ───────────────────────────────────────────
    let box_widths: Vec<f64> = classes.iter().map(|(name, methods)| {
        let hw = text_width(name) + text_pad * 2.0;
        let mw = methods.iter()
            .map(|m| text_width(&format!("+ {}()", m)) + text_pad)
            .fold(0.0_f64, f64::max);
        f64::max(min_bw, f64::max(hw, mw))
    }).collect();
    let box_heights: Vec<f64> = classes.iter()
        .map(|(_, methods)| hdr_h + methods.len() as f64 * row_h + PAD)
        .collect();

    // ── 7. Canvas width (max row width; stable across reorderings) ────────────
    let row_w_fn = |row: &Vec<usize>| -> f64 {
        row.iter().map(|&ci| box_widths[ci]).sum::<f64>()
            + row.len().saturating_sub(1) as f64 * h_gap
    };
    let canvas_w = rows.iter().map(|r| row_w_fn(r)).fold(0.0_f64, f64::max) + 2.0 * margin;

    // ── 8. Barycenter reordering (2 top-down passes) ─────────────────────────
    let mut pred: Vec<Vec<usize>> = vec![vec![]; nc];
    for &(fi, ti) in &class_edges { pred[ti].push(fi); }

    for _pass in 0..2 {
        for r in 1..rows.len() {
            let prev_rw = row_w_fn(&rows[r - 1]);
            let mut px = (canvas_w - prev_rw) / 2.0;
            let mut prev_cx: HashMap<usize, f64> = HashMap::new();
            for &ci in &rows[r - 1] {
                prev_cx.insert(ci, px + box_widths[ci] / 2.0);
                px += box_widths[ci] + h_gap;
            }
            rows[r].sort_by(|&a, &b| {
                let bc = |ci: usize| -> f64 {
                    let ps: Vec<f64> = pred[ci].iter()
                        .filter(|&&pi| class_row[pi] + 1 == r)
                        .filter_map(|&pi| prev_cx.get(&pi))
                        .copied().collect();
                    if ps.is_empty() { f64::MAX } else { ps.iter().sum::<f64>() / ps.len() as f64 }
                };
                bc(a).partial_cmp(&bc(b)).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    // ── 9. Column index per class (after barycenter) ──────────────────────────
    let mut class_col = vec![0usize; nc];
    for row in &rows {
        for (k, &ci) in row.iter().enumerate() { class_col[ci] = k; }
    }

    // ── 10. Lane assignment per gap zone ─────────────────────────────────────
    // For each inter-row gap (indexed by the row above it = source row fr),
    // group all forward edges that route their horizontal knee through that gap.
    // Sort by (exit_col, target_row, entry_col) for minimal crossings.
    let n_rows = rows.len();
    let mut gap_edges: Vec<Vec<(usize, usize)>> = vec![vec![]; n_rows];
    for &(fi, ti) in &class_edges {
        let fr = class_row[fi];
        let tr = class_row[ti];
        if fr < tr { gap_edges[fr].push((fi, ti)); }
    }
    for fr in 0..n_rows {
        gap_edges[fr].sort_by_key(|&(fi, ti)| (class_col[fi], class_row[ti], class_col[ti]));
    }
    let mut edge_lane: HashMap<(usize, usize), usize> = HashMap::new();
    for (_fr, edges) in gap_edges.iter().enumerate() {
        for (k, &(fi, ti)) in edges.iter().enumerate() {
            edge_lane.insert((fi, ti), k);
        }
    }

    // ── 11. Per-gap vertical size (expand if many lanes) ─────────────────────
    let min_lane_h  = 22.0_f64;  // minimum vertical distance between lane lines
    let gap_margin  = 16.0_f64;  // top + bottom margin within zone
    let per_gap_v: Vec<f64> = (0..n_rows).map(|fr| {
        let n = gap_edges[fr].len();
        let needed = n as f64 * min_lane_h + gap_margin;
        f64::max(base_v_gap, needed)
    }).collect();

    // ── 12. Final box positions ───────────────────────────────────────────────
    let row_max_h: Vec<f64> = rows.iter()
        .map(|r| r.iter().map(|&ci| box_heights[ci]).fold(0.0_f64, f64::max))
        .collect();

    let mut bx_pos = vec![0.0_f64; nc];
    let mut by_pos = vec![0.0_f64; nc];
    let mut row_bot_y = vec![0.0_f64; n_rows];

    let mut y_acc = margin;
    for (r, row) in rows.iter().enumerate() {
        let rw = row_w_fn(row);
        let mut x_acc = (canvas_w - rw) / 2.0;
        for &ci in row {
            bx_pos[ci] = x_acc;
            by_pos[ci] = y_acc;
            x_acc += box_widths[ci] + h_gap;
        }
        row_bot_y[r] = y_acc + row_max_h[r];
        // Add gap below this row (not after the last row)
        y_acc += row_max_h[r] + if r + 1 < n_rows { per_gap_v[r] } else { 0.0 };
    }
    let canvas_h = y_acc + margin;

    // ── 13. Fan-out x for exits / entries ────────────────────────────────────
    let mut fwd_exits:   HashMap<usize, Vec<usize>> = HashMap::new();
    let mut fwd_entries: HashMap<usize, Vec<usize>> = HashMap::new();
    for &(fi, ti) in &class_edges {
        if class_row[fi] < class_row[ti] {
            fwd_exits.entry(fi).or_default().push(ti);
            fwd_entries.entry(ti).or_default().push(fi);
        }
    }
    for v in fwd_exits.values_mut()   { v.sort_by(|&a, &b| bx_pos[a].partial_cmp(&bx_pos[b]).unwrap()); }
    for v in fwd_entries.values_mut() { v.sort_by(|&a, &b| bx_pos[a].partial_cmp(&bx_pos[b]).unwrap()); }

    let fan_x = |ci: usize, group: &[usize], k: usize| -> f64 {
        let n = group.len();
        if n <= 1 { return bx_pos[ci] + box_widths[ci] / 2.0; }
        let usable  = box_widths[ci] - 2.0 * text_pad;
        let spacing = (usable / n as f64).min(30.0);
        let total   = spacing * (n as f64 - 1.0);
        bx_pos[ci] + box_widths[ci] / 2.0 - total / 2.0 + k as f64 * spacing
    };

    let mut exit_x:  HashMap<(usize, usize), f64> = HashMap::new();
    let mut entry_x: HashMap<(usize, usize), f64> = HashMap::new();
    for (&fi, targets) in &fwd_exits {
        for (k, &ti) in targets.iter().enumerate() {
            exit_x.insert((fi, ti), fan_x(fi, targets, k));
        }
    }
    for (&ti, sources) in &fwd_entries {
        for (k, &fi) in sources.iter().enumerate() {
            entry_x.insert((fi, ti), fan_x(ti, sources, k));
        }
    }

    // ── 14. Draw edges FIRST ─────────────────────────────────────────────────
    let mut body = String::new();

    for &(fi, ti) in &class_edges {
        let fr = class_row[fi];
        let tr = class_row[ti];

        let path_d = if fr == tr {
            // Same row: side-to-side horizontal
            let y1 = by_pos[fi] + box_heights[fi] / 2.0;
            let y2 = by_pos[ti] + box_heights[ti] / 2.0;
            if bx_pos[fi] < bx_pos[ti] {
                format!("M {:.1},{:.1} L {:.1},{:.1}",
                    bx_pos[fi] + box_widths[fi], y1, bx_pos[ti], y2)
            } else {
                format!("M {:.1},{:.1} L {:.1},{:.1}",
                    bx_pos[fi], y1, bx_pos[ti] + box_widths[ti], y2)
            }
        } else if fr < tr {
            // Forward edge: lane-staggered horizontal knee in gap below row fr
            let ex = exit_x.get(&(fi, ti)).copied()
                .unwrap_or(bx_pos[fi] + box_widths[fi] / 2.0);
            let nx = entry_x.get(&(fi, ti)).copied()
                .unwrap_or(bx_pos[ti] + box_widths[ti] / 2.0);
            let y1  = by_pos[fi] + box_heights[fi];
            let y2  = by_pos[ti];
            // Lane Y
            let n_lanes = gap_edges[fr].len().max(1);
            let lane    = *edge_lane.get(&(fi, ti)).unwrap_or(&0);
            let gap_h   = per_gap_v[fr];
            let spacing = gap_h / (n_lanes as f64 + 1.0);
            let gy      = row_bot_y[fr] + spacing * (lane as f64 + 1.0);

            if (ex - nx).abs() < 1.0 {
                // Perfectly aligned: vertical straight line through lane Y
                format!("M {:.1},{:.1} L {:.1},{:.1}", ex, y1, nx, y2)
            } else {
                format!("M {:.1},{:.1} L {:.1},{:.1} L {:.1},{:.1} L {:.1},{:.1}",
                    ex, y1, ex, gy, nx, gy, nx, y2)
            }
        } else {
            // Back-edge: route along right margin
            let rx = canvas_w - margin * 0.4;
            let y1 = by_pos[fi] + box_heights[fi] / 2.0;
            let y2 = by_pos[ti] + box_heights[ti] / 2.0;
            let x1 = bx_pos[fi] + box_widths[fi];
            let x2 = bx_pos[ti] + box_widths[ti];
            format!("M {:.1},{:.1} L {:.1},{:.1} L {:.1},{:.1} L {:.1},{:.1}",
                x1, y1, rx, y1, rx, y2, x2, y2)
        };

        body.push_str(&format!(
            "  <path d=\"{p}\" class=\"edge-line\" fill=\"none\" marker-end=\"url(#arr)\"/>\n",
            p = path_d
        ));
    }

    // ── 15. Draw class boxes ON TOP ───────────────────────────────────────────
    for (i, (name, methods)) in classes.iter().enumerate() {
        let x   = bx_pos[i];
        let y   = by_pos[i];
        let bw  = box_widths[i];
        let bh  = box_heights[i];
        let mid = x + bw / 2.0;
        let cph = format!("cph{i}");
        let cpm = format!("cpm{i}");
        body.push_str(&format!(
            "  <clipPath id=\"{id}\"><rect x=\"{cx:.1}\" y=\"{cy:.1}\" width=\"{cw:.1}\" height=\"{ch:.1}\"/></clipPath>\n",
            id=cph, cx=x+2.0, cy=y, cw=bw-4.0, ch=hdr_h
        ));
        body.push_str(&format!(
            "  <clipPath id=\"{id}\"><rect x=\"{cx:.1}\" y=\"{cy:.1}\" width=\"{cw:.1}\" height=\"{ch:.1}\"/></clipPath>\n",
            id=cpm, cx=x+2.0, cy=y+hdr_h, cw=bw-4.0, ch=bh-hdr_h
        ));
        body.push_str(&format!(
            "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bw:.1}\" height=\"{bh:.1}\" rx=\"4\" class=\"class-box\"/>\n",
            x=x, y=y, bw=bw, bh=bh
        ));
        body.push_str(&format!(
            "  <rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bw:.1}\" height=\"{hh:.1}\" rx=\"4\" class=\"class-hdr\"/>\n",
            x=x, y=y, bw=bw, hh=hdr_h
        ));
        body.push_str(&format!(
            "  <text x=\"{mid:.1}\" y=\"{ty:.1}\" text-anchor=\"middle\" class=\"class-htxt\" clip-path=\"url(#{clip})\">{name}</text>\n",
            mid=mid, ty=y+hdr_h/2.0+FONT_SIZE/2.0-2.0, clip=cph, name=xml_escape(name)
        ));
        for (mi, method) in methods.iter().enumerate() {
            let my = y + hdr_h + (mi as f64 + 0.5) * row_h + PAD / 2.0;
            body.push_str(&format!(
                "  <text x=\"{tx:.1}\" y=\"{my:.1}\" class=\"edge-lab\" clip-path=\"url(#{clip})\">+ {m}()</text>\n",
                tx=x+PAD, my=my, clip=cpm, m=xml_escape(method)
            ));
        }
    }

    with_title(title, canvas_w, canvas_h, &body)
}

// ---------------------------------------------------------------------------
// BFS level assignment helper
// ---------------------------------------------------------------------------

fn bfs_levels(graph: &CallGraph) -> (Vec<usize>, usize) {
    use std::collections::VecDeque;
    let n = graph.nodes.len();
    let id_map: HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, nd)| (nd.id.as_str(), i))
        .collect();

    let mut levels = vec![usize::MAX; n];
    let mut queue = VecDeque::new();

    // Entry point = first node
    if !graph.nodes.is_empty() {
        let entry_idx = id_map.get(graph.entry.as_str()).copied().unwrap_or(0);
        levels[entry_idx] = 0;
        queue.push_back(entry_idx);
    }

    while let Some(cur) = queue.pop_front() {
        let cur_id = &graph.nodes[cur].id;
        for edge in &graph.edges {
            if &edge.from == cur_id {
                if let Some(&ti) = id_map.get(edge.to.as_str()) {
                    if levels[ti] == usize::MAX {
                        levels[ti] = levels[cur] + 1;
                        queue.push_back(ti);
                    }
                }
            }
        }
    }

    // Assign unreachable nodes to level after max
    let max = levels.iter().filter(|&&l| l != usize::MAX).max().copied().unwrap_or(0);
    for l in &mut levels {
        if *l == usize::MAX {
            *l = max + 1;
        }
    }
    let max = levels.iter().copied().max().unwrap_or(0);
    (levels, max)
}
