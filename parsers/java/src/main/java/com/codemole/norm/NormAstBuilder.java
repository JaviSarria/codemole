package com.codemole.norm;

import com.github.javaparser.ast.NodeList;
import com.github.javaparser.ast.expr.*;
import com.github.javaparser.ast.stmt.*;

import java.util.*;

/**
 * Converts a JavaParser method body into the language-agnostic
 * <em>Normalized AST</em> format consumed by the Rust {@code NStmt} type.
 *
 * <h2>Mapping</h2>
 * <table>
 *   <tr><th>JavaParser AST node</th><th>NStmt kind</th></tr>
 *   <tr><td>{@code IfStmt}</td>         <td>{@code "if"}</td></tr>
 *   <tr><td>{@code ForStmt}</td>         <td>{@code "for"}</td></tr>
 *   <tr><td>{@code ForEachStmt}</td>     <td>{@code "for"}</td></tr>
 *   <tr><td>{@code WhileStmt}</td>       <td>{@code "while"}</td></tr>
 *   <tr><td>{@code DoStmt}</td>          <td>{@code "while"}</td></tr>
 *   <tr><td>{@code MethodCallExpr}</td>  <td>{@code "call"}</td></tr>
 *   <tr><td>{@code ReturnStmt}</td>      <td>{@code "return"}</td></tr>
 *   <tr><td>{@code TryStmt}</td>         <td>{@code "try"}</td></tr>
 *   <tr><td>{@code ThrowStmt}</td>       <td>{@code "throw"}</td></tr>
 *   <tr><td>{@code BreakStmt}</td>       <td>{@code "break"}</td></tr>
 *   <tr><td>{@code ContinueStmt}</td>    <td>{@code "continue"}</td></tr>
 *   <tr><td>{@code BlockStmt}</td>       <td>{@code "block"}</td></tr>
 *   <tr><td>everything else</td>         <td>{@code "expr"}</td></tr>
 * </table>
 *
 * <h2>Output format</h2>
 * Returns a {@code Map<String,Object>} with {@code "kind": "block"} at the top
 * level, matching the {@code NStmt::Block} Rust variant serialized as:
 * <pre>
 * { "kind": "block", "body": [ ... ] }
 * </pre>
 */
public class NormAstBuilder {

    // ── Public entry point ────────────────────────────────────────────────────

    /**
     * Build the normalized body for a method's statement list.
     *
     * @param stmts the top-level statements of the method body
     * @return a {@code Map} representing {@code NStmt::Block}
     */
    public static Map<String, Object> buildBody(NodeList<Statement> stmts) {
        return blockNode(stmts);
    }

    // ── Statement dispatcher ──────────────────────────────────────────────────

    private static Map<String, Object> buildStmt(Statement stmt) {

        if (stmt instanceof IfStmt ifStmt) {
            return buildIf(ifStmt);

        } else if (stmt instanceof ForStmt fs) {
            return buildFor(fs);

        } else if (stmt instanceof ForEachStmt fe) {
            return buildForEach(fe);

        } else if (stmt instanceof WhileStmt ws) {
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind",      "while");
            n.put("condition", ws.getCondition().toString());
            n.put("body",      wrapBlock(ws.getBody()));
            return n;

        } else if (stmt instanceof DoStmt ds) {
            // do-while: same shape as while — condition evaluated at end,
            // but for the normalized AST the distinction is not needed.
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind",      "while");
            n.put("condition", ds.getCondition().toString());
            n.put("body",      wrapBlock(ds.getBody()));
            return n;

        } else if (stmt instanceof TryStmt ts) {
            return buildTry(ts);

        } else if (stmt instanceof ReturnStmt rs) {
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind",  "return");
            n.put("value", rs.getExpression().map(Object::toString).orElse(null));
            return n;

        } else if (stmt instanceof ThrowStmt ts) {
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind",  "throw");
            n.put("value", ts.getExpression().toString());
            return n;

        } else if (stmt instanceof BreakStmt) {
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind", "break");
            return n;

        } else if (stmt instanceof ContinueStmt) {
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind", "continue");
            return n;

        } else if (stmt instanceof BlockStmt bs) {
            return blockNode(bs.getStatements());

        } else if (stmt instanceof ExpressionStmt es) {
            return buildExpr(es.getExpression());

        } else {
            // Catch-all: SwitchStmt, SynchronizedStmt, LabeledStmt, etc.
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind", "expr");
            n.put("text", stmt.toString().trim());
            return n;
        }
    }

