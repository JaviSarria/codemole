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
