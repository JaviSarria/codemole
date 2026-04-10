/// Call-graph BFS traversal — language-agnostic driver.
///
/// For each node in the queue we:
///   1. Read the source file
///   2. Find the function/method body
///   3. Extract call sites (filtered: only calls found in the codebase)
///   4. Resolve each call to a file + class + method
///   5. Add unvisited callees to the queue
///
/// We skip:
///   • Standard library symbols (see STDLIB_* lists)
///   • Calls whose definition is not found in the scanned codebase
use std::collections::{HashMap, HashSet, VecDeque};
use regex::Regex;
use walkdir::WalkDir;

use crate::finder::EntryPoint;

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
    match lang {
        "java" => traverse(root, entry, &java_extractor(skip_symbols)),
        "python" => traverse(root, entry, &python_extractor(skip_symbols)),
        "go" => traverse(root, entry, &go_extractor(skip_symbols)),
        _ => CallGraph::default(),
    }
}

// ---------------------------------------------------------------------------
// Language-specific patterns packaged as an Extractor struct
// ---------------------------------------------------------------------------

struct Extractor {
    /// Extension of source files to scan
    ext: &'static str,
    /// Regex to locate a function/method definition: capture group 1 = name
    re_def: Regex,
    /// Regex to extract call sites from a body line: capture group 1 = callee name
    re_call: Regex,
    /// Identifiers to skip — loaded from the DB at startup
    stdlib: HashSet<String>,
}

fn java_extractor(skip: HashSet<String>) -> Extractor {
    Extractor {
        ext: "java",
        re_def: Regex::new(
            r"(?:public|protected|private|static|\s)+[\w<>\[\]]+\s+(\w+)\s*\(",
        )
        .unwrap(),
        re_call: Regex::new(r"\b(\w+)\s*\(").unwrap(),
        stdlib: skip,
    }
}

fn python_extractor(skip: HashSet<String>) -> Extractor {
    Extractor {
        ext: "py",
        re_def: Regex::new(r"^(?:async\s+)?def\s+(\w+)\s*\(").unwrap(),
        re_call: Regex::new(r"\b(\w+)\s*\(").unwrap(),
        stdlib: skip,
    }
}

fn go_extractor(skip: HashSet<String>) -> Extractor {
    Extractor {
        ext: "go",
        re_def: Regex::new(r"^func\s+(?:\([^)]+\)\s+)?(\w+)\s*\(").unwrap(),
        re_call: Regex::new(r"\b(\w+)\s*\(").unwrap(),
        stdlib: skip,
    }
}

// ---------------------------------------------------------------------------
// Core BFS traversal
// ---------------------------------------------------------------------------

fn traverse(root: &str, entry: &EntryPoint, extractor: &Extractor) -> CallGraph {
    // Index all definitions in the codebase: method_name → Vec<(file, line, class)>
    let def_index = build_def_index(root, extractor);

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
        // Read body of the current function
        let (body_calls, return_type, return_expr) = extract_body_info(root, &current, extractor);
        current.return_type = return_type;
        current.return_expr = return_expr;

        graph.nodes.push(current.clone());

        for callee_name in body_calls {
            if extractor.stdlib.contains(&callee_name) {
                continue;
            }
            // Java: skip constructor calls (uppercase-first identifiers)
            if extractor.ext == "java"
                && callee_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            {
                continue;
            }
            // Look up definition in codebase
            let defs = match def_index.get(&callee_name) {
                Some(d) => d,
                None => continue, // not in codebase → skip
            };
            // Prefer same-class def, then any non-interface def
            let def = choose_best_def(defs, &current.class);
            let callee_id = node_id(&def.class, &callee_name);

            // Record edge
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
    class: String,
    is_interface: bool,
}

fn build_def_index(root: &str, extractor: &Extractor) -> HashMap<String, Vec<DefInfo>> {
    let mut index: HashMap<String, Vec<DefInfo>> = HashMap::new();

    let files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|x| x == extractor.ext)
                .unwrap_or(false)
        })
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    let re_class = Regex::new(r"(?:^|\s)(class|interface|enum)\s+(\w+)").unwrap();

    for file in &files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        let rel_file = relative(file, root);

        let mut current_class = String::from("Unknown");
        let mut current_is_iface = false;

        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_class.captures(line) {
                current_class = cap[2].to_string();
                current_is_iface = &cap[1] == "interface";
            }
            if let Some(cap) = extractor.re_def.captures(line) {
                let method = cap[1].to_string();
                index.entry(method).or_default().push(DefInfo {
                    file: rel_file.clone(),
                    line: i + 1,
                    class: current_class.clone(),
                    is_interface: current_is_iface,
                });
            }
        }
    }
    index
}

