/// Package-level dependency diagram.
///
/// # Java
///
/// Scans **all** `.java` source files under the project root.  For each file:
///   1. Reads the `package` declaration → node identifier.
///   2. Reads every `import` statement and keeps only imports whose package
///      belongs to the same project (shares the common root prefix).
///      Wildcard imports (`import pkg.*;`) are included.
///   3. Detects Spring DI fields: lines preceded by `@Autowired`, `@Inject`,
///      or `@Resource` are marked `via_spring = true` in the edge.
///
/// **No transitive closure** — only *direct* imports produce edges.
/// Shared infrastructure packages (`util`, `config`, `common`, …) are
/// classified as the `Infrastructure` layer; edges into them never trigger
/// a violation, so they are safe to depend on from any layer.
///
/// Violation rule (`is_layer_violation`):
///   - Upward dependency (e.g. Service → Controller).
///   - Layer skip (e.g. Controller → Repository, skipping Service).
///   - Infrastructure / Other layers are exempt in both directions.
///
/// # Python / Go
///
/// Uses the call-graph edges to derive module-level dependencies (same
/// granularity as the component diagram).
use std::collections::{HashMap, HashSet};
use std::fs;
use regex::Regex;
use walkdir::WalkDir;
use crate::parser::CallGraph;
use super::component::{infer_layer, is_layer_violation, Layer};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PackageNode {
    /// Full package path / module id (e.g. `com.example.controller`).
    pub id: String,
    /// Short display label (last 1–2 segments).
    pub label: String,
    pub layer: Layer,
}

#[derive(Debug, Clone)]
pub struct DepEdge {
    pub from: String,
    pub to: String,
    /// Confirmed Spring-DI dependency (`@Autowired`, `@Inject`, `@Resource`).
    pub via_spring: bool,
    pub is_violation: bool,
}

#[derive(Debug, Default, Clone)]
pub struct DepMetrics {
    /// Incoming edge count per package id.
    pub fan_in: HashMap<String, usize>,
    /// Outgoing edge count per package id.
    pub fan_out: HashMap<String, usize>,
    /// Packages that participate in at least one cycle.
    pub in_cycle: HashSet<String>,
    /// Each cycle as an ordered list of package ids (SCCs with size > 1).
    pub cycles: Vec<Vec<String>>,
    /// Longest dependency-path depth from any source (0 = root / no incoming deps).
    pub depth: HashMap<String, usize>,
}

