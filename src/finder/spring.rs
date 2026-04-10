/// Spring Boot endpoint finder.
///
/// Recognises:
///   @GetMapping("/path")          @GetMapping(value = "/path")
///   @PostMapping, @PutMapping, @DeleteMapping, @PatchMapping
///   @RequestMapping("/path")      (class-level prefix + method-level)
///
/// When the matched class is an interface, it scans other files to find the
/// concrete implementation class so the call graph is built from business logic.
use std::collections::HashMap;
use regex::Regex;
use walkdir::WalkDir;

use super::EntryPoint;

pub fn find(endpoint: &str, root: &str) -> Option<EntryPoint> {
    // Collect all Java source files
    let java_files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "java").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    // ------------------------------------------------------------------
    // 1.  Build a "class scope" index: file → list of (start_line, end_line, class_name, is_interface)
    // ------------------------------------------------------------------
    let class_scopes = build_class_scopes(&java_files);

    // ------------------------------------------------------------------
    // 2.  Scan every file for a matching endpoint annotation
    // ------------------------------------------------------------------
    let re_mapping = Regex::new(
        r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*(?:\([^)]*?"([^"]*)"[^)]*?\)|\(\s*\))?"#,
    )
    .unwrap();

    for file in &java_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        // class-level @RequestMapping prefix
        let class_prefix = extract_class_prefix(&lines);

        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_mapping.captures(line) {
                let raw_path = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                // Normalise and match
                let full_path = normalise_path(&format!("{}{}", class_prefix, raw_path));
                if !paths_match(endpoint, &full_path) {
                    continue;
                }

                // Find the method that follows this annotation (skip blank/annotation lines)
                let (method_name, method_line) = find_next_method(&lines, i + 1);

                // Determine class name and whether it is an interface
                let (class_name, is_iface) = get_class_at_line(&class_scopes, file, i + 1);

                if is_iface {
                    // Try to resolve concrete implementation
                    if let Some(ep) = resolve_implementation(
                        &java_files,
                        &class_scopes,
                        &class_name,
                        &method_name,
                        &class_prefix,
                        raw_path,
                        root,
                    ) {
                        return Some(ep);
                    }
                    // Fall back to interface location
                    return Some(EntryPoint {
                        file: relative(file, root),
                        line: method_line,
                        class: class_name,
                        method: method_name,
                        interface_class: None,
                    });
                }

                return Some(EntryPoint {
                    file: relative(file, root),
                    line: method_line,
                    class: class_name,
                    method: method_name,
                    interface_class: None,
                });
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn relative(file: &str, root: &str) -> String {
    let root = root.trim_end_matches(['/', '\\']);
    file.strip_prefix(root)
        .unwrap_or(file)
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/")
}

fn normalise_path(p: &str) -> String {
    let p = if p.starts_with('/') {
        p.to_string()
    } else {
        format!("/{}", p)
    };
    // collapse double slashes
    let mut out = String::new();
    let mut prev = ' ';
    for c in p.chars() {
        if !(c == '/' && prev == '/') {
            out.push(c);
        }
        prev = c;
    }
    out
}

/// Path match: exact segment comparison (with `{param}` wildcards) OR
/// suffix match — the user may supply only the method-level part of the path
/// when the class has a prefix.
///
/// Important: `{param}` in the REGISTERED path is only treated as a wildcard
/// on a FULL-LENGTH match. In suffix matches, only the USER's `{param}` patterns
/// act as wildcards, to avoid false-positives (e.g. `{requestId}` matching
/// a literal endpoint name).
fn paths_match(want: &str, have: &str) -> bool {
    let want_parts: Vec<&str> = want.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let have_parts: Vec<&str> = have.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

    if want_parts.len() > have_parts.len() {
        return false;
    }

    if want_parts.len() == have_parts.len() {
        // Full-length match: both sides can use {param} wildcards
        return have_parts.iter().zip(want_parts.iter()).all(|(h, w)| {
            w == h || h.starts_with('{') || w.starts_with('{')
        });
    }

    // Suffix match (user omitted the class-level prefix).
    // Only the user's {param} patterns act as wildcards — registered path
    // parameters are matched literally to avoid false positives.
    let offset = have_parts.len() - want_parts.len();
    let slice = &have_parts[offset..];
    slice.iter().zip(want_parts.iter()).all(|(h, w)| {
        w == h || w.starts_with('{')
    })
}

fn extract_class_prefix(lines: &[&str]) -> String {
    let re = Regex::new(r#"@RequestMapping\s*\(\s*[^)]*?"([^"]*)"#).unwrap();
    for line in lines {
        if let Some(cap) = re.captures(line) {
            return cap[1].to_string();
        }
    }
    String::new()
}

fn find_next_method(lines: &[&str], from: usize) -> (String, usize) {
    let re_method = Regex::new(
        r"(?:public|protected|private|default)\s+\S+\s+(\w+)\s*\(",
    )
    .unwrap();
    for i in from..lines.len() {
        if let Some(cap) = re_method.captures(lines[i]) {
            return (cap[1].to_string(), i + 1); // 1-based
        }
    }
    ("unknown".to_string(), from + 1)
}

// ---------------------------------------------------------------------------
// Class scope index
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ClassScope {
    start: usize, // 1-based
    end: usize,
    name: String,
    is_interface: bool,
}

fn build_class_scopes(files: &[String]) -> HashMap<String, Vec<ClassScope>> {
    let re_class = Regex::new(
        r"(?:^|\s)(class|interface|enum)\s+(\w+)",
    )
    .unwrap();

    let mut result = HashMap::new();
    for file in files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        let mut scopes: Vec<ClassScope> = Vec::new();
        let mut brace_depth = 0usize;
        let mut pending: Vec<(String, bool, usize)> = Vec::new(); // (name, is_iface, open_line)

        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_class.captures(line) {
                let kind = &cap[1];
                let name = cap[2].to_string();
                let is_iface = kind == "interface";
                // Mark start at next { or same line
                if line.contains('{') {
                    pending.push((name, is_iface, i + 1));
                    brace_depth += line.chars().filter(|&c| c == '{').count();
                    brace_depth -= line.chars().filter(|&c| c == '}').count();
                } else {
                    pending.push((name, is_iface, i + 1));
                }
            } else {
                let opens = line.chars().filter(|&c| c == '{').count();
                let closes = line.chars().filter(|&c| c == '}').count();
                brace_depth += opens;
                if brace_depth >= closes {
                    brace_depth -= closes;
                } else {
                    brace_depth = 0;
                }
            }

            // When we have no more open braces, finalise topmost pending scope
            if brace_depth == 0 && !pending.is_empty() {
                for (name, is_iface, start) in pending.drain(..) {
                    scopes.push(ClassScope {
                        start,
                        end: i + 1,
                        name,
                        is_interface: is_iface,
                    });
                }
            }
        }
        // Close any still-open scopes
        let n = lines.len();
        for (name, is_iface, start) in pending.drain(..) {
            scopes.push(ClassScope { start, end: n, name, is_interface: is_iface });
        }
        result.insert(file.clone(), scopes);
    }
    result
}

