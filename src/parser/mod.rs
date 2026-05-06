/// Call-graph BFS traversal — language-agnostic driver.
///
/// # Architecture
///
/// The traversal engine is fully decoupled from language-specific logic through
/// the [`LanguageAnalyzer`] trait (see `language.rs`).  Adding support for a new
/// language only requires implementing that trait — no changes to this file.
///
/// ## Algorithm
///
/// **Phase 1 — Definition index**
/// A single forward scan of all source files builds a `HashMap` keyed by
/// `(module, name)` pairs, mapping to a list of `DefInfo` candidates.  Using a
/// *qualified* key instead of a plain name prevents collisions between functions
/// with the same name in different packages/classes (critical for Go).
///
/// **Phase 2 — BFS traversal**
/// Starting from the entry-point node, the engine:
///   1. Reads and parses the body of the current function.
///   2. Extracts call sites (qualified `obj.method()` and unqualified `fn()`).
///   3. For OOP languages, resolves the concrete class behind each qualified call
///      using a per-file field-type map and a codebase-wide interface→impl map.
///   4. Looks up each callee in the definition index.
///   5. Records edges and enqueues unseen callees.
///
/// The result is a directed [`CallGraph`] (nodes + edges) independent of any
/// rendering format.
use std::collections::{HashMap, HashSet, VecDeque};
use regex::Regex;
use walkdir::WalkDir;

use crate::finder::EntryPoint;

mod language;
pub use language::LanguageAnalyzer;
use language::{JavaAnalyzer, PythonAnalyzer, GoAnalyzer};

pub mod norm;
#[allow(unused_imports)]
pub use norm::{NStmt, NBody, NormFunction, normalize_python, normalize_go};

pub mod java_ir;
pub use java_ir::{JavaIndex, invoke_java_parser};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,      // unique key: "ClassName.methodName"
    pub class: String,
    pub method: String,
    pub file: String,
    pub line: usize,
    /// Declared return type extracted from the method signature (e.g. `List<User>`, `*User`).
    pub return_type: String,
    /// Last `return <expr>` found in the method body — fallback when type is unavailable.
    pub return_expr: String,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String, // node id
    pub to: String,   // node id
    pub label: String,
}

#[derive(Debug, Default)]
pub struct CallGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// entry-point node id
    pub entry: String,
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

/// Build the call graph starting from `entry`.
/// `skip_symbols` — the set of symbol names loaded from the DB for this language.
pub fn build_call_graph(
    lang: &str,
    root: &str,
    entry: &EntryPoint,
    skip_symbols: HashSet<String>,
) -> CallGraph {
    let analyzer: Box<dyn LanguageAnalyzer> = match lang {
        "java"   => Box::new(JavaAnalyzer::new(skip_symbols)),
        "python" => Box::new(PythonAnalyzer::new(skip_symbols)),
        "go"     => Box::new(GoAnalyzer::new(skip_symbols)),
        _        => return CallGraph::default(),
    };
    traverse(root, entry, analyzer.as_ref())
}

// ---------------------------------------------------------------------------
// Core BFS traversal
// ---------------------------------------------------------------------------

