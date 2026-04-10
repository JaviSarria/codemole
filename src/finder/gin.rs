/// Gin (Go) endpoint finder.
///
/// Recognises:
///   r.GET("/path", handler)
///   r.POST("/path", handler)
///   group.DELETE("/path", handler)
///   (and put, patch variants, any receiver name)
///
/// Also resolves Gin route groups, combining the group prefix with the
/// route path to build the full endpoint URL.
///
/// Returns the EntryPoint pointing to the DEFINITION of the handler method,
/// not to the route registration line.
use regex::Regex;
use std::collections::HashMap;
use walkdir::WalkDir;

use super::EntryPoint;

pub fn find(endpoint: &str, root: &str) -> Option<EntryPoint> {
    let re_route = Regex::new(
        r#"(?i)(\w+)\.(GET|POST|PUT|DELETE|PATCH)\s*\(\s*["']([^"']*)["']"#,
    )
    .unwrap();

    let re_group = Regex::new(
        r#"(\w+)\s*:?=\s*(\w+)\.Group\s*\(\s*["']([^"']*)["']"#,
    )
    .unwrap();

    // Matches: func (varName *StructType) or func (varName StructType)
    let re_func_recv = Regex::new(r"^func\s+\(\s*(\w+)\s+\*?(\w+)\s*\)").unwrap();

    let go_files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "go").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    for file in &go_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        // Build group map: var_name -> (parent_var, prefix)
        let mut group_map: HashMap<String, (String, String)> = HashMap::new();
        for line in &lines {
            if let Some(cap) = re_group.captures(line) {
                group_map.insert(
                    cap[1].to_string(),
                    (cap[2].to_string(), cap[3].to_string()),
                );
            }
        }

        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_route.captures(line) {
                let receiver = &cap[1];
                let route_path = &cap[3];

                let combined = build_full_path(receiver, route_path, &group_map);
                if !paths_match(endpoint, &combined) {
                    continue;
                }

                let handler = extract_handler(line);
                let recv_var = extract_receiver_var(line);

                // Resolve the struct type that owns the handler (e.g. TopologyHandler)
                let struct_type =
                    resolve_enclosing_struct(&lines, i, &recv_var, &re_func_recv);

                // Locate the actual handler method definition in the codebase
                let (def_file, def_line) = find_handler_def(
                    &go_files,
                    &handler,
                    struct_type.as_deref(),
                    root,
                )
                .unwrap_or_else(|| (relative(file, root), i + 1));

                let class = struct_type.unwrap_or_else(|| package_name(file, root));

                return Some(EntryPoint {
                    file: def_file,
                    line: def_line,
                    class,
                    method: handler,
                    interface_class: None,
                });
            }
        }
    }
    None
}

/// Extract the method name from a Gin route handler argument.
/// e.g. `group.POST("/path", h.HandlerMethod)` → "HandlerMethod"
fn extract_handler(line: &str) -> String {
    if let Some(open) = line.find('(') {
        let args_part = &line[open + 1..];
        let args_part = args_part.split(')').next().unwrap_or(args_part);
        let parts: Vec<&str> = args_part.split(',').collect();
        if parts.len() >= 2 {
            let last = parts.last().unwrap().trim();
            // Strip receiver/package prefix: "h.Method" → "Method"
            let ident = last.rsplit('.').next().unwrap_or(last);
            return ident.to_string();
        }
    }
    "handler".to_string()
}

/// Extract the receiver variable name from a Gin route handler argument.
/// e.g. `group.POST("/path", h.HandlerMethod)` → "h"
fn extract_receiver_var(line: &str) -> String {
    if let Some(open) = line.find('(') {
        let args_part = &line[open + 1..];
        let args_part = args_part.split(')').next().unwrap_or(args_part);
        let parts: Vec<&str> = args_part.split(',').collect();
        if parts.len() >= 2 {
            let last = parts.last().unwrap().trim();
            if let Some(dot_pos) = last.rfind('.') {
                return last[..dot_pos].trim().to_string();
            }
        }
    }
    String::new()
}

/// Scan upward from `before_line` to find `func (recv_var *StructType)` and
/// return "StructType". Returns None for package-level functions.
fn resolve_enclosing_struct(
    lines: &[&str],
    before_line: usize,
    recv_var: &str,
    re_func_recv: &Regex,
) -> Option<String> {
    if recv_var.is_empty() {
        return None;
    }
    for line in lines[..=before_line].iter().rev() {
        if let Some(cap) = re_func_recv.captures(line) {
            if &cap[1] == recv_var {
                return Some(cap[2].to_string());
            }
        }
    }
    None
}

/// Search all Go files for `func (... *StructType) handlerName(` and return
/// (relative_file, 1-based line). Falls back to None when not found.
fn find_handler_def(
    go_files: &[String],
    handler: &str,
    struct_type: Option<&str>,
    root: &str,
) -> Option<(String, usize)> {
    let pattern = if let Some(stype) = struct_type {
        format!(
            r"^func\s+\([^)]*\*?{}\s*\)\s+{}\s*\(",
            regex::escape(stype),
            regex::escape(handler),
        )
    } else {
        format!(r"^func\s+{}\s*\(", regex::escape(handler))
    };
    let re = Regex::new(&pattern).ok()?;

    for file in go_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                return Some((relative(file, root), i + 1));
            }
        }
    }
    None
}


/// Walk up the group chain and prepend all prefix segments to `route_path`.
fn build_full_path(
    receiver: &str,
    route_path: &str,
    group_map: &HashMap<String, (String, String)>,
) -> String {
    let mut prefixes: Vec<String> = Vec::new();
    let mut current = receiver.to_string();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        if !visited.insert(current.clone()) {
            break;
        }
        match group_map.get(&current) {
            Some((parent, prefix)) => {
                prefixes.push(prefix.clone());
                current = parent.clone();
            }
            None => break,
        }
    }

    prefixes.reverse();
    let mut full = String::new();
    for p in &prefixes {
        let p = p.trim_matches('/');
        if !p.is_empty() {
            full.push('/');
            full.push_str(p);
        }
    }
    let rp = route_path.trim_matches('/');
    if !rp.is_empty() {
        full.push('/');
        full.push_str(rp);
    }
    if full.is_empty() {
        "/".to_string()
    } else {
        full
    }
}

fn paths_match(want: &str, have: &str) -> bool {
    let want_parts: Vec<&str> = want.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let have_parts: Vec<&str> = have.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();

    if want_parts.len() == have_parts.len() {
        return parts_equal(&want_parts, &have_parts);
    }

    if have_parts.len() > want_parts.len() {
        // have is longer: check if want matches the tail of have
        let offset = have_parts.len() - want_parts.len();
        return parts_equal(&want_parts, &have_parts[offset..]);
    }

    // want is longer: check if have matches the tail of want
    let offset = want_parts.len() - have_parts.len();
    parts_equal(&have_parts, &want_parts[offset..])
}

fn parts_equal(a: &[&str], b: &[&str]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| {
        x == y
            || x.starts_with(':')
            || x.starts_with('{')
            || y.starts_with(':')
            || y.starts_with('{')
    })
}

fn relative(file: &str, root: &str) -> String {
    let root = root.trim_end_matches(['/', '\\']);
    file.strip_prefix(root)
        .unwrap_or(file)
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/")
}

fn package_name(file: &str, root: &str) -> String {
    let rel = relative(file, root);
    std::path::Path::new(&rel)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "main".to_string())
}
