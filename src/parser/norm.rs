//! Normalized AST — language-agnostic statement tree.
//!
//! Every language-specific parser converts its raw AST to this form.
//! The normalized representation sits between the raw AST (step 1) and the
//! IR / CFG stages (steps 3–4), providing a uniform structure for all
//! languages before further analysis.
//!
//! # Mapping tables
//!
//! ## Java (JavaParser)
//! | AST node         | [`NStmt`] variant |
//! |------------------|-------------------|
//! | `IfStmt`         | `If`              |
//! | `ForStmt`        | `For`             |
//! | `ForEachStmt`    | `For`             |
//! | `WhileStmt`      | `While`           |
//! | `DoStmt`         | `While`           |
//! | `MethodCallExpr` | `Call`            |
//! | `ReturnStmt`     | `Return`          |
//! | `TryStmt`        | `Try`             |
//! | `ThrowStmt`      | `Throw`           |
//! | `BreakStmt`      | `Break`           |
//! | `ContinueStmt`   | `Continue`        |
//! | `BlockStmt`      | `Block`           |
//!
//! ## Python (ast module)
//! | AST node      | [`NStmt`] variant |
//! |---------------|-------------------|
//! | `ast.If`      | `If`              |
//! | `ast.For`     | `For`             |
//! | `ast.While`   | `While`           |
//! | `ast.Call`    | `Call`            |
//! | `ast.Return`  | `Return`          |
//! | `ast.Try`     | `Try`             |
//! | `ast.Raise`   | `Throw`           |
//! | `ast.Break`   | `Break`           |
//! | `ast.Continue`| `Continue`        |
//!
//! ## Go (go/ast)
//! | AST node      | [`NStmt`] variant |
//! |---------------|-------------------|
//! | `IfStmt`      | `If`              |
//! | `ForStmt`     | `For`             |
//! | `RangeStmt`   | `For`             |
//! | `CallExpr`    | `Call`            |
//! | `ReturnStmt`  | `Return`          |
//! | `BlockStmt`   | `Block`           |
//! | `BranchStmt`  | `Break`/`Continue`|

// Infrastructure module — types and invocation helpers will be used once
// the CFG and diagram stages are wired to the normalized AST.
#![allow(dead_code)]

use std::process::Command;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single normalized statement node.
///
/// Serialized as a tagged JSON object: `{ "kind": "if", ... }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NStmt {
    /// A sequential block of statements.
    Block { body: Vec<NStmt> },

    /// Generic expression — catch-all for nodes that don't fit other variants.
    Expr { text: String },

    /// A function / method call.
    Call { target: String },

    /// `if` / `else if` / `else`.
    If {
        condition:   String,
        then_branch: Box<NStmt>,
        else_branch: Option<Box<NStmt>>,
    },

    /// `while` loop (also covers `do … while`).
    While {
        condition: String,
        body:      Box<NStmt>,
    },

    /// `for` loop (classic C-style, `for-each`, `for-range`).
    For {
        /// Initializer expression or `target in iterable` (for-each / range).
        init:      Option<String>,
        condition: Option<String>,
        update:    Option<String>,
        body:      Box<NStmt>,
    },

    /// `return` statement.
    Return { value: Option<String> },

    /// `break` statement.
    Break,

    /// `continue` statement.
    Continue,

    /// `try / catch / finally` (or `defer/recover` in Go).
    Try {
        try_block:     Box<NStmt>,
        catch_block:   Option<Box<NStmt>>,
        finally_block: Option<Box<NStmt>>,
    },

    /// `throw` / `raise` statement.
    Throw { value: Option<String> },
}

/// Normalized body of a single function — a top-level block.
pub type NBody = NStmt;

// ---------------------------------------------------------------------------
// Per-function record produced by the external normalizers
// ---------------------------------------------------------------------------

/// A single function entry returned by a language normalizer.
#[derive(Debug, Deserialize)]
pub struct NormFunction {
    pub file:     String,
    pub name:     String,
    pub line:     usize,
    pub body_ast: NBody,
}

/// Container returned by Python / Go normalizer scripts.
#[derive(Debug, Deserialize)]
struct NormOutput {
    functions: Vec<NormFunction>,
}

// ---------------------------------------------------------------------------
// Python normalizer invocation
// ---------------------------------------------------------------------------

/// Invoke `parsers/python/normalize.py` on `source_root`.
///
/// Returns `None` on any error so callers can fall back gracefully.
/// The script is located relative to the JAR path prefix provided, or via
/// `CODEMOLE_PYTHON_NORM` env var.
pub fn normalize_python(source_root: &str, script_path: &str) -> Option<Vec<NormFunction>> {
    let out = Command::new("python3")
        .arg(script_path)
        .arg(source_root)
        .output()
        .or_else(|_| {
            // Fallback: try `python` on systems where python3 is not in PATH
            Command::new("python")
                .arg(script_path)
                .arg(source_root)
                .output()
        })
        .map_err(|e| eprintln!("[python-norm] could not launch: {e}"))
        .ok()?;

    if !out.status.success() {
        eprintln!("[python-norm] {}", String::from_utf8_lossy(&out.stderr));
        return None;
    }

    let json = String::from_utf8(out.stdout)
        .map_err(|e| eprintln!("[python-norm] non-UTF8 output: {e}"))
        .ok()?;

    serde_json::from_str::<NormOutput>(&json)
        .map(|o| o.functions)
        .map_err(|e| eprintln!("[python-norm] JSON parse error: {e}"))
        .ok()
}

// ---------------------------------------------------------------------------
// Go normalizer invocation
// ---------------------------------------------------------------------------

/// Invoke the `go-normalize` binary on `source_root`.
///
/// Returns `None` on any error so callers can fall back gracefully.
pub fn normalize_go(source_root: &str, binary_path: &str) -> Option<Vec<NormFunction>> {
    let out = Command::new(binary_path)
        .arg(source_root)
        .output()
        .map_err(|e| eprintln!("[go-norm] could not launch: {e}"))
        .ok()?;

    if !out.status.success() {
        eprintln!("[go-norm] {}", String::from_utf8_lossy(&out.stderr));
        return None;
    }

    let json = String::from_utf8(out.stdout)
        .map_err(|e| eprintln!("[go-norm] non-UTF8 output: {e}"))
        .ok()?;

    serde_json::from_str::<NormOutput>(&json)
        .map(|o| o.functions)
        .map_err(|e| eprintln!("[go-norm] JSON parse error: {e}"))
        .ok()
}
