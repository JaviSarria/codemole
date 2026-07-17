/// Language-specific analysis strategies.
///
/// # Extensibility
///
/// To add support for a new language, implement [`LanguageAnalyzer`] and register it
/// in [`crate::parser::build_call_graph`].  No changes are needed to the core BFS
/// traversal logic.
///
/// # Design
///
/// Each language is modelled as a set of *capabilities* that the generic traversal
/// engine queries:
///
/// | Method              | Purpose                                                  |
/// |---------------------|----------------------------------------------------------|
/// `file_ext`            | Filter source files by extension                        |
/// `is_oop`              | Enable qualified-call resolution (Java / Python)        |
/// `def_pattern`         | Locate function/method definitions                      |
/// `call_pattern`        | Extract call sites from a body line                     |
/// `class_from_line`     | Update the "current class" context while scanning       |
/// `return_type`         | Parse return type from a definition line                |
/// `skip_symbols`        | Identifiers that must not be followed (stdlib, …)       |
///
/// The engine handles BFS, deduplication, scope resolution and graph construction.
use regex::Regex;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

pub trait LanguageAnalyzer: Send + Sync {
    /// Source file extension (without the leading dot), e.g. `"java"`.
    fn file_ext(&self) -> &str;

    /// Whether the language uses OOP qualified-call semantics (`obj.method()`).
    /// When `true` the engine performs field-type resolution across class boundaries.
    fn is_oop(&self) -> bool;

    /// Regex that matches a *function / method definition* line.
    /// **Capture group 1** must contain the function name.
    fn def_pattern(&self) -> &Regex;

    /// Regex that matches a *call site* within a body line.
    /// **Capture group 1** must contain the callee name.
    fn call_pattern(&self) -> &Regex;

