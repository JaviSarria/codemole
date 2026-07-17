/// Component diagram — groups CallGraph nodes into modules/packages and surfaces
/// inter-module dependencies.
///
/// # Algorithm
///
/// 1. **Module extraction** — derives a module identifier from each node's file path.
///    - Java: strips `src/main/java/` prefix and converts the remainder to dot notation
///      (`com.example.controller`).  The last segment is used as the display label.
///    - Python / Go: uses the immediate parent directory name as the module identifier
///      (e.g. `handler`, `routers`).
///
/// 2. **Layer inference** — classifies each module into one of five architectural
///    layers by scanning the module path and class name for well-known keywords
///    (controller, service, repository, …).  When multiple nodes in a module
///    suggest different layers the most specific one wins.
///
/// 3. **Dependency extraction** — iterates CallGraph edges; whenever the caller
///    and callee belong to different modules, an inter-module dependency edge is
///    recorded.  Duplicate edges are deduplicated.
use std::collections::{HashMap, HashSet};
use crate::parser::CallGraph;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    Controller,
    Service,
    Repository,
    Infrastructure,
    Other,
}

impl Layer {
    pub fn label(self) -> &'static str {
        match self {
            Layer::Controller     => "Presentation",
            Layer::Service        => "Business",
            Layer::Repository     => "Data",
            Layer::Infrastructure => "Infrastructure",
            Layer::Other          => "Other",
        }
    }

    /// DOT-safe suffix used in subgraph ids.
    pub fn id(self) -> &'static str {
        match self {
            Layer::Controller     => "presentation",
            Layer::Service        => "business",
            Layer::Repository     => "data",
            Layer::Infrastructure => "infra",
            Layer::Other          => "other",
        }
    }

    /// Background fill for the layer swimlane.
    pub fn band_color(self) -> &'static str {
        match self {
            Layer::Controller     => "#f0f5ff",
            Layer::Service        => "#f0fff5",
            Layer::Repository     => "#fff8f0",
            Layer::Infrastructure => "#f8f8f8",
            Layer::Other          => "#fdfdfd",
        }
    }

    /// Fill colour for a module box inside its swimlane.
    pub fn box_color(self) -> &'static str {
        match self {
            Layer::Controller     => "#deeeff",
            Layer::Service        => "#d8f8e4",
            Layer::Repository     => "#ffeedd",
            Layer::Infrastructure => "#eeeeee",
            Layer::Other          => "#f5f5f5",
        }
    }

    /// Border / header colour for a module box.
    pub fn border_color(self) -> &'static str {
        match self {
            Layer::Controller     => "#4a90d9",
            Layer::Service        => "#3a9a50",
            Layer::Repository     => "#d07820",
            Layer::Infrastructure => "#888888",
            Layer::Other          => "#aaaaaa",
        }
    }

    /// All layers in display order (most "upstream" first).
    pub fn all_ordered() -> [Layer; 5] {
        [
            Layer::Controller,
            Layer::Service,
            Layer::Repository,
            Layer::Infrastructure,
            Layer::Other,
        ]
    }

    fn rank(self) -> u8 {
        match self {
            Layer::Controller     => 0,
            Layer::Service        => 1,
            Layer::Repository     => 2,
            Layer::Infrastructure => 3,
            Layer::Other          => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModuleNode {
    /// Unique module identifier (full package / path, separator-normalized).
    pub id: String,
    /// Human-readable display label (last path segment).
    pub label: String,
    pub layer: Layer,
    /// Distinct classes / structs / functions inside this module.
    pub classes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ComponentDiagram {
    pub modules: Vec<ModuleNode>,
    /// Directed dependency edges `(from_id, to_id)`.
    pub deps: Vec<(String, String)>,
    /// Subset of `deps` that cross layer boundaries (architecture violations).
    pub violations: HashSet<(String, String)>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a [`ComponentDiagram`] from a call graph.
pub fn build_component_diagram(lang: &str, graph: &CallGraph) -> ComponentDiagram {
    if graph.nodes.is_empty() {
        return ComponentDiagram::default();
    }

    // 1. Derive a module id for every graph node
    let node_mods: Vec<String> = graph.nodes.iter()
        .map(|n| extract_module(lang, &n.file))
        .collect();

    // 2. Build ModuleNode entries in BFS-visit order
    let mut mod_label:   HashMap<String, String>          = HashMap::new();
    let mut mod_layer:   HashMap<String, Layer>           = HashMap::new();
    let mut mod_classes: HashMap<String, HashSet<String>> = HashMap::new();
    let mut mod_order:   Vec<String>                      = Vec::new();

    for (node, m) in graph.nodes.iter().zip(node_mods.iter()) {
        let layer = infer_layer(m, &node.class);
        if !mod_label.contains_key(m) {
            mod_order.push(m.clone());
            mod_label.insert(m.clone(), module_label(lang, m));
        }
        // Keep the most specific (lowest rank) layer found for this module
        let entry = mod_layer.entry(m.clone()).or_insert(Layer::Other);
        if layer.rank() < entry.rank() {
            *entry = layer;
        }
        mod_classes.entry(m.clone()).or_default().insert(node.class.clone());
    }

    let modules: Vec<ModuleNode> = mod_order.iter().map(|id| {
        let mut classes: Vec<String> = mod_classes.remove(id)
            .unwrap_or_default().into_iter().collect();
        classes.sort();
        ModuleNode {
            id: id.clone(),
            label: mod_label.remove(id).unwrap_or_default(),
            layer: mod_layer.remove(id).unwrap_or(Layer::Other),
            classes,
        }
    }).collect();

    // 3. Inter-module deps from call graph
    let id_to_mod: HashMap<&str, &str> = graph.nodes.iter().zip(node_mods.iter())
        .map(|(n, m)| (n.id.as_str(), m.as_str()))
        .collect();

    let mut deps_set: HashSet<(String, String)> = HashSet::new();
    for edge in &graph.edges {
        let fm = id_to_mod.get(edge.from.as_str()).copied().unwrap_or("");
        let tm = id_to_mod.get(edge.to.as_str()).copied().unwrap_or("");
        if !fm.is_empty() && !tm.is_empty() && fm != tm {
            deps_set.insert((fm.to_string(), tm.to_string()));
        }
    }
    let mut deps: Vec<(String, String)> = deps_set.into_iter().collect();
    deps.sort();

    // 4. Violation detection
    let mod_layer_map: HashMap<&str, Layer> = modules.iter().map(|m| (m.id.as_str(), m.layer)).collect();
    let violations: HashSet<(String, String)> = deps.iter()
        .filter(|(f, t)| {
            let fl = mod_layer_map.get(f.as_str()).copied().unwrap_or(Layer::Other);
            let tl = mod_layer_map.get(t.as_str()).copied().unwrap_or(Layer::Other);
            is_layer_violation(fl, tl)
        })
        .cloned().collect();

    ComponentDiagram { modules, deps, violations }
}

/// Serialize a [`ComponentDiagram`] to Graphviz DOT format.
pub fn component_dot(diagram: &ComponentDiagram) -> String {
    let mut out = String::new();
    out.push_str("digraph components {\n");
    out.push_str("  graph [rankdir=TB, bgcolor=\"white\", fontname=\"Helvetica\",\n");
    out.push_str("         nodesep=0.9, ranksep=1.3, concentrate=true];\n");
    out.push_str("  node  [fontname=\"Helvetica\", fontsize=10, margin=0.18];\n");
    out.push_str("  edge  [style=dashed, arrowhead=open, color=\"#555555\"];\n\n");

    // Group by layer in display order
    let mut by_layer: HashMap<&'static str, Vec<&ModuleNode>> = HashMap::new();
    for m in &diagram.modules {
        by_layer.entry(m.layer.id()).or_default().push(m);
    }

    for layer in Layer::all_ordered() {
        let nodes = match by_layer.get_mut(layer.id()) {
            Some(v) if !v.is_empty() => v,
            _ => continue,
        };

        out.push_str(&format!("  subgraph cluster_{} {{\n", layer.id()));
        out.push_str(&format!("    label=<<B>{}</B>>;\n", layer.label()));
        out.push_str(&format!("    style=filled; fillcolor=\"{}\";\n", layer.band_color()));
        out.push_str(&format!("    color=\"{}\"; penwidth=2;\n", layer.border_color()));
        out.push_str("    fontname=\"Helvetica\"; fontsize=12;\n");

        for m in nodes.iter() {
            let did   = dot_id(&m.id);
            let parts = if m.classes.is_empty() {
                m.label.clone()
            } else {
                format!("{}\\n{}", m.label, m.classes.join("\\n"))
            };
            out.push_str(&format!(
                "    {} [label=\"{}\", shape=component, style=filled,\n       fillcolor=\"{}\", color=\"{}\"];\n",
                did, dot_esc(&parts), layer.box_color(), layer.border_color()
            ));
        }
        out.push_str("  }\n\n");
    }

    for (from, to) in &diagram.deps {
        let did = dot_id(from);
        let tid = dot_id(to);
        if diagram.violations.contains(&(from.clone(), to.clone())) {
            out.push_str(&format!(
                "  {} -> {} [style=solid, color=\"#cc2222\", penwidth=2.5, \
                 label=\"\u{26a0} violation\", fontcolor=\"#cc2222\"];\n",
                did, tid
            ));
        } else {
            out.push_str(&format!("  {} -> {};\n", did, tid));
        }
    }

    out.push_str("}\n");
    out
}

// ---------------------------------------------------------------------------
// Module extraction
// ---------------------------------------------------------------------------

fn extract_module(lang: &str, file: &str) -> String {
    let norm = file.replace('\\', "/");
    let dir  = match norm.rfind('/') {
        Some(i) => norm[..i].to_string(),
        None    => String::new(),
    };
    match lang {
        "java" => java_pkg(&dir),
        _      => last_segment(&dir),
    }
}

fn java_pkg(dir: &str) -> String {
    const PREFIXES: &[&str] = &[
        "src/main/java/",
        "src/test/java/",
        "main/java/",
        "test/java/",
        "src/main/",
        "src/test/",
        "src/",
    ];
    for &pfx in PREFIXES {
        if let Some(idx) = dir.find(pfx) {
            let rel = dir[idx + pfx.len()..].trim_matches('/');
            let pkg = rel.replace('/', ".");
            if !pkg.is_empty() {
                return pkg;
            }
        }
    }
    // Fallback: last two directory segments as pseudo-package
    let parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => "root".to_string(),
        1 => parts[0].to_string(),
        _ => format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]),
    }
}

fn last_segment(dir: &str) -> String {
    dir.split('/').filter(|s| !s.is_empty()).last()
        .unwrap_or("root")
        .to_string()
}

fn module_label(lang: &str, module_id: &str) -> String {
    if lang == "java" {
        module_id.split('.').last().unwrap_or(module_id).to_string()
    } else {
        module_id.to_string()
    }
}

// ---------------------------------------------------------------------------
// Layer inference
// ---------------------------------------------------------------------------

pub(crate) fn infer_layer(module_id: &str, class: &str) -> Layer {
    let s   = format!("{} {}", module_id.to_lowercase(), class.to_lowercase());
    let has = |kw: &str| s.contains(kw);

    if      has("controller") || has("handler") || has("resource") || has("rest") || has("route") {
        Layer::Controller
    } else if has("service") || has("usecase") || has("use_case") || has("business") {
        Layer::Service
    } else if has("repository") || has("repo") || has("dao") || has("store") || has("mapper") || has("persistence") {
        Layer::Repository
    } else if has("config") || has("util") || has("helper") || has("common") || has("infra") || has("middleware") {
        Layer::Infrastructure
    } else {
        Layer::Other
    }
}

// ---------------------------------------------------------------------------
// DOT helpers
// ---------------------------------------------------------------------------

/// Returns true when `from` directly depending on `to` crosses architectural
/// layer boundaries (e.g. Presentation → Data, or any upward dependency).
///
/// Infrastructure and Other layers are cross-cutting and never trigger
/// violations regardless of direction.
pub(crate) fn is_layer_violation(from: Layer, to: Layer) -> bool {
    // Cross-cutting layers are exempt in both directions
    if from == Layer::Infrastructure || from == Layer::Other { return false; }
    if to   == Layer::Infrastructure || to   == Layer::Other { return false; }
    let fr = from.rank() as i8;
    let tr = to.rank() as i8;
    // Upward dependency OR layer skip (e.g. Presentation → Data)
    tr < fr || tr - fr > 1
}

fn dot_id(s: &str) -> String {
    let id: String = s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    format!("m_{}", id)
}

fn dot_esc(s: &str) -> String {
    s.replace('"', "\\\"")
}