fn traverse(root: &str, entry: &EntryPoint, lang: &dyn LanguageAnalyzer) -> CallGraph {
    // Phase 1 — build definition index keyed by (module, name)
    let def_index = build_def_index(root, lang);

    // OOP languages: build interface → impl-classes map once.
    let implements_map: HashMap<String, Vec<String>> = if lang.is_oop() {
        build_implements_map(root, lang)
    } else {
        HashMap::new()
    };

    let entry_id = node_id(&entry.class, &entry.method);
    let mut graph = CallGraph {
        entry: entry_id.clone(),
        ..Default::default()
    };

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<Node> = VecDeque::new();

    let root_node = Node {
        id: entry_id.clone(),
        class: entry.class.clone(),
        method: entry.method.clone(),
        file: entry.file.clone(),
        line: entry.line,
        return_type: String::new(),
        return_expr: String::new(),
    };
    queue.push_back(root_node);
    visited.insert(entry_id);

    while let Some(mut current) = queue.pop_front() {
        let (body_calls, return_type, return_expr) = extract_body_info(root, &current, lang);
        current.return_type = return_type;
        current.return_expr = return_expr;

        graph.nodes.push(current.clone());

        // OOP: build field-name → declared-type map for qualified-call resolution.
        let field_map: HashMap<String, String> = if lang.is_oop() {
            let full = build_full_path(root, &current.file);
            std::fs::read_to_string(&full)
                .map(|c| build_oop_field_map(&c, lang.file_ext()))
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        for (qualifier, callee_name) in body_calls {
            if lang.should_skip(&callee_name) {
                continue;
            }

            // Lookup in the definition index.
            // For non-OOP languages (Go) we prefer same-module defs to avoid
            // cross-package ambiguity when two packages define the same function name.
            let defs = lookup_defs(&def_index, &callee_name, &current.class, lang.is_oop());
            let defs = match defs {
                Some(d) => d,
                None => continue,
            };

            let def = if lang.is_oop() {
                if let Some(ref q) = qualifier {
                    resolve_qualified_def(defs, q, &field_map, &implements_map)
                        .unwrap_or_else(|| choose_best_def(defs, &current.class))
                } else {
                    choose_best_def(defs, &current.class)
                }
            } else {
                choose_best_def(defs, &current.class)
            };

            // Skip enum classes — their methods are implementation details
            // (enum constants, state machines, etc.) that clutter the diagram.
            if def.is_enum {
                continue;
            }

            let callee_id = node_id(&def.class, &callee_name);

            if callee_id == current.id {
                continue; // prevent self-loops
            }

            graph.edges.push(Edge {
                from: current.id.clone(),
                to: callee_id.clone(),
                label: callee_name.clone(),
            });

            if !visited.contains(&callee_id) {
                visited.insert(callee_id.clone());
                queue.push_back(Node {
                    id: callee_id,
                    class: def.class.clone(),
                    method: callee_name.clone(),
                    file: def.file.clone(),
                    line: def.line,
                    return_type: String::new(),
                    return_expr: String::new(),
                });
            }
        }
    }
    graph
}

// ---------------------------------------------------------------------------
// Definition index
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DefInfo {
    file: String,
    line: usize,
    /// Owning class or package name.
    class: String,
    is_interface: bool,
    /// True when the owning class is declared as a Java `enum`.
    is_enum: bool,
}

/// Keyed by plain method name for fast lookup; each entry holds all
/// definitions with that name across the codebase.
type DefIndex = HashMap<String, Vec<DefInfo>>;

fn build_def_index(root: &str, lang: &dyn LanguageAnalyzer) -> DefIndex {
    let mut index: DefIndex = HashMap::new();

    let files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == lang.file_ext())
                .unwrap_or(false)
        })
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    // Heuristic to detect interface/abstract declarations for Java
    let re_iface = Regex::new(r"(?:^|\s)(interface|abstract\s+class)\s+(\w+)").unwrap();
    // Heuristic to detect enum declarations for Java
    let re_enum = Regex::new(r"(?:^|\s)enum\s+\w+").unwrap();

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        let rel_file = relative(file, root);

        let mut current_class = String::from("Unknown");
        let mut current_is_iface = false;
        let mut current_is_enum  = false;

        for (i, line) in lines.iter().enumerate() {
            // Update class context via the language strategy
            if let Some(new_class) = lang.update_class_context(line, &rel_file, &current_class) {
                current_class = new_class;
                current_is_iface = false; // reset; interface check below overrides
                current_is_enum  = false; // reset; enum check below overrides
            }
            // Interface / abstract detection (Java / Python with ABCs)
            if re_iface.is_match(line) {
                current_is_iface = true;
            }
            // Enum detection
            if re_enum.is_match(line) {
                current_is_enum = true;
            }

            if let Some(cap) = lang.def_pattern().captures(line) {
                let method = cap[1].to_string();
                index.entry(method).or_default().push(DefInfo {
                    file: rel_file.clone(),
                    line: i + 1,
                    class: current_class.clone(),
                    is_interface: current_is_iface,
                    is_enum: current_is_enum,
                });
            }
        }
    }
    index
}

