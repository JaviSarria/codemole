//! Minimal Java IR: types, JAR invocation, and fast-lookup index.
//!
//! Invokes the Java AST parser JAR and deserialises enough of its output to
//! reconstruct `if`/`loop` structures for enriched sequence diagrams.

use std::collections::HashMap;
use std::process::Command;
use serde::Deserialize;

use super::norm::NStmt;

// ── IR types (subset of the full Program schema) ──────────────────────────────

#[derive(Deserialize)]
pub struct JavaProgram {
    #[serde(default)]
    pub types:     Vec<JType>,
    #[serde(default)]
    pub functions: Vec<JFunction>,
    #[serde(default)]
    pub calls:     Vec<JCall>,
    #[serde(default)]
    pub cfgs:      Vec<JCfg>,
}

#[derive(Deserialize)]
pub struct JType {
    pub id:   String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct JFunction {
    pub id:            String,
    pub name:          String,
    pub owner_type_id: Option<String>,
    #[serde(default)]
    pub return_type:   Option<String>,
    /// Normalized AST body emitted by `NormAstBuilder` in the Java parser JAR.
    /// `None` when the method has no body (abstract / native) or an older JAR
    /// version is used.
    #[serde(default)]
    pub body_ast:      Option<NStmt>,
}

#[derive(Deserialize)]
pub struct JCall {
    pub id:        String,
    pub callee_id: String,
}

#[derive(Deserialize)]
pub struct JCfg {
    pub function_id: String,
    pub entry_block: String,
    pub exit_block:  String,
    pub blocks:      Vec<JBlock>,
}

#[derive(Deserialize)]
pub struct JBlock {
    pub id:         String,
    pub statements: Vec<JStatement>,
    pub edges:      Vec<JEdge>,
}

#[derive(Deserialize)]
pub struct JStatement {
    pub kind:       String,
    #[serde(default)]
    pub call_id:    Option<String>,
    #[serde(default)]
    pub expression: Option<String>,
}

#[derive(Deserialize)]
pub struct JEdge {
    pub to: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

// ── JAR invocation ─────────────────────────────────────────────────────────────

/// Invoke the Java AST parser JAR for `source_root`.
/// Returns `None` on any error so callers can fall back to the flat renderer.
pub fn invoke_java_parser(source_root: &str, jar_path: &str) -> Option<JavaProgram> {
    let out = Command::new("java")
        .arg("-jar")
        .arg(jar_path)
        .arg(source_root)
        .output()
        .map_err(|e| eprintln!("[java-parser] could not launch: {e}"))
        .ok()?;

    if !out.status.success() {
        eprintln!("[java-parser] {}", String::from_utf8_lossy(&out.stderr));
        return None;
    }

    let json = String::from_utf8(out.stdout)
        .map_err(|e| eprintln!("[java-parser] non-UTF8 output: {e}"))
        .ok()?;

    serde_json::from_str(&json)
        .map_err(|e| eprintln!("[java-parser] JSON parse error: {e}"))
        .ok()
}

// ── Fast-lookup index ─────────────────────────────────────────────────────────

/// Pre-built index over a `JavaProgram` for O(1) look-ups.
pub struct JavaIndex {
    /// function_id → (class_name, method_name)
    func_info:   HashMap<String, (String, String)>,
    /// function_id → return_type label (falls back to method name when None)
    func_return: HashMap<String, Option<String>>,
    /// call_id → callee_function_id
    call_callee: HashMap<String, String>,
    /// (class_name, method_name) → function_id
    func_by_key: HashMap<(String, String), String>,
    /// function_id → CFG
    pub cfgs: HashMap<String, JCfg>,
    /// function_id → normalized AST body
    #[allow(dead_code)]
    pub bodies: HashMap<String, NStmt>,
}

impl JavaIndex {
    pub fn build(prog: JavaProgram) -> Self {
        let type_names: HashMap<String, String> = prog.types.into_iter()
            .map(|t| (t.id, t.name))
            .collect();

        let mut func_info:   HashMap<String, (String, String)>  = HashMap::new();
        let mut func_return: HashMap<String, Option<String>>     = HashMap::new();
        let mut func_by_key: HashMap<(String, String), String>   = HashMap::new();
        let mut bodies:      HashMap<String, NStmt>              = HashMap::new();

        for f in prog.functions {
            let class = f.owner_type_id.as_deref()
                .and_then(|tid| type_names.get(tid))
                .cloned()
                .unwrap_or_default();
            func_info.insert(f.id.clone(), (class.clone(), f.name.clone()));
            func_return.insert(f.id.clone(), f.return_type.clone());
            func_by_key.insert((class, f.name.clone()), f.id.clone());
            if let Some(body) = f.body_ast {
                bodies.insert(f.id.clone(), body);
            }
        }

        let call_callee: HashMap<String, String> = prog.calls.into_iter()
            .map(|c| (c.id, c.callee_id))
            .collect();

        let cfgs: HashMap<String, JCfg> = prog.cfgs.into_iter()
            .map(|c| (c.function_id.clone(), c))
            .collect();

        JavaIndex { func_info, func_return, call_callee, func_by_key, cfgs, bodies }
    }

    /// Returns the CFG for `(class, method)`, if available.
    pub fn get_cfg(&self, class: &str, method: &str) -> Option<&JCfg> {
        let fid = self.func_by_key.get(&(class.to_string(), method.to_string()))?;
        self.cfgs.get(fid)
    }

    /// Returns the normalized AST body for `(class, method)`, if available.
    #[allow(dead_code)]
    pub fn get_body(&self, class: &str, method: &str) -> Option<&NStmt> {
        let fid = self.func_by_key.get(&(class.to_string(), method.to_string()))?;
        self.bodies.get(fid)
    }

    /// Returns `(callee_class, callee_method)` for a `call_id`.
    pub fn callee_of(&self, call_id: &str) -> Option<(&str, &str)> {
        let callee_fid = self.call_callee.get(call_id)?;
        let (class, method) = self.func_info.get(callee_fid)?;
        Some((class.as_str(), method.as_str()))
    }

    /// Returns the return-type label for `(callee_class, callee_method)`.
    /// Falls back to the method name when the return type is void or unknown.
    pub fn return_label_of(&self, callee_class: &str, callee_method: &str) -> String {
        self.func_by_key
            .get(&(callee_class.to_string(), callee_method.to_string()))
            .and_then(|fid| self.func_return.get(fid))
            .and_then(|r| r.as_deref())
            .unwrap_or(callee_method)
            .to_string()
    }
}