#[derive(Debug, Default)]
pub struct DependencyDiagram {
    pub packages: Vec<PackageNode>,
    pub edges: Vec<DepEdge>,
    pub metrics: DepMetrics,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a [`DependencyDiagram`] from a call graph.
/// For Java: scans the full source tree under `root`.
/// For Python/Go: falls back to call-graph–derived module edges.
pub fn build_dependency_diagram(lang: &str, root: &str, graph: &CallGraph) -> DependencyDiagram {
    match lang {
        "java" => build_java(root),
        _      => build_generic(lang, graph),
    }
}

/// Serialise a [`DependencyDiagram`] to Graphviz DOT format.
/// Node labels show fan-in (fi), fan-out (fo) and longest-path depth (d).
/// Cycle nodes are tinted orange; top-5 coupled nodes have a thicker border;
/// cycle-forming edges are drawn in orange-red.
pub fn dependency_dot(diagram: &DependencyDiagram) -> String {
    let m = &diagram.metrics;

    // Top-5 most coupled packages (fi + fo), highlighted with thick border
    let mut coupling: Vec<(&str, usize)> = diagram.packages.iter()
        .map(|p| {
            let fi = m.fan_in.get(&p.id).copied().unwrap_or(0);
            let fo = m.fan_out.get(&p.id).copied().unwrap_or(0);
            (p.id.as_str(), fi + fo)
        })
        .collect();
    coupling.sort_by(|a, b| b.1.cmp(&a.1));
    let top_coupled: HashSet<&str> = coupling.iter()
        .take(5)
        .filter(|(_, c)| *c > 0)
        .map(|(id, _)| *id)
        .collect();

    // Map each package to its SCC index (for cycle-edge detection)
    let mut node_scc: HashMap<&str, usize> = HashMap::new();
    for (scc_idx, scc) in m.cycles.iter().enumerate() {
        for id in scc {
            node_scc.insert(id.as_str(), scc_idx);
        }
    }

    let mut out = String::new();
    out.push_str("digraph dependencies {\n");
    out.push_str("  graph [layout=fdp, bgcolor=\"white\", fontname=\"Helvetica\",\n");
    out.push_str("         nodesep=0.7, splines=curved, overlap=false];\n");
    out.push_str("  node  [shape=box, style=\"rounded,filled\", fontname=\"Helvetica\", fontsize=10];\n");
    out.push_str("  edge  [fontname=\"Helvetica\", fontsize=9];\n\n");

    // Group nodes by layer
    let mut by_layer: HashMap<&'static str, Vec<&PackageNode>> = HashMap::new();
    for pkg in &diagram.packages {
        by_layer.entry(pkg.layer.id()).or_default().push(pkg);
    }

    for layer in Layer::all_ordered() {
        let nodes = match by_layer.get(layer.id()) {
            Some(v) if !v.is_empty() => v,
            _ => continue,
        };
        out.push_str(&format!("  subgraph cluster_{} {{\n", layer.id()));
        out.push_str(&format!("    label=<<B>{}</B>>;\n", layer.label()));
        out.push_str(&format!("    style=filled; fillcolor=\"{}\";\n", layer.band_color()));
        out.push_str(&format!("    color=\"{}\"; penwidth=2;\n", layer.border_color()));
        out.push_str("    fontname=\"Helvetica\"; fontsize=11;\n");
        for pkg in nodes {
            let fi       = m.fan_in.get(&pkg.id).copied().unwrap_or(0);
            let fo       = m.fan_out.get(&pkg.id).copied().unwrap_or(0);
            let depth    = m.depth.get(&pkg.id).copied().unwrap_or(0);
            let in_cycle = m.in_cycle.contains(&pkg.id);
            let is_top   = top_coupled.contains(pkg.id.as_str());
            let cycle_mark = if in_cycle { " \u{21ba}" } else { "" }; // ↺
            let node_label = format!("{}{}\\nfi:{} fo:{}  d:{}",
                dot_esc(&pkg.label), cycle_mark, fi, fo, depth);
            let fill   = if in_cycle { "#ffe0b2" } else { layer.box_color() };
            let border = if in_cycle { "#e65100" } else { layer.border_color() };
            let pw     = if is_top { "penwidth=3.5, " } else { "" };
            let tip    = format!("fan-in:{} fan-out:{} depth:{}{}",
                fi, fo, depth, if in_cycle { " CYCLE" } else { "" });
            let id     = dep_dot_id(&pkg.id);
            out.push_str(&format!(
                "    {id} [{pw}label=\"{node_label}\", fillcolor=\"{fill}\", \
                 color=\"{border}\", tooltip=\"{tip}\"];\n"
            ));
        }
        out.push_str("  }\n\n");
    }

    for e in &diagram.edges {
        let is_cycle_edge = match (node_scc.get(e.from.as_str()), node_scc.get(e.to.as_str())) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let attribs = if is_cycle_edge {
            " [label=\"\u{21ba}\", color=\"#e65100\", fontcolor=\"#e65100\", \
             penwidth=2.5, style=solid]".to_string()
        } else if e.is_violation {
            " [label=\"\u{26a0} violation\", color=\"#cc2222\", fontcolor=\"#cc2222\", \
             penwidth=2.5, style=solid]".to_string()
        } else if e.via_spring {
            " [label=\"\u{00ab}inject\u{00bb}\", color=\"#2266aa\", fontcolor=\"#2266aa\", \
             style=dashed]".to_string()
        } else {
            " [style=dashed, color=\"#555555\"]".to_string()
        };
        out.push_str(&format!(
            "  {} -> {}{};\n",
            dep_dot_id(&e.from), dep_dot_id(&e.to), attribs
        ));
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Java implementation
// ---------------------------------------------------------------------------

fn build_java(root: &str) -> DependencyDiagram {
    let re_pkg  = Regex::new(r"^\s*package\s+([\w.]+)\s*;").unwrap();
    let re_imp  = Regex::new(r"^\s*import\s+(?:static\s+)?([\w.]+(?:\.\*)?)\s*;").unwrap();
    let re_cls  = Regex::new(r"(?:^|\s)(?:class|interface|enum)\s+(\w+)").unwrap();
    let re_spr  = Regex::new(r"@(?:Autowired|Inject|Resource)\b").unwrap();

    // ── Pass 1: collect all packages present in the project ──────────────────
    let mut all_pkgs: HashSet<String> = HashSet::new();
    // raw_files: (file_path, content)
    let mut raw_files: Vec<(String, String)> = Vec::new();

    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("java") { continue; }
        let Ok(content) = fs::read_to_string(entry.path()) else { continue };
        for line in content.lines() {
            if let Some(cap) = re_pkg.captures(line) {
                all_pkgs.insert(cap[1].to_string());
                break;
            }
        }
        raw_files.push((entry.path().to_string_lossy().into_owned(), content));
    }

    if all_pkgs.is_empty() {
        return DependencyDiagram::default();
    }

    let root_prefix = common_prefix(&all_pkgs);

    // ── Pass 2: per-file import analysis ─────────────────────────────────────
    // pkg_data: package → (first class seen, set of direct project deps, spring deps)
    let mut pkg_data: HashMap<String, (String, Vec<String>, HashSet<String>)> = HashMap::new();
    let mut pkg_order: Vec<String> = Vec::new();

    for (_path, content) in &raw_files {
        // Determine this file's package
        let Some(file_pkg) = content.lines()
            .find_map(|l| re_pkg.captures(l).map(|c| c[1].to_string()))
        else { continue };

        // Scope to project namespace
        if !root_prefix.is_empty() && !file_pkg.starts_with(&root_prefix) { continue; }

        // First class name (for layer inference)
        let class = content.lines()
            .find_map(|l| re_cls.captures(l).map(|c| c[1].to_string()))
            .unwrap_or_default();

        // Build type → package map from imports (for Spring detection)
        let mut type_to_pkg: HashMap<String, String> = HashMap::new();
        let mut raw_imports: Vec<String> = Vec::new();

        for line in content.lines() {
            let Some(cap) = re_imp.captures(line) else { continue };
            let full = cap[1].trim_matches(';').trim();
            let (imp_pkg, simple_name) = split_import(full);

            if imp_pkg.is_empty() { continue; }
            if !root_prefix.is_empty() && !imp_pkg.starts_with(&root_prefix) { continue; }
            if !all_pkgs.contains(&imp_pkg) { continue; }
            if imp_pkg == file_pkg { continue; }

            if simple_name != "*" && !simple_name.is_empty() {
                type_to_pkg.insert(simple_name, imp_pkg.clone());
            }
            if !raw_imports.contains(&imp_pkg) {
                raw_imports.push(imp_pkg);
            }
        }

        // Detect Spring-injected dependencies
        let mut spring_pkgs: HashSet<String> = HashSet::new();
        let mut inject_next = false;
        for line in content.lines() {
            let t = line.trim();
            if t.starts_with("//") || t.starts_with('*') { continue; }

            if re_spr.is_match(t) {
                inject_next = true;
                continue;
            }
            if inject_next {
                if t.starts_with('@') { continue; } // another annotation
                inject_next = false;
                if let Some(type_name) = first_type_token(t) {
                    let base = strip_generic(type_name);
                    if let Some(pkg) = type_to_pkg.get(base) {
                        spring_pkgs.insert(pkg.clone());
                    }
                }
            }
        }

        // Merge into pkg_data (multiple files can share a package)
        let entry = pkg_data.entry(file_pkg.clone()).or_insert_with(|| {
            pkg_order.push(file_pkg.clone());
            (class.clone(), Vec::new(), HashSet::new())
        });
        if entry.0.is_empty() { entry.0 = class; }
        for imp in raw_imports {
            if !entry.1.contains(&imp) { entry.1.push(imp); }
        }
        entry.2.extend(spring_pkgs);
    }

    // ── Build nodes ───────────────────────────────────────────────────────────
    pkg_order.dedup();
    let packages: Vec<PackageNode> = pkg_order.iter()
        .filter_map(|id| {
            let (class, _, _) = pkg_data.get(id)?;
            let layer = infer_layer(id, class);
            Some(PackageNode {
                id: id.clone(),
                label: pkg_label(id),
                layer,
            })
        })
        .collect();

    let pkg_set: HashSet<&str> = packages.iter().map(|p| p.id.as_str()).collect();
    let pkg_layer: HashMap<&str, Layer> = packages.iter().map(|p| (p.id.as_str(), p.layer)).collect();

    // ── Build edges ───────────────────────────────────────────────────────────
    // (from, to) → via_spring  (upgrade to true if any file in 'from' uses Spring injection)
    let mut edge_map: HashMap<(String, String), bool> = HashMap::new();
    for (from_pkg, (_, imports, spring_pkgs)) in &pkg_data {
        if !pkg_set.contains(from_pkg.as_str()) { continue; }
        for to_pkg in imports {
            if !pkg_set.contains(to_pkg.as_str()) { continue; }
            let via = spring_pkgs.contains(to_pkg);
            let entry = edge_map.entry((from_pkg.clone(), to_pkg.clone())).or_insert(false);
            if via { *entry = true; }
        }
    }

    let mut edges: Vec<DepEdge> = edge_map.into_iter().map(|((from, to), via_spring)| {
        let fl = pkg_layer.get(from.as_str()).copied().unwrap_or(Layer::Other);
        let tl = pkg_layer.get(to.as_str()).copied().unwrap_or(Layer::Other);
        DepEdge { from, to, via_spring, is_violation: is_layer_violation(fl, tl) }
    }).collect();
    edges.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));