fn get_class_at_line(
    scopes: &HashMap<String, Vec<ClassScope>>,
    file: &str,
    line: usize,
) -> (String, bool) {
    if let Some(file_scopes) = scopes.get(file) {
        // Prefer most-inner scope that contains the line
        let mut best: Option<&ClassScope> = None;
        for scope in file_scopes {
            if scope.start <= line && line <= scope.end {
                if best.is_none() || (scope.end - scope.start) < (best.unwrap().end - best.unwrap().start) {
                    best = Some(scope);
                }
            }
        }
        if let Some(s) = best {
            return (s.name.clone(), s.is_interface);
        }
    }
    ("Unknown".to_string(), false)
}

// ---------------------------------------------------------------------------
// Interface → Implementation resolution
// ---------------------------------------------------------------------------

fn resolve_implementation(
    java_files: &[String],
    _class_scopes: &HashMap<String, Vec<ClassScope>>,
    iface_name: &str,
    method_name: &str,
    _class_prefix: &str,
    _raw_path: &str,
    root: &str,
) -> Option<EntryPoint> {
    // Regex that matches "class Foo implements ... InterfaceName ..."
    let re_impl = Regex::new(&format!(
        r"class\s+(\w+)\s+(?:extends\s+\w+\s+)?implements\s+[^{{]*\b{}\b",
        regex::escape(iface_name)
    ))
    .ok()?;

    let re_method = Regex::new(
        r"(?:public|protected|private|default)\s+\S+\s+(\w+)\s*\(",
    )
    .unwrap();

    for file in java_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        // Check if this file has an implementing class
        let mut impl_class: Option<String> = None;
        for line in &lines {
            if let Some(cap) = re_impl.captures(line) {
                impl_class = Some(cap[1].to_string());
                break;
            }
        }
        let impl_class = match impl_class {
            Some(c) => c,
            None => continue,
        };

        // Find the method in this file
        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_method.captures(line) {
                if &cap[1] == method_name {
                    return Some(EntryPoint {
                        file: relative(file, root),
                        line: i + 1,
                        class: impl_class.clone(),
                        method: method_name.to_string(),
                        interface_class: Some(iface_name.to_string()),
                    });
                }
            }
        }

        // Method not found in implementing class — return class location anyway
        return Some(EntryPoint {
            file: relative(file, root),
            line: 1,
            class: impl_class,
            method: method_name.to_string(),
            interface_class: Some(iface_name.to_string()),
        });
    }
    None
}
