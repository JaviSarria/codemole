/// Gin (Go) endpoint finder.
///
/// Recognises:
///   r.GET("/path", handler)
///   r.POST("/path", handler)
///   group.DELETE("/path", handler)
///   (and put, patch variants, any receiver name)
use regex::Regex;
use walkdir::WalkDir;

use super::EntryPoint;

pub fn find(endpoint: &str, root: &str) -> Option<EntryPoint> {
    // Match:  <anything>.GET|POST|PUT|DELETE|PATCH("/path_or_placeholder", someHandler)
    let re_route = Regex::new(
        r#"(?i)\w+\.(GET|POST|PUT|DELETE|PATCH)\s*\(\s*["']([^"']*)["']"#,
    )
    .unwrap();

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

        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_route.captures(line) {
                let route_path = &cap[2];
                if !paths_match(endpoint, route_path) {
                    continue;
                }

                // Extract the handler name (last argument before closing paren)
                // e.g.  r.GET("/health", mypkg.HealthHandler)  →  HealthHandler
                let handler = extract_handler(line);
                let package = package_name(file, root);

                return Some(EntryPoint {
                    file: relative(file, root),
                    line: i + 1,
                    class: package.clone(),
                    method: handler,
                    interface_class: None,
                });
            }
        }
    }
    None
}

/// Extract the last comma-separated argument before `)`, strip package qualifier.
fn extract_handler(line: &str) -> String {
    if let Some(open) = line.find('(') {
        let args_part = &line[open + 1..];
        // Remove trailing ')' and anything after
        let args_part = args_part.split(')').next().unwrap_or(args_part);
        let parts: Vec<&str> = args_part.split(',').collect();
        if parts.len() >= 2 {
            let last = parts.last().unwrap().trim();
            // Strip package prefix like "pkg.Handler" → "Handler"
            let ident = last.rsplit('.').next().unwrap_or(last);
            return ident.to_string();
        }
    }
    "handler".to_string()
}

fn paths_match(want: &str, have: &str) -> bool {
    let want_parts: Vec<&str> = want.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let have_parts: Vec<&str> = have.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    if want_parts.len() > have_parts.len() { return false; }
    if want_parts.len() == have_parts.len() {
        return have_parts.iter().zip(want_parts.iter()).all(|(h, w)| {
            w == h || h.starts_with(':') || h.starts_with('{') || w.starts_with(':') || w.starts_with('{')
        });
    }
    let offset = have_parts.len() - want_parts.len();
    let slice = &have_parts[offset..];
    slice.iter().zip(want_parts.iter()).all(|(h, w)| {
        w == h || w.starts_with(':') || w.starts_with('{')
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
