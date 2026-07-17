#!/usr/bin/env python3
"""
Normalizes Python source files to the language-agnostic NStmt JSON format.

Usage:
    python normalize.py <source-root>

Output (stdout):
    {
      "functions": [
        { "file": "rel/path.py", "name": "fn_name", "line": 10, "body_ast": { ... } },
        ...
      ]
    }

Mapping (ast node → NStmt kind):
    ast.If          → "if"
    ast.For         → "for"
    ast.While       → "while"
    ast.Call        → "call"    (when the whole expression is a call)
    ast.Return      → "return"
    ast.Try         → "try"
    ast.Raise       → "throw"
    ast.Break       → "break"
    ast.Continue    → "continue"
    ast.FunctionDef → "block"   (body is normalized recursively)
    everything else → "expr"
"""

import ast
import json
import sys
from pathlib import Path


# ---------------------------------------------------------------------------
# Expression helpers
# ---------------------------------------------------------------------------

def _unparse(node) -> str:
    """Return a compact source representation of an AST expression/statement."""
    try:
        return ast.unparse(node)
    except Exception:
        return type(node).__name__


# ---------------------------------------------------------------------------
# Statement normalization
# ---------------------------------------------------------------------------

def norm_stmts(stmts: list) -> dict:
    """Wrap a list of statements in an NStmt::Block node."""
    return {"kind": "block", "body": [norm_stmt(s) for s in stmts]}


def norm_stmt(node) -> dict:
    """Convert a single Python AST statement node to an NStmt dict."""

    # ── if ────────────────────────────────────────────────────────────────────
    if isinstance(node, ast.If):
        else_branch = norm_stmts(node.orelse) if node.orelse else None
        return {
            "kind":        "if",
            "condition":   _unparse(node.test),
            "then_branch": norm_stmts(node.body),
            "else_branch": else_branch,
        }

    # ── for ───────────────────────────────────────────────────────────────────
    if isinstance(node, ast.For):
        return {
            "kind":      "for",
            "init":      f"{_unparse(node.target)} in {_unparse(node.iter)}",
            "condition": None,
            "update":    None,
            "body":      norm_stmts(node.body),
        }

    # ── while ─────────────────────────────────────────────────────────────────
    if isinstance(node, ast.While):
        return {
            "kind":      "while",
            "condition": _unparse(node.test),
            "body":      norm_stmts(node.body),
        }

    # ── return ────────────────────────────────────────────────────────────────
    if isinstance(node, ast.Return):
        return {
            "kind":  "return",
            "value": _unparse(node.value) if node.value else None,
        }

    # ── call (bare expression whose value is a Call) ──────────────────────────
    if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
        return {
            "kind":   "call",
            "target": _unparse(node.value.func),
        }

    # ── try (Python 3.11+ splits Try into TryStar; handle both) ──────────────
    if isinstance(node, (ast.Try,)) or (
        hasattr(ast, "TryStar") and isinstance(node, ast.TryStar)
    ):
        catch_stmts = []
        for handler in node.handlers:
            catch_stmts.extend(handler.body)

        finally_body = getattr(node, "finalbody", None)
        return {
            "kind":          "try",
            "try_block":     norm_stmts(node.body),
            "catch_block":   norm_stmts(catch_stmts) if catch_stmts else None,
            "finally_block": norm_stmts(finally_body) if finally_body else None,
        }

    # ── raise ─────────────────────────────────────────────────────────────────
    if isinstance(node, ast.Raise):
        return {
            "kind":  "throw",
            "value": _unparse(node.exc) if node.exc else None,
        }

    # ── break / continue ──────────────────────────────────────────────────────
    if isinstance(node, ast.Break):
        return {"kind": "break"}

    if isinstance(node, ast.Continue):
        return {"kind": "continue"}

    # ── nested function / async function — recurse into body ──────────────────
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
        return norm_stmts(node.body)

    # ── fallback ──────────────────────────────────────────────────────────────
    return {"kind": "expr", "text": _unparse(node)}


# ---------------------------------------------------------------------------
# File-level collection
# ---------------------------------------------------------------------------

def collect_functions(tree: ast.AST, rel_path: str) -> list:
    """Return one entry per function/method found anywhere in the module."""
    results = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            results.append({
                "file":     rel_path,
                "name":     node.name,
                "line":     node.lineno,
                "body_ast": norm_stmts(node.body),
            })
    return results


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    if len(sys.argv) < 2:
        print("Usage: normalize.py <source-root>", file=sys.stderr)
        sys.exit(1)

    root = Path(sys.argv[1]).resolve()
    all_functions: list = []

    for path in sorted(root.rglob("*.py")):
        try:
            source = path.read_text(encoding="utf-8")
            tree   = ast.parse(source, filename=str(path))
            rel    = path.relative_to(root).as_posix()
            all_functions.extend(collect_functions(tree, rel))
        except Exception as exc:
            print(f"[warn] {path}: {exc}", file=sys.stderr)

    print(json.dumps({"functions": all_functions}, indent=2))


if __name__ == "__main__":
    main()