    /// Given a source line and the current accumulated class name, return the
    /// updated class name (or `None` to keep the existing one).
    ///
    /// The engine calls this for every line during the definition-index pass.
    /// For Go, this reads the receiver type from `func (r *T)` headers.
    /// For Java/Python it looks for `class Foo` declarations.
    fn update_class_context<'a>(
        &self,
        line: &'a str,
        rel_file: &str,
        current_class: &str,
    ) -> Option<String>;

    /// Extract the declared return type from a method definition line.
    /// Returns an empty string when the type is `void`, unknown, or not annotated.
    fn return_type(&self, def_line: &str, method_name: &str) -> String;

    /// Set of symbol names that should never be followed during traversal.
    fn skip_symbols(&self) -> &HashSet<String>;

    /// Whether a callee name should be skipped before looking it up in the index.
    /// Default: skip symbols from `skip_symbols()` and, for OOP, skip
    /// uppercase-first names (constructors).
    fn should_skip(&self, callee: &str) -> bool {
        if self.skip_symbols().contains(callee) {
            return true;
        }
        if self.is_oop() {
            // Skip constructor calls (Class-name patterns)
            if callee.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

pub struct JavaAnalyzer {
    re_def:  Regex,
    re_call: Regex,
    re_class: Regex,
    skip: HashSet<String>,
}

impl JavaAnalyzer {
    pub fn new(skip: HashSet<String>) -> Self {
        Self {
            // Stricter regex: requires explicit access modifier (public/protected/private)
            // but flexible on return type to handle generics and complex types.
            // Prevents matching method calls inside strings like logger.info("crudTicket()...")
            // because those won't have public/protected/private before them.
            re_def: Regex::new(
                r"(?:public|protected|private)\s+(?:static\s+)?(?:final\s+)?(?:synchronized\s+)?(?:abstract\s+)?(?:native\s+)?.*?(\w+)\s*\(",
            ).unwrap(),
            re_call:  Regex::new(r"\b(\w+)\s*\(").unwrap(),
            re_class: Regex::new(r"(?:^|\s)(class|interface|enum)\s+(\w+)").unwrap(),
            skip,
        }
    }
}

impl LanguageAnalyzer for JavaAnalyzer {
    fn file_ext(&self) -> &str { "java" }
    fn is_oop(&self)   -> bool { true }
    fn def_pattern(&self)  -> &Regex { &self.re_def  }
    fn call_pattern(&self) -> &Regex { &self.re_call }
    fn skip_symbols(&self) -> &HashSet<String> { &self.skip }

    fn update_class_context<'a>(
        &self,
        line: &'a str,
        _rel_file: &str,
        _current_class: &str,
    ) -> Option<String> {
        self.re_class.captures(line).map(|cap| cap[2].to_string())
    }

    fn return_type(&self, def_line: &str, method_name: &str) -> String {
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
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

pub struct PythonAnalyzer {
    re_def:   Regex,
    re_call:  Regex,
    re_class: Regex,
    skip: HashSet<String>,
}

impl PythonAnalyzer {
    pub fn new(skip: HashSet<String>) -> Self {
        Self {
            re_def:   Regex::new(r"^(?:async\s+)?def\s+(\w+)\s*\(").unwrap(),
            re_call:  Regex::new(r"\b(\w+)\s*\(").unwrap(),
            re_class: Regex::new(r"^class\s+(\w+)").unwrap(),
            skip,
        }
    }
}

impl LanguageAnalyzer for PythonAnalyzer {
    fn file_ext(&self) -> &str { "py" }
    fn is_oop(&self)   -> bool { true }
    fn def_pattern(&self)  -> &Regex { &self.re_def  }
    fn call_pattern(&self) -> &Regex { &self.re_call }
    fn skip_symbols(&self) -> &HashSet<String> { &self.skip }

    fn update_class_context<'a>(
        &self,
        line: &'a str,
        _rel_file: &str,
        _current_class: &str,
    ) -> Option<String> {
        self.re_class.captures(line).map(|cap| cap[1].to_string())
    }

    fn return_type(&self, def_line: &str, _method_name: &str) -> String {
        // PEP 3107: def foo(...) -> ReturnType:
        if let Some(arrow) = def_line.find("->") {
            let after = def_line[arrow + 2..].trim();
            let t = after.trim_end_matches(':').trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

pub struct GoAnalyzer {
    re_def:      Regex,
    re_call:     Regex,
    re_recv:     Regex,  // func (r *T) Name(
    skip: HashSet<String>,
}

impl GoAnalyzer {
    pub fn new(skip: HashSet<String>) -> Self {
        Self {
            re_def:  Regex::new(r"^func\s+(?:\([^)]+\)\s+)?(\w+)\s*\(").unwrap(),
            re_call: Regex::new(r"\b(\w+)\s*\(").unwrap(),
            re_recv: Regex::new(r"^func\s+\([^)]*\*?(\w+)\s*\)").unwrap(),
            skip,
        }
    }
}

impl LanguageAnalyzer for GoAnalyzer {
    fn file_ext(&self) -> &str { "go" }
    fn is_oop(&self)   -> bool { false }
    fn def_pattern(&self)  -> &Regex { &self.re_def  }
    fn call_pattern(&self) -> &Regex { &self.re_call }
    fn skip_symbols(&self) -> &HashSet<String> { &self.skip }

    fn update_class_context<'a>(
        &self,
        line: &'a str,
        rel_file: &str,
        _current_class: &str,
    ) -> Option<String> {
        if let Some(cap) = self.re_recv.captures(line) {
            // Method of a named struct receiver
            Some(cap[1].to_string())
        } else if self.re_def.is_match(line) {
            // Package-level function — use directory name as the "class"
            Some(go_pkg_name(rel_file))
        } else {
            None
        }
    }

    fn return_type(&self, def_line: &str, _method_name: &str) -> String {
        // func (r *Recv) Name(args) ReturnType {
        // func (r *Recv) Name(args) (T1, T2) {
        let line = def_line.trim_end_matches('{').trim();
        if let Some(pos) = line.rfind(')') {
            let ret = line[pos + 1..].trim().to_string();
            if !ret.is_empty() {
                return ret;
            }
        }
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derive a Go package name from a relative file path (last directory component).
pub fn go_pkg_name(rel_file: &str) -> String {
    std::path::Path::new(rel_file)
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "main".to_string())
}
