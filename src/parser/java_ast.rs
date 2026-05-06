/// Rust mirror types for the JSON produced by the Java AST parser.
///
/// These structs are used **only** for deserialization — they are immediately
/// converted to the unified [`crate::ir`] model by [`crate::parser::java_ir`].
/// Keeping them separate prevents the IR from leaking Java-specific details.

use serde::Deserialize;

// ── Root ──────────────────────────────────────────────────────────────────────

/// Top-level object written to stdout by `java-parser.jar`.
#[derive(Debug, Deserialize)]
pub struct JavaAstOutput {
    pub source_root:       String,
    pub compilation_units: Vec<JavaCompUnit>,
}

// ── Compilation unit ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaCompUnit {
    pub file:    String,
    pub package: String,
    pub types:   Vec<JavaType>,
}

// ── Type (class / interface / enum) ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaType {
    pub name:           String,
    pub qualified_name: String,
    /// `"Class"` | `"Interface"` | `"Enum"`
    pub kind:           String,
    pub visibility:     String,
    pub annotations:    Vec<String>,
    /// Simple name of the extended class (if any).
    pub extends:        Option<String>,
    /// Simple names of implemented interfaces.
    pub implements:     Vec<String>,
    pub fields:         Vec<JavaField>,
    pub methods:        Vec<JavaMethod>,
}

// ── Field ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaField {
    pub name:        String,
    #[serde(rename = "type")]
    pub field_type:  String,
    pub visibility:  String,
    pub annotations: Vec<String>,
}

// ── Method ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaMethod {
    pub name:        String,
    pub visibility:  String,
    pub return_type: String,
    pub parameters:  Vec<JavaParam>,
    pub annotations: Vec<String>,
    pub line:        usize,
    pub calls:       Vec<JavaCall>,
    pub cfg_blocks:  Vec<JavaCfgBlock>,
}

// ── Parameter ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaParam {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,
}

// ── Call site ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaCall {
    /// Object or field the call is made on (e.g. `"userRepo"`), or `null`.
    pub scope:          Option<String>,
    pub method:         String,
    pub line:           usize,
    /// Fully-qualified owner resolved by the Symbol Solver, or `null`.
    pub resolved_owner: Option<String>,
    /// `"Direct"` | `"Virtual"` | `"External"`
    pub call_type:      String,
}

// ── CFG block ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaCfgBlock {
    pub id:         u64,
    pub statements: Vec<JavaStatement>,
    pub edges:      Vec<JavaCfgEdge>,
}

// ── Statement ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaStatement {
    /// `"Call"` | `"Return"` | `"Assign"` | `"Condition"` | `"Loop"` | `"Throw"`
    pub kind:   String,
    /// Callee name when `kind == "Call"`, otherwise `null`.
    pub method: Option<String>,
    pub line:   usize,
}

// ── CFG edge ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JavaCfgEdge {
    /// Target block id; `-1` is the sentinel for the virtual exit.
    pub to:   i64,
    /// `"Normal"` | `"True"` | `"False"` | `"Loop"` | `"Return"` | `"Exception"`
    pub kind: String,
}