    // ── Specialised builders ──────────────────────────────────────────────────

    private static Map<String, Object> buildIf(IfStmt ifStmt) {
        Map<String, Object> n = new LinkedHashMap<>();
        n.put("kind",        "if");
        n.put("condition",   ifStmt.getCondition().toString());
        n.put("then_branch", wrapBlock(ifStmt.getThenStmt()));
        n.put("else_branch", ifStmt.getElseStmt()
                .map(NormAstBuilder::wrapBlock).orElse(null));
        return n;
    }

    private static Map<String, Object> buildFor(ForStmt fs) {
        Map<String, Object> n = new LinkedHashMap<>();
        n.put("kind",      "for");
        n.put("init",      fs.getInitialization().isEmpty()
                ? null : fs.getInitialization().toString());
        n.put("condition", fs.getCompare().map(Object::toString).orElse(null));
        n.put("update",    fs.getUpdate().isEmpty()
                ? null : fs.getUpdate().toString());
        n.put("body",      wrapBlock(fs.getBody()));
        return n;
    }

    private static Map<String, Object> buildForEach(ForEachStmt fe) {
        Map<String, Object> n = new LinkedHashMap<>();
        n.put("kind",      "for");
        n.put("init",      fe.getVariable() + " : " + fe.getIterable());
        n.put("condition", null);
        n.put("update",    null);
        n.put("body",      wrapBlock(fe.getBody()));
        return n;
    }

    private static Map<String, Object> buildTry(TryStmt ts) {
        Map<String, Object> n = new LinkedHashMap<>();
        n.put("kind",          "try");
        n.put("try_block",     blockNode(ts.getTryBlock().getStatements()));

        // Merge all catch clauses into a single block (simplified).
        if (!ts.getCatchClauses().isEmpty()) {
            List<Map<String, Object>> catchStmts = new ArrayList<>();
            for (var clause : ts.getCatchClauses()) {
                catchStmts.addAll(buildStmtList(clause.getBody().getStatements()));
            }
            Map<String, Object> catchBlock = new LinkedHashMap<>();
            catchBlock.put("kind", "block");
            catchBlock.put("body", catchStmts);
            n.put("catch_block", catchBlock);
        } else {
            n.put("catch_block", null);
        }

        n.put("finally_block", ts.getFinallyBlock()
                .map(b -> (Object) blockNode(b.getStatements()))
                .orElse(null));
        return n;
    }

    private static Map<String, Object> buildExpr(Expression expr) {
        if (expr instanceof MethodCallExpr call) {
            Map<String, Object> n = new LinkedHashMap<>();
            n.put("kind",   "call");
            n.put("target", call.getNameAsString());
            return n;
        }
        Map<String, Object> n = new LinkedHashMap<>();
        n.put("kind", "expr");
        n.put("text", expr.toString());
        return n;
    }

    // ── Block helpers ─────────────────────────────────────────────────────────

    /**
     * Wrap a {@link Statement} in a block node.
     * If it is already a {@link BlockStmt}, unwrap it directly.
     */
    private static Map<String, Object> wrapBlock(Statement stmt) {
        if (stmt instanceof BlockStmt bs) {
            return blockNode(bs.getStatements());
        }
        Map<String, Object> single = buildStmt(stmt);
        Map<String, Object> block  = new LinkedHashMap<>();
        block.put("kind", "block");
        block.put("body", single != null ? List.of(single) : List.of());
        return block;
    }

    private static Map<String, Object> blockNode(NodeList<Statement> stmts) {
        Map<String, Object> n = new LinkedHashMap<>();
        n.put("kind", "block");
        n.put("body", buildStmtList(stmts));
        return n;
    }

    private static List<Map<String, Object>> buildStmtList(NodeList<Statement> stmts) {
        List<Map<String, Object>> result = new ArrayList<>(stmts.size());
        for (Statement s : stmts) {
            Map<String, Object> node = buildStmt(s);
            if (node != null) result.add(node);
        }
        return result;
    }
}
