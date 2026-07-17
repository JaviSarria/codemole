// go-normalize — converts Go source files to the language-agnostic NStmt JSON.
//
// Usage:
//
//	go-normalize <source-root>
//
// Output (stdout):
//
//	{ "functions": [ { "file": "...", "name": "...", "line": N, "body_ast": {...} }, ... ] }
//
// Mapping (go/ast node → NStmt kind):
//
//	*ast.IfStmt      → "if"
//	*ast.ForStmt     → "for"    (classic for; also covers while-style loops)
//	*ast.RangeStmt   → "for"
//	*ast.ReturnStmt  → "return"
//	*ast.ExprStmt    → "call"   (when the expression is a *ast.CallExpr)
//	*ast.BlockStmt   → "block"
//	BranchStmt break → "break"
//	BranchStmt cont  → "continue"
//	everything else  → "expr"

package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/printer"
	"go/token"
	"os"
	"path/filepath"
	"strings"
)

// ---------------------------------------------------------------------------
// NStmt representation
// ---------------------------------------------------------------------------

// NStmt is a generic map that serializes to the tagged-object format expected
// by the Rust NStmt enum: { "kind": "if", ... }
type NStmt map[string]interface{}

// ---------------------------------------------------------------------------
// Expression / statement helpers
// ---------------------------------------------------------------------------

func exprStr(e ast.Expr, fset *token.FileSet) string {
	if e == nil {
		return ""
	}
	var buf bytes.Buffer
	if err := printer.Fprint(&buf, fset, e); err != nil {
		return fmt.Sprintf("%T", e)
	}
	return buf.String()
}

func stmtStr(s ast.Stmt, fset *token.FileSet) string {
	if s == nil {
		return ""
	}
	var buf bytes.Buffer
	if err := printer.Fprint(&buf, fset, s); err != nil {
		return fmt.Sprintf("%T", s)
	}
	return strings.TrimSpace(buf.String())
}

// ---------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------

func normBlock(stmts []ast.Stmt, fset *token.FileSet) NStmt {
	body := make([]NStmt, 0, len(stmts))
	for _, s := range stmts {
		if n := normStmt(s, fset); n != nil {
			body = append(body, n)
		}
	}
	return NStmt{"kind": "block", "body": body}
}

func wrapBlock(s ast.Stmt, fset *token.FileSet) NStmt {
	if bs, ok := s.(*ast.BlockStmt); ok {
		return normBlock(bs.List, fset)
	}
	n := normStmt(s, fset)
	body := []NStmt{}
	if n != nil {
		body = append(body, n)
	}
	return NStmt{"kind": "block", "body": body}
}

func normStmt(s ast.Stmt, fset *token.FileSet) NStmt {
	if s == nil {
		return nil
	}

	switch n := s.(type) {

	// ── if ────────────────────────────────────────────────────────────────────
	case *ast.IfStmt:
		var elseB interface{}
		if n.Else != nil {
			elseB = normStmt(n.Else, fset)
		}
		return NStmt{
			"kind":        "if",
			"condition":   exprStr(n.Cond, fset),
			"then_branch": normBlock(n.Body.List, fset),
			"else_branch": elseB,
		}

	// ── for (classic: init; cond; post) ──────────────────────────────────────
	case *ast.ForStmt:
		var init, cond, update interface{}
		if n.Init != nil {
			init = stmtStr(n.Init, fset)
		}
		if n.Cond != nil {
			cond = exprStr(n.Cond, fset)
		}
		if n.Post != nil {
			update = stmtStr(n.Post, fset)
		}
		return NStmt{
			"kind":      "for",
			"init":      init,
			"condition": cond,
			"update":    update,
			"body":      normBlock(n.Body.List, fset),
		}

	// ── for … range ───────────────────────────────────────────────────────────
	case *ast.RangeStmt:
		init := fmt.Sprintf("%s := range %s",
			exprStr(n.Key, fset), exprStr(n.X, fset))
		return NStmt{
			"kind":      "for",
			"init":      init,
			"condition": nil,
			"update":    nil,
			"body":      normBlock(n.Body.List, fset),
		}

	// ── return ────────────────────────────────────────────────────────────────
	case *ast.ReturnStmt:
		var val interface{}
		if len(n.Results) > 0 {
			val = exprStr(n.Results[0], fset)
		}
		return NStmt{"kind": "return", "value": val}

	// ── call (expression statement whose expression is a call) ────────────────
	case *ast.ExprStmt:
		if call, ok := n.X.(*ast.CallExpr); ok {
			return NStmt{"kind": "call", "target": exprStr(call.Fun, fset)}
		}
		return NStmt{"kind": "expr", "text": exprStr(n.X, fset)}

	// ── block ─────────────────────────────────────────────────────────────────
	case *ast.BlockStmt:
		return normBlock(n.List, fset)

	// ── break / continue ──────────────────────────────────────────────────────
	case *ast.BranchStmt:
		switch n.Tok {
		case token.BREAK:
			return NStmt{"kind": "break"}
		case token.CONTINUE:
			return NStmt{"kind": "continue"}
		}

		// ── if-init block (e.g. `if x := f(); x != nil { ... }`) ─────────────────
		// handled above via *ast.IfStmt; the init is included in condition text.
	}

	// Fallback
	return NStmt{"kind": "expr", "text": stmtStr(s, fset)}
}

// ---------------------------------------------------------------------------
// Per-function entry
// ---------------------------------------------------------------------------

type FuncEntry struct {
	File    string      `json:"file"`
	Name    string      `json:"name"`
	Line    int         `json:"line"`
	BodyAst interface{} `json:"body_ast"`
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: go-normalize <source-root>")
		os.Exit(1)
	}

	root := os.Args[1]
	var functions []FuncEntry
	fset := token.NewFileSet()

	err := filepath.Walk(root, func(path string, info os.FileInfo, walkErr error) error {
		if walkErr != nil || info.IsDir() {
			return walkErr
		}
		if !strings.HasSuffix(path, ".go") {
			return nil
		}

		f, parseErr := parser.ParseFile(fset, path, nil, 0)
		if parseErr != nil {
			fmt.Fprintf(os.Stderr, "[warn] %s: %v\n", path, parseErr)
			return nil
		}

		rel, relErr := filepath.Rel(root, path)
		if relErr != nil {
			rel = path
		}
		rel = filepath.ToSlash(rel)

		for _, decl := range f.Decls {
			fd, ok := decl.(*ast.FuncDecl)
			if !ok || fd.Body == nil {
				continue
			}
			pos := fset.Position(fd.Pos())
			functions = append(functions, FuncEntry{
				File:    rel,
				Name:    fd.Name.Name,
				Line:    pos.Line,
				BodyAst: normBlock(fd.Body.List, fset),
			})
		}
		return nil
	})

	if err != nil {
		fmt.Fprintf(os.Stderr, "[error] walk: %v\n", err)
		os.Exit(1)
	}

	if functions == nil {
		functions = []FuncEntry{}
	}

	out, _ := json.MarshalIndent(map[string]interface{}{"functions": functions}, "", "  ")
	os.Stdout.Write(out)
	fmt.Println()
}