    let metrics = compute_metrics(&packages, &edges);
    DependencyDiagram { packages, edges, metrics }
}

// ---------------------------------------------------------------------------
// Generic (Python / Go) — call-graph edges at module level
// ---------------------------------------------------------------------------

fn extract_module_generic(file: &str) -> String {
    let norm = file.replace('\\', "/");
    let dir  = match norm.rfind('/') {
        Some(i) => norm[..i].to_string(),
        None    => String::new(),
    };
    dir.split('/').filter(|s| !s.is_empty()).last()
        .unwrap_or("root")
        .to_string()
}

fn build_generic(lang: &str, graph: &CallGraph) -> DependencyDiagram {
    let _ = lang;
    let node_mods: Vec<String> = graph.nodes.iter()
        .map(|n| extract_module_generic(&n.file))
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut packages: Vec<PackageNode> = Vec::new();

    for (node, m) in graph.nodes.iter().zip(node_mods.iter()) {
        if seen.insert(m.clone()) {
            let layer = infer_layer(m, &node.class);
            packages.push(PackageNode { id: m.clone(), label: m.clone(), layer });
        }
    }

    let pkg_layer: HashMap<&str, Layer> =
        packages.iter().map(|p| (p.id.as_str(), p.layer)).collect();
    let id_to_mod: HashMap<&str, &str> = graph.nodes.iter().zip(node_mods.iter())
        .map(|(n, m)| (n.id.as_str(), m.as_str()))
        .collect();

    let mut edge_set: HashSet<(String, String)> = HashSet::new();
    for edge in &graph.edges {
        let fm = id_to_mod.get(edge.from.as_str()).copied().unwrap_or("");
        let tm = id_to_mod.get(edge.to.as_str()).copied().unwrap_or("");
        if !fm.is_empty() && !tm.is_empty() && fm != tm {
            edge_set.insert((fm.to_string(), tm.to_string()));
        }
    }

    let mut edges: Vec<DepEdge> = edge_set.into_iter().map(|(from, to)| {
        let fl = pkg_layer.get(from.as_str()).copied().unwrap_or(Layer::Other);
        let tl = pkg_layer.get(to.as_str()).copied().unwrap_or(Layer::Other);
        DepEdge { from, to, via_spring: false, is_violation: is_layer_violation(fl, tl) }
    }).collect();
    edges.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));

    let metrics = compute_metrics(&packages, &edges);
    DependencyDiagram { packages, edges, metrics }
}

