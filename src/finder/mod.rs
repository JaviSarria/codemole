mod spring;
mod fastapi;
mod gin;

/// Location of the endpoint handler found in the codebase.
#[derive(Debug, Clone)]
pub struct EntryPoint {
    /// Source file path (relative to scan root)
    pub file: String,
    /// 1-based line number where the handler function/method starts
    pub line: usize,
    /// Class name (or module name for Python/Go)
    pub class: String,
    /// Method/function name
    pub method: String,
    /// Set to the interface name when the entry point is a concrete implementation
    pub interface_class: Option<String>,
}

/// Find the handler for `endpoint` in `root_path` using the rules for `lang`.
/// Returns `None` when no matching handler is found.
pub fn find_endpoint(lang: &str, endpoint: &str, root_path: &str) -> Option<EntryPoint> {
    match lang {
        "java" => spring::find(endpoint, root_path),
        "python" => fastapi::find(endpoint, root_path),
        "go" => gin::find(endpoint, root_path),
        _ => None,
    }
}

/// Find a function/method by name in the given class and scope (package/module).
///
/// - `funcion`: function or method name to search for
/// - `clase`: class name (required for java and python; `None` for go)
/// - `scope`: package for java, module for python, module/package for go
/// - `root_path`: root directory of the source code
pub fn find_function(
    lang: &str,
    funcion: &str,
    clase: Option<&str>,
    scope: Option<&str>,
    root_path: &str,
) -> Option<EntryPoint> {
    match lang {
        "java" => spring::find_function(funcion, clase.unwrap_or(""), scope.unwrap_or(""), root_path),
        "python" => fastapi::find_function(funcion, clase.unwrap_or(""), scope.unwrap_or(""), root_path),
        "go" => gin::find_function(funcion, scope.unwrap_or(""), root_path),
        _ => None,
    }
}
