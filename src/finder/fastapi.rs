/// FastAPI endpoint finder.
///
/// Recognises:
///   @app.get("/path")
///   @app.post("/path")
///   @router.get("/path", ...)
///   @router.post("/path", ...)
///   (and put, delete, patch variants)
///
/// Returns the function name immediately below the matched decorator.
use regex::Regex;
use walkdir::WalkDir;

use super::EntryPoint;

pub fn find(endpoint: &str, root: &str) -> Option<EntryPoint> {
    let re_decorator = Regex::new(
        r#"@(?:\w+)\.(get|post|put|delete|patch)\s*\(\s*["']([^"']*)["']"#,
    )
    .unwrap();
    let re_func = Regex::new(r"^(?:async\s+)?def\s+(\w+)\s*\(").unwrap();

    let py_files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "py").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    for file in &py_files {
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if let Some(cap) = re_decorator.captures(line) {
                let route_path = &cap[2];
                if !paths_match(endpoint, route_path) {
                    continue;
                }

                // Find the 'def' line that follows (skip blank lines and other decorators)
                for j in (i + 1)..lines.len() {
                    let l = lines[j].trim();
                    if l.is_empty() || l.starts_with('@') {
                        continue;
                    }
                    if let Some(c2) = re_func.captures(l) {
                        let module = module_name(file, root);
                        return Some(EntryPoint {
                            file: relative(file, root),
                            line: j + 1,
                            class: module,
                            method: c2[1].to_string(),
                            interface_class: None,
                        });
                    }
                    break;
                }
            }
        }
    }
    None
}

fn paths_match(want: &str, have: &str) -> bool {
    let want_parts: Vec<&str> = want.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    let have_parts: Vec<&str> = have.trim_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    if want_parts.len() > have_parts.len() { return false; }
    if want_parts.len() == have_parts.len() {
        return have_parts.iter().zip(want_parts.iter()).all(|(h, w)| {
            w == h || h.starts_with('{') || w.starts_with('{')
        });
    }
    let offset = have_parts.len() - want_parts.len();
    let slice = &have_parts[offset..];
    slice.iter().zip(want_parts.iter()).all(|(h, w)| {
        w == h || w.starts_with('{')
    })
}

fn relative(file: &str, root: &str) -> String {
    let root = root.trim_end_matches(['/', '\\']);
    file.strip_prefix(root)
        .unwrap_or(file)
        .trim_start_matches(['/', '\\'])
        .replace('\\', "/")
}

/// Derive a module-like name from the file path (e.g. "routers/users.py" → "users").
fn module_name(file: &str, root: &str) -> String {
    let rel = relative(file, root);
    std::path::Path::new(&rel)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| rel.clone())
}

// ---------------------------------------------------------------------------
// Function-mode entry point
// ---------------------------------------------------------------------------

/// Find a method by name inside a specific class within a Python module.
///
/// `modulo` is matched against the file stem (e.g. "users" matches "routers/users.py").
/// `clase` must be a class defined at module level.
pub fn find_function(funcion: &str, clase: &str, modulo: &str, root: &str) -> Option<EntryPoint> {
    let py_files: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "py").unwrap_or(false))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect();

    let re_class  = Regex::new(&format!(r"^class\s+{}\s*[:(]", regex::escape(clase))).unwrap();
    let re_method = Regex::new(&format!(
        r"^\s+(?:async\s+)?def\s+{}\s*\(",
        regex::escape(funcion),
    )).unwrap();

    for file in &py_files {
        if module_name(file, root) != modulo {
            continue;
        }

        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();

        let mut in_class     = false;
        let mut class_indent = 0usize;

        for (i, line) in lines.iter().enumerate() {
            if re_class.is_match(line) {
                in_class     = true;
                class_indent = line.len() - line.trim_start().len();
                continue;
            }
            if in_class {
                // A non-empty line whose indent is ≤ the class definition means we left the class
                if !line.trim().is_empty() {
                    let line_indent = line.len() - line.trim_start().len();
                    if line_indent <= class_indent {
                        in_class = false;
                        continue;
                    }
                }
                if re_method.is_match(line) {
                    return Some(EntryPoint {
                        file: relative(file, root),
                        line: i + 1,
                        class: clase.to_string(),
                        method: funcion.to_string(),
                        interface_class: None,
                    });
                }
            }
        }
    }
    None
}