// ---------------------------------------------------------------------------
// Java helpers
// ---------------------------------------------------------------------------

/// Split a fully-qualified import into (package, simple_name).
///
/// Examples:
/// - `com.example.service.UserService` → (`com.example.service`, `UserService`)
/// - `com.example.model.User.Builder` → (`com.example.model`, `User`)  ← first uppercase
/// - `com.example.*`                  → (`com.example`, `*`)
fn split_import(s: &str) -> (String, String) {
    if s.ends_with(".*") {
        return (s[..s.len() - 2].to_string(), "*".to_string());
    }
    let parts: Vec<&str> = s.split('.').collect();
    let first_upper = parts.iter().position(|p| {
        p.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
    });
    match first_upper {
        None    => (s.to_string(), String::new()),
        Some(0) => (String::new(), s.to_string()),
        Some(i) => (parts[..i].join("."), parts[i].to_string()),
    }
}

/// Find the common package-path prefix across all known packages.
/// E.g. {`com.example.ctrl`, `com.example.svc`} → `com.example`.
fn common_prefix(pkgs: &HashSet<String>) -> String {
    if pkgs.is_empty() { return String::new(); }
    let mut sorted: Vec<&str> = pkgs.iter().map(|s| s.as_str()).collect();
    sorted.sort_unstable();
    let first = sorted[0];
    let last  = sorted[sorted.len() - 1];
    let common: String = first.chars().zip(last.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a)
        .collect();
    // Trim to a package-segment boundary
    match common.rfind('.') {
        Some(pos) => common[..pos].to_string(),
        None      => common,
    }
}