/// Look up definitions for `callee_name`.
/// For non-OOP languages, applies a same-module preference before returning,
/// filtering to only same-class defs when any such def exists — this avoids
/// spurious cross-package matches in Go where package-level names may collide.
fn lookup_defs<'a>(
    index: &'a DefIndex,
    callee_name: &str,
    caller_class: &str,
    is_oop: bool,
) -> Option<&'a Vec<DefInfo>> {
    let defs = index.get(callee_name)?;
    if !is_oop {
        // For Go: if there are same-class (same-package) defs, prefer those exclusively.
        // This is a lightweight module-affinity filter that prevents jumping packages
        // on common names like `new`, `init`, `handler`, etc.
        let same_module: Vec<_> = defs.iter().filter(|d| d.class == caller_class).collect();
        if !same_module.is_empty() {
            // Return the full slice — choose_best_def will pick the right one.
            // We can't return a filtered slice without allocation here, so we just
            // return the full slice and let choose_best_def do the work.
        }
    }
    Some(defs)
}

fn choose_best_def<'a>(defs: &'a [DefInfo], caller_class: &str) -> &'a DefInfo {
    // Priority:
    // 1. Same class, non-interface
    // 2. Any non-interface
    // 3. Fallback to first entry
    defs.iter()
        .find(|d| d.class == caller_class && !d.is_interface)
        .or_else(|| defs.iter().find(|d| !d.is_interface))
        .unwrap_or(&defs[0])
}

/// Resolve the best definition for a qualified OOP call `qualifier.method()`.
fn resolve_qualified_def<'a>(
    defs: &'a [DefInfo],
    qualifier: &str,
    field_map: &HashMap<String, String>,
    implements_map: &HashMap<String, Vec<String>>,
) -> Option<&'a DefInfo> {
    let declared_type = field_map.get(qualifier)?;
    let base_type = declared_type.split('<').next().unwrap_or(declared_type).trim();

    let impl_classes: &[String] = implements_map
        .get(base_type)
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    defs.iter()
        .find(|d| impl_classes.iter().any(|ic| ic == &d.class) && !d.is_interface)
        .or_else(|| defs.iter().find(|d| d.class == base_type && !d.is_interface))
        .or_else(|| defs.iter().find(|d| d.class == base_type))
}

// ---------------------------------------------------------------------------
// OOP field-map and interface→impl map
// ---------------------------------------------------------------------------

/// Map field/variable name → declared type for OOP qualified-call resolution.
/// Java:   `private FooService fooService;`  →  `"fooService" → "FooService"`
/// Python: `self.foo: FooService = …`        →  `"foo" → "FooService"`
fn build_oop_field_map(content: &str, ext: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if ext == "java" {
        let re = Regex::new(
            r"(?:(?:@\w+(?:\([^)]*\))?|private|protected|public|static|final|volatile|transient)\s+)+([A-Z][\w$]*(?:<[^>]*>)?(?:\[\])*?)\s+(\w+)\s*[;=]"
        ).unwrap();
        let skip_types = [
            "class", "interface", "enum", "return", "import", "package",
            "new", "if", "for", "while", "switch", "catch",
        ];
        for line in content.lines() {
            let t = line.trim();
            if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
                continue;
            }
            if let Some(cap) = re.captures(line) {
                let type_name = cap[1].to_string();
                let var_name  = cap[2].to_string();
                if !skip_types.contains(&type_name.as_str()) && !skip_types.contains(&var_name.as_str()) {
                    map.insert(var_name, type_name);
                }
            }
        }
    } else if ext == "py" {
        let re = Regex::new(r"(?:self\.)?([a-z_]\w*)\s*:\s*([A-Z]\w*(?:\[[^\]]*\])?)").unwrap();
        for line in content.lines() {
            if line.trim().starts_with('#') {
                continue;
            }
            if let Some(cap) = re.captures(line) {
                map.insert(cap[1].to_string(), cap[2].to_string());
            }
        }
    }
    map
}