fn choose_best_def<'a>(defs: &'a [DefInfo], caller_class: &str) -> &'a DefInfo {
    // 1. Same class, non-interface
    // 2. Any non-interface
    // 3. Fallback to first entry
    defs.iter()
        .find(|d| d.class == caller_class && !d.is_interface)
        .or_else(|| defs.iter().find(|d| !d.is_interface))
        .unwrap_or(&defs[0])
}

// ---------------------------------------------------------------------------
// Comment stripping
// ---------------------------------------------------------------------------

/// Strip comments from a slice of source lines.
/// Java/Go: removes `//` line comments and `/* */` block comments.
/// Python:  removes `#` line comments.
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
    // Java / Go: character-by-character pass to handle // and /* */
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
                break; // rest of line is a line comment
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
// Call-site extraction
// ---------------------------------------------------------------------------

fn extract_body_info(root: &str, node: &Node, extractor: &Extractor) -> (Vec<String>, String, String) {
    let full_path = build_full_path(root, &node.file);
    let content = match std::fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return (vec![], String::new(), String::new()),
    };
    let lines: Vec<&str> = content.lines().collect();

    let start = if node.line > 0 { node.line - 1 } else { 0 };

    // Extract return type from the method signature line (before parsing the body)
    let def_line = lines.get(start).copied().unwrap_or("");
    let return_type = extract_return_type(def_line, &node.method, extractor.ext);

    let raw_lines = find_body(&lines, start);
    let body_lines = strip_comments(raw_lines, extractor.ext);

    // Extract call sites
    let mut calls = Vec::new();
    let mut seen = HashSet::new();
    for line in &body_lines {
        for cap in extractor.re_call.captures_iter(line) {
            let name = cap[1].to_string();
            if seen.insert(name.clone()) {
                calls.push(name);
            }
        }
    }

    // Extract last `return <expr>` — fallback when the declared type is unavailable
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

/// Extract the declared return type from a method definition line.
/// Returns an empty string when the type cannot be determined or is void.
fn extract_return_type(def_line: &str, method_name: &str, ext: &str) -> String {
    match ext {
        "java" => {
            // Match: <ReturnType> methodName(
            // Handles simple types, arrays and one level of generics: List<User>, ResponseEntity<Object>
            let escaped = regex::escape(method_name);
            let pat = format!(r"([\w$]+(?:<[^>()]*>)?(?:\[\])*?)\s+{}\s*\(", escaped);
            if let Ok(re) = Regex::new(&pat) {
                if let Some(cap) = re.captures(def_line) {
                    let t = cap[1].trim().to_string();
                    let skip = [
                        "public", "protected", "private", "static", "final",
                        "synchronized", "abstract", "native", "void",
                    ];
                    if !t.is_empty() && !skip.contains(&t.as_str()) {
                        return t;
                    }
                }
            }
            String::new()
        }
        "py" => {
            // PEP 3107 return annotation: def foo(...) -> ReturnType:
            if let Some(arrow) = def_line.find("->") {
                let after = def_line[arrow + 2..].trim();
                let t = after.trim_end_matches(':').trim().to_string();
                if !t.is_empty() {
                    return t;
                }
            }
            String::new()
        }
        "go" => {
            // func (r *Recv) Name(args) ReturnType {
            // or func (r *Recv) Name(args) (Type1, Type2) {
            let line = def_line.trim_end_matches('{').trim();
            // Find the closing paren of the parameter list, then take what follows
            if let Some(pos) = line.rfind(')') {
                let ret = line[pos + 1..].trim().to_string();
                if !ret.is_empty() {
                    return ret;
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

/// Return the lines that constitute the body of the function starting at `start_idx`.
/// Uses brace counting for Java/Go, indentation for Python.
fn find_body<'a>(lines: &'a [&'a str], start: usize) -> Vec<&'a str> {
    if start >= lines.len() {
        return vec![];
    }
    // Determine language heuristic: if first line has '{', use braces; else use indent
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
            if c == '{' {
                depth += 1;
                started = true;
            } else if c == '}' {
                depth -= 1;
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
    // Get the indentation of the def line, body is deeper
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