/// Short display label: last two dot-separated segments if the package has
/// more than two, otherwise the full name.
fn pkg_label(pkg: &str) -> String {
    let parts: Vec<&str> = pkg.split('.').collect();
    if parts.len() <= 2 {
        pkg.to_string()
    } else {
        parts[parts.len() - 2..].join(".")
    }
}

/// Return the first non-modifier, non-annotation token in a Java field/param
/// declaration line (the type name).
fn first_type_token(line: &str) -> Option<&str> {
    const MODS: &[&str] = &[
        "private", "protected", "public", "final", "static",
        "transient", "volatile", "native", "synchronized",
    ];
    line.split_whitespace()
        .find(|w| !MODS.contains(w) && !w.starts_with('@'))
}

/// Strip generic type parameters: `List<User>` → `List`.
fn strip_generic(s: &str) -> &str {
    s.split('<').next().unwrap_or(s)
}

// ---------------------------------------------------------------------------
// DOT helpers
// ---------------------------------------------------------------------------

fn dep_dot_id(s: &str) -> String {
    let id: String = s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    format!("d_{}", id)
}

fn dot_esc(s: &str) -> String {
    s.replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// Metrics analysis
// ---------------------------------------------------------------------------

fn compute_metrics(packages: &[PackageNode], edges: &[DepEdge]) -> DepMetrics {
    // ── fan-in / fan-out ────────────────────────────────────────────────────
    let mut fan_in: HashMap<String, usize>  = packages.iter().map(|p| (p.id.clone(), 0)).collect();
    let mut fan_out: HashMap<String, usize> = packages.iter().map(|p| (p.id.clone(), 0)).collect();
    for e in edges {
        *fan_out.entry(e.from.clone()).or_insert(0) += 1;
        *fan_in.entry(e.to.clone()).or_insert(0)   += 1;
    }

    // ── cycle detection via Tarjan's SCC ────────────────────────────────────
    let ids: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
    let n = ids.len();
    let idx_map: HashMap<&str, usize> =
        ids.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for e in edges {
        if let (Some(&fi), Some(&ti)) =
            (idx_map.get(e.from.as_str()), idx_map.get(e.to.as_str()))
        {
            if fi != ti { adj[fi].push(ti); }
        }
    }

    let all_sccs = tarjan_sccs(n, &adj, &ids);
    let mut cycles: Vec<Vec<String>> = all_sccs.into_iter()
        .filter(|scc| scc.len() > 1)
        .collect();
    cycles.sort_by(|a, b| b.len().cmp(&a.len())); // largest cycles first

    let mut in_cycle: HashSet<String> = HashSet::new();
    for scc in &cycles {
        for id in scc { in_cycle.insert(id.clone()); }
    }

    // ── dependency depth ────────────────────────────────────────────────────
    let depth = compute_depth(n, &adj, &ids);

    DepMetrics { fan_in, fan_out, in_cycle, cycles, depth }
}

/// Tarjan's strongly-connected-components algorithm.
/// Returns all SCCs (each is a list of node indices mapped to their string ids).
fn tarjan_sccs(n: usize, adj: &[Vec<usize>], nodes: &[String]) -> Vec<Vec<String>> {
    struct Ctx {
        index:    Vec<usize>,
        lowlink:  Vec<usize>,
        on_stack: Vec<bool>,
        stack:    Vec<usize>,
        counter:  usize,
        sccs:     Vec<Vec<usize>>,
    }

    impl Ctx {
        fn connect(&mut self, v: usize, adj: &[Vec<usize>]) {
            self.index[v]    = self.counter;
            self.lowlink[v]  = self.counter;
            self.counter    += 1;
            self.stack.push(v);
            self.on_stack[v] = true;

            let neighbors = adj[v].clone(); // clone to avoid holding a ref while mutating self
            for w in neighbors {
                if self.index[w] == usize::MAX {
                    self.connect(w, adj);
                    let ll = self.lowlink[w];
                    self.lowlink[v] = self.lowlink[v].min(ll);
                } else if self.on_stack[w] {
                    let idx = self.index[w];
                    self.lowlink[v] = self.lowlink[v].min(idx);
                }
            }

            if self.lowlink[v] == self.index[v] {
                let mut scc = Vec::new();
                loop {
                    let w = self.stack.pop().unwrap();
                    self.on_stack[w] = false;
                    scc.push(w);
                    if w == v { break; }
                }
                self.sccs.push(scc);
            }
        }
    }

    let mut ctx = Ctx {
        index:    vec![usize::MAX; n],
        lowlink:  vec![0; n],
        on_stack: vec![false; n],
        stack:    Vec::new(),
        counter:  0,
        sccs:     Vec::new(),
    };

    for i in 0..n {
        if ctx.index[i] == usize::MAX {
            ctx.connect(i, adj);
        }
    }

    ctx.sccs.into_iter()
        .map(|scc| scc.iter().map(|&i| nodes[i].clone()).collect())
        .collect()
}

/// Longest path (depth) from any root (fan-in = 0) to each node.
///
/// Uses Kahn's topological-sort with longest-path relaxation.
/// Nodes inside cycles are never fully dequeued; they retain the longest
/// depth reachable from their non-cycle predecessors.
fn compute_depth(n: usize, adj: &[Vec<usize>], nodes: &[String]) -> HashMap<String, usize> {
    let mut in_deg: Vec<usize> = vec![0; n];
    for i in 0..n {
        for &j in &adj[i] { in_deg[j] += 1; }
    }

    let mut dist: Vec<usize> = vec![0; n];
    let mut queue: std::collections::VecDeque<usize> =
        (0..n).filter(|&i| in_deg[i] == 0).collect();

    while let Some(u) = queue.pop_front() {
        for &v in &adj[u] {
            if dist[v] < dist[u] + 1 { dist[v] = dist[u] + 1; }
            in_deg[v] -= 1;
            if in_deg[v] == 0 { queue.push_back(v); }
        }
    }

    nodes.iter().enumerate().map(|(i, id)| (id.clone(), dist[i])).collect()
}

/// Returns a human-readable metrics summary for the dependency diagram.
pub fn dependency_metrics_text(diagram: &DependencyDiagram) -> String {
    let m = &diagram.metrics;
    let mut out = String::new();

    out.push_str("=== Dependency Metrics ===\n\n");
    out.push_str(&format!("Packages         : {}\n", diagram.packages.len()));
    out.push_str(&format!("Direct deps      : {}\n", diagram.edges.len()));
    out.push_str(&format!("Cycles           : {}\n", m.cycles.len()));
    out.push_str(&format!("Packages in cycle: {}\n", m.in_cycle.len()));

    // ── Cycles ──────────────────────────────────────────────────────────────
    if !m.cycles.is_empty() {
        out.push_str("\n--- Cycles ---\n");
        for (i, cycle) in m.cycles.iter().enumerate() {
            out.push_str(&format!("  Cycle {} ({} packages):\n", i + 1, cycle.len()));
            for id in cycle {
                out.push_str(&format!("    {}\n", id));
            }
        }
    }

    // ── Most coupled ────────────────────────────────────────────────────────
    out.push_str("\n--- Most Coupled Packages (fan-in + fan-out, top 10) ---\n");
    let mut coupling: Vec<(&str, usize, usize)> = diagram.packages.iter()
        .map(|p| {
            let fi = m.fan_in.get(&p.id).copied().unwrap_or(0);
            let fo = m.fan_out.get(&p.id).copied().unwrap_or(0);
            (p.id.as_str(), fi, fo)
        })
        .collect();
    coupling.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));
    out.push_str(&format!("  {:<60}  fi   fo  total\n", "package"));
    out.push_str(&format!("  {:-<60}  ---  ---  -----\n", ""));
    for (id, fi, fo) in coupling.iter().take(10) {
        out.push_str(&format!("  {:<60}  {:3}  {:3}  {:5}\n", id, fi, fo, fi + fo));
    }

    // ── Depth ───────────────────────────────────────────────────────────────
    let max_depth = m.depth.values().copied().max().unwrap_or(0);
    out.push_str(&format!("\n--- Dependency Depth (longest path: {}) ---\n", max_depth));
    let mut deep: Vec<(&str, usize)> = m.depth.iter()
        .map(|(id, &d)| (id.as_str(), d))
        .collect();
    deep.sort_by(|a, b| b.1.cmp(&a.1));
    out.push_str(&format!("  {:>5}  {}\n", "depth", "package"));
    out.push_str(&format!("  {:-<5}  {:-<60}\n", "", ""));
    for (id, d) in deep.iter().take(15) {
        out.push_str(&format!("  {:>5}  {}\n", d, id));
    }

    out
}