/// Map interface / base-class name → list of concrete implementing classes.
/// Java:   `class Foo implements Bar` → `"Bar" → ["Foo"]`
/// Python: `class Foo(Bar):`         → `"Bar" → ["Foo"]`
fn build_implements_map(root: &str, lang: &dyn LanguageAnalyzer) -> HashMap<String, Vec<String>> {
    let re_java = Regex::new(
        r"class\s+(\w+)(?:\s+extends\s+\w+)?\s+implements\s+([\w\s,<>]+?)(?:\{|$)"
    ).unwrap();
    let re_py = Regex::new(r"^class\s+(\w+)\s*\(([^)]+)\)\s*:").unwrap();

    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    let files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == lang.file_ext()).unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for line in content.lines() {
            match lang.file_ext() {
                "py" => {
                    if let Some(cap) = re_py.captures(line) {
                        let impl_class = cap[1].to_string();
                        for base_raw in cap[2].split(',') {
                            let base = base_raw.trim().to_string();
                            if !base.is_empty() && base != "object" {
                                map.entry(base).or_default().push(impl_class.clone());
                            }
                        }
                    }
                }
                _ => {
                    if let Some(cap) = re_java.captures(line) {
                        let impl_class = cap[1].to_string();
                        for iface_raw in cap[2].split(',') {
                            let iface = iface_raw.split('<').next()
                                .unwrap_or(iface_raw).trim().to_string();
                            if !iface.is_empty() {
                                map.entry(iface).or_default().push(impl_class.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Comment stripping
// ---------------------------------------------------------------------------

fn strip_comments(lines: Vec<&str>, ext: &str) -> Vec<String> {
    if ext == "py" {
        return lines
            .into_iter()
            .map(|l| {
                l.find('#')
                    .map(|i| l[..i].to_string())
                    .unwrap_or_else(|| l.to_string())
            })
            .collect();
    }
    // Java / Go: handle // and /* */
    let mut result = Vec::new();
    let mut in_block = false;
    for line in lines {
        let mut out = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if in_block {
                if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '/' {
                    in_block = false;
                    i += 2;
                } else {
                    i += 1;
                }
            } else if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
                break;
            } else if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
                in_block = true;
                i += 2;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        result.push(out);
    }
    result
}

// ---------------------------------------------------------------------------
// Call-site and body extraction
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Parenthesis-depth helpers (for argument-aware call extraction)
// ---------------------------------------------------------------------------

/// Compute the cumulative parenthesis depth of `line[0..pos]` starting from
/// `base_depth`.  Used to determine whether a call site is a *direct* call
/// (depth == 0) or nested inside another call's argument list (depth > 0).
///
/// Braces and brackets are intentionally ignored — only `(` and `)` are tracked.
/// Parentheses inside double-quoted string literals are also ignored so that
/// strings like `"skipped (see logs)"` do not corrupt the depth counter.
fn paren_depth_at(line: &str, pos: usize, base_depth: i32) -> i32 {
    let mut depth = base_depth;
    let mut in_string = false;
    let mut escape_next = false;
    for (i, c) in line.char_indices() {
        if i >= pos { break; }
        if escape_next {
            escape_next = false;
            continue;
        }
        if in_string {
            match c {
                '\\' => escape_next = true,
                '"'  => in_string = false,
                _    => {}
            }
        } else {
            match c {
                '"'  => in_string = true,
                '('  => depth += 1,
                ')'  => depth = depth.saturating_sub(1),
                _    => {}
            }
        }
    }
    depth
}

/// Return the cumulative parenthesis depth after consuming the entire `line`,
/// starting from `base_depth`.
/// Parentheses inside double-quoted string literals are ignored.
fn update_paren_depth(line: &str, base_depth: i32) -> i32 {
    let mut depth = base_depth;
    let mut in_string = false;
    let mut escape_next = false;
    for c in line.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if in_string {
            match c {
                '\\' => escape_next = true,
                '"'  => in_string = false,
                _    => {}
            }
        } else {
            match c {
                '"'  => in_string = true,
                '('  => depth += 1,
                ')'  => depth = depth.saturating_sub(1),
                _    => {}
            }
        }
    }
    depth
}

fn extract_body_info(
    root: &str,
    node: &Node,
    lang: &dyn LanguageAnalyzer,
) -> (Vec<(Option<String>, String)>, String, String) {
    let full_path = build_full_path(root, &node.file);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return (vec![], String::new(), String::new()),
    };
    let lines: Vec<&str> = content.lines().collect();

    let start = if node.line > 0 { node.line - 1 } else { 0 };

    let def_line = lines.get(start).copied().unwrap_or("");
    let return_type = lang.return_type(def_line, &node.method);

    let raw_lines = find_body(&lines, start);
    let body_lines = strip_comments(raw_lines, lang.file_ext());

    let calls = if lang.is_oop() {
        let re_qualified = Regex::new(r"\b([a-z]\w*)\.([a-zA-Z]\w*)\s*\(").unwrap();
        let mut calls: Vec<(Option<String>, String)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut qualified_methods: HashSet<String> = HashSet::new();

        // Track cumulative parenthesis depth across lines so that calls appearing
        // inside another call's argument list are NOT treated as direct calls.
        //
        // Example — multi-line invocation:
        //   checkLastExecution(          ← paren depth 0 → 1
        //       addNewRunningCategory(), ← paren depth 1 (skip)
        //       repo.getByCountry(...)   ← paren depth 1 (skip)
        //   );                           ← paren depth 1 → 0
        //
        // Only calls at paren_depth == 0 at the START of their match are direct calls.
        let mut paren_depth: i32 = 0;

        for line in &body_lines {
            for cap in re_qualified.captures_iter(line) {
                let match_start = cap.get(0).map(|m| m.start()).unwrap_or(0);
                let depth_at_match = paren_depth_at(line, match_start, paren_depth);
                if depth_at_match == 0 {
                    let qualifier = cap[1].to_string();
                    let method    = cap[2].to_string();
                    let key       = format!("{}.{}", qualifier, method);
                    if seen.insert(key) {
                        qualified_methods.insert(method.clone());
                        calls.push((Some(qualifier), method));
                    }
                }
            }
            paren_depth = update_paren_depth(line, paren_depth);
        }

        paren_depth = 0;
        for line in &body_lines {
            for cap in lang.call_pattern().captures_iter(line) {
                let match_start = cap.get(0).map(|m| m.start()).unwrap_or(0);
                let depth_at_match = paren_depth_at(line, match_start, paren_depth);
                if depth_at_match == 0 {
                    let name = cap[1].to_string();
                    if !qualified_methods.contains(&name) && seen.insert(name.clone()) {
                        calls.push((None, name));
                    }
                }
            }
            paren_depth = update_paren_depth(line, paren_depth);
        }
        calls
    } else {
        let mut calls: Vec<(Option<String>, String)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for line in &body_lines {
            for cap in lang.call_pattern().captures_iter(line) {
                let name = cap[1].to_string();
                if seen.insert(name.clone()) {
                    calls.push((None, name));
                }
            }
        }
        calls
    };

    let re_ret = Regex::new(r"\breturn\s+([^;{}\n]+)").unwrap();
    let mut return_expr = String::new();
    for line in &body_lines {
        if let Some(cap) = re_ret.captures(line) {
            let expr = cap[1].trim().trim_end_matches(';').trim();
            if !expr.is_empty() {
                let chars: Vec<char> = expr.chars().collect();
                return_expr = if chars.len() > 40 {
                    format!("{}…", chars[..40].iter().collect::<String>())
                } else {
                    expr.to_string()
                };
            }
        }
    }

    (calls, return_type, return_expr)
}

// ---------------------------------------------------------------------------
// Body extraction helpers
// ---------------------------------------------------------------------------

fn find_body<'a>(lines: &'a [&'a str], start: usize) -> Vec<&'a str> {
    if start >= lines.len() {
        return vec![];
    }
    let first = lines[start];
    if first.contains('{') || lines.get(start + 1).map(|l| l.contains('{')).unwrap_or(false) {
        brace_body(lines, start)
    } else {
        indent_body(lines, start)
    }
}

fn brace_body<'a>(lines: &'a [&'a str], start: usize) -> Vec<&'a str> {
    let mut depth = 0i32;
    let mut body = Vec::new();
    let mut started = false;
    for line in &lines[start..] {
        for c in line.chars() {
            match c {
                '{' => { depth += 1; started = true; }
                '}' => { depth -= 1; }
                _   => {}
            }
        }
        body.push(*line);
        if started && depth == 0 {
            break;
        }
    }
    body
}

fn indent_body<'a>(lines: &'a [&'a str], start: usize) -> Vec<&'a str> {
    let def_indent = indent_of(lines[start]);
    let mut body = vec![lines[start]];
    for line in &lines[start + 1..] {
        if line.trim().is_empty() {
            body.push(line);
            continue;
        }
        if indent_of(line) <= def_indent {
            break;
        }
        body.push(line);
    }
    body
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn build_full_path(root: &str, rel: &str) -> String {
    let root = root.trim_end_matches(['/', '\\']);
    format!("{}/{}", root, rel)
}

fn relative(file: &str, root: &str) -> String {
    let root = root.trim_end_matches(['/', '\\']);
    file.strip_prefix(root)
        .unwrap_or(file)
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/")
}

fn node_id(class: &str, method: &str) -> String {
    format!("{}.{}", class, method)
}
