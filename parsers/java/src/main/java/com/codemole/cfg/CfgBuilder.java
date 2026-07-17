package com.codemole.cfg;

import com.github.javaparser.ast.NodeList;
import com.github.javaparser.ast.expr.*;
import com.github.javaparser.ast.stmt.*;

import java.util.*;

/**
 * Builds a Control-Flow Graph (CFG) for a single Java method body.
 *
 * <h2>Output format</h2>
 * Returns a {@code Map<String, Object>} matching the {@code Cfg} JSON schema:
 * <pre>
 * {
 *   "function_id": "f1",
 *   "entry_block": "b1",
 *   "exit_block":  "bN",
 *   "blocks": [
 *     {
 *       "id":         "b1",
 *       "statements": [ {"kind": "call", "call_id": "c3"}, ... ],
 *       "edges":      [ {"to": "b2", "type": "true"}, ... ]
 *     }, ...
 *   ]
 * }
 * </pre>
 *
 * <h2>Call-ID resolution</h2>
 * The caller passes an {@link IdentityHashMap}{@code <MethodCallExpr, String>}
 * built during call registration.  Because both this builder and the call
 * extractor operate on the same AST nodes (by identity), each
 * {@code MethodCallExpr} encountered here is resolved to its global call ID
 * without any line-number heuristics.
 *
 * <h2>Block IDs</h2>
 * IDs are local to the CFG ({@code "b1"}, {@code "b2"}, …) and unique within it.
 * The exit block is pre-allocated and referenced by {@code "return"} /
 * {@code "exception"} edges.
 */
public class CfgBuilder {

    private final String                         funcId;
    /** AST node identity → global call ID (populated by the caller). */
    private final Map<MethodCallExpr, String>    callIdMap;

    private int  blockCtr = 0;
    private final List<Map<String, Object>>          blocks   = new ArrayList<>();
    private final Map<String, Map<String, Object>>   blockMap = new LinkedHashMap<>();

    public CfgBuilder(String funcId, Map<MethodCallExpr, String> callIdMap) {
        this.funcId    = funcId;
        this.callIdMap = callIdMap;
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /**
     * Builds the CFG and returns the complete CFG map, or {@code null} when
     * the method body is empty.
     */
    public Map<String, Object> build(NodeList<Statement> stmts) {
        if (stmts.isEmpty()) return null;

        String entryId = newBlock();
        String exitId  = newBlock();   // pre-allocated exit

        String last = processStatements(stmts, entryId, exitId);
        if (last != null) addEdge(last, exitId, "normal");

        Map<String, Object> cfg = new LinkedHashMap<>();
        cfg.put("function_id",  funcId);
        cfg.put("entry_block",  entryId);
        cfg.put("exit_block",   exitId);
        cfg.put("blocks",       blocks);
        return cfg;
    }

    // ── Block management ──────────────────────────────────────────────────────

    private String newBlock() {
        String id = "b" + (++blockCtr);
        Map<String, Object> b = new LinkedHashMap<>();
        b.put("id",         id);
        b.put("statements", new ArrayList<Map<String, Object>>());
        b.put("edges",      new ArrayList<Map<String, Object>>());
        blocks.add(b);
        blockMap.put(id, b);
        return id;
    }

    @SuppressWarnings("unchecked")
    private void addStatement(String blockId, Map<String, Object> stmt) {
        ((List<Map<String, Object>>) blockMap.get(blockId).get("statements")).add(stmt);
    }

    @SuppressWarnings("unchecked")
    private void addEdge(String from, String to, String type) {
        Map<String, Object> e = new LinkedHashMap<>();
        e.put("to",   to);
        e.put("type", type);
        ((List<Map<String, Object>>) blockMap.get(from).get("edges")).add(e);
    }

    // ── Statement processing ──────────────────────────────────────────────────

    /**
     * Processes a list of statements inside {@code currentBlock}.
     *
     * @return the ID of the last live block, or {@code null} if all paths ended
     *         in a {@code return} / {@code throw}.
     */
    private String processStatements(NodeList<Statement> stmtList,
                                     String currentBlock, String exitId) {
        String block = currentBlock;
        for (Statement stmt : stmtList) {
            if (block == null) break;
            block = processStatement(stmt, block, exitId);
        }
        return block;
    }

    private String processStatement(Statement stmt, String block, String exitId) {

        if (stmt instanceof ExpressionStmt es) {
            return processExpression(es.getExpression(), block);

        } else if (stmt instanceof ReturnStmt ret) {
            addStatement(block, kindStmt("return"));
            addEdge(block, exitId, "return");
            return null;

        } else if (stmt instanceof ThrowStmt thr) {
            addStatement(block, kindStmt("throw"));
            addEdge(block, exitId, "exception");
            return null;

        } else if (stmt instanceof IfStmt ifStmt) {
            return processIf(ifStmt, block, exitId);

        } else if (stmt instanceof WhileStmt ws) {
            return processLoop(ws.getBody(), block, exitId);

        } else if (stmt instanceof ForStmt fs) {
            return processLoop(fs.getBody(), block, exitId);

        } else if (stmt instanceof ForEachStmt fe) {
            return processLoop(fe.getBody(), block, exitId);

        } else if (stmt instanceof DoStmt ds) {
            return processLoop(ds.getBody(), block, exitId);

        } else if (stmt instanceof TryStmt ts) {
            return processTry(ts, block, exitId);

        } else if (stmt instanceof SwitchStmt ss) {
            return processSwitch(ss, block, exitId);

        } else if (stmt instanceof BlockStmt bs) {
            return processStatements(bs.getStatements(), block, exitId);
        }

        return block;
    }

    private String processExpression(Expression expr, String block) {
        if (expr instanceof MethodCallExpr call) {
            addStatement(block, callStmt(call));

        } else if (expr instanceof AssignExpr assign) {
            addStatement(block, kindStmt("assign"));
            // RHS may be a call
            assign.getValue().ifMethodCallExpr(inner ->
                    addStatement(block, callStmt(inner)));

        } else if (expr instanceof VariableDeclarationExpr varDecl) {
            addStatement(block, kindStmt("assign"));
            varDecl.getVariables().forEach(vd ->
                    vd.getInitializer().ifPresent(init -> {
                        if (init instanceof MethodCallExpr inner) {
                            addStatement(block, callStmt(inner));
                        }
                    }));
        }
        return block;
    }

    // ── Control-flow shapes ───────────────────────────────────────────────────

    private String processIf(IfStmt ifStmt, String condBlock, String exitId) {
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("kind",       "condition");
        s.put("expression", ifStmt.getCondition().toString());
        addStatement(condBlock, s);

        String mergeId = newBlock();

        String trueId = newBlock();
        addEdge(condBlock, trueId, "true");
        String trueExit = processStatements(toList(ifStmt.getThenStmt()), trueId, exitId);
        if (trueExit != null) addEdge(trueExit, mergeId, "normal");

        if (ifStmt.getElseStmt().isPresent()) {
            String falseId = newBlock();
            addEdge(condBlock, falseId, "false");
            String falseExit = processStatements(
                    toList(ifStmt.getElseStmt().get()), falseId, exitId);
            if (falseExit != null) addEdge(falseExit, mergeId, "normal");
        } else {
            addEdge(condBlock, mergeId, "false");
        }

        return mergeId;
    }

    private String processLoop(Statement body, String preBlock, String exitId) {
        String headerId  = newBlock();
        addEdge(preBlock, headerId, "normal");
        addStatement(headerId, kindStmt("loop"));

        String bodyId    = newBlock();
        String loopExit  = newBlock();
        addEdge(headerId, bodyId,   "true");
        addEdge(headerId, loopExit, "false");

        String bodyExit = processStatements(toList(body), bodyId, exitId);
        if (bodyExit != null) addEdge(bodyExit, headerId, "loop");

        return loopExit;
    }

    private String processTry(TryStmt ts, String block, String exitId) {
        String tryBodyId = newBlock();
        addEdge(block, tryBodyId, "normal");

        String mergeId   = newBlock();
        String tryExit   = processStatements(ts.getTryBlock().getStatements(), tryBodyId, exitId);
        if (tryExit != null) addEdge(tryExit, mergeId, "normal");

        for (CatchClause clause : ts.getCatchClauses()) {
            String catchId   = newBlock();
            addEdge(tryBodyId, catchId, "exception");
            String catchExit = processStatements(clause.getBody().getStatements(), catchId, exitId);
            if (catchExit != null) addEdge(catchExit, mergeId, "normal");
        }

        if (ts.getFinallyBlock().isPresent()) {
            String finallyId   = newBlock();
            addEdge(mergeId, finallyId, "normal");
            String finallyExit = processStatements(
                    ts.getFinallyBlock().get().getStatements(), finallyId, exitId);
            return finallyExit; // finally block is the new continuation
        }

        return mergeId;
    }

    private String processSwitch(SwitchStmt ss, String block, String exitId) {
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("kind",       "condition");
        s.put("expression", ss.getSelector().toString());
        addStatement(block, s);

        String mergeId = newBlock();
        for (SwitchEntry entry : ss.getEntries()) {
            String caseId   = newBlock();
            addEdge(block, caseId, "normal");
            String caseExit = processStatements(entry.getStatements(), caseId, exitId);
            if (caseExit != null) addEdge(caseExit, mergeId, "normal");
        }
        return mergeId;
    }

    // ── Statement builders ────────────────────────────────────────────────────

    /** Creates a statement map with just a {@code kind} field. */
    private static Map<String, Object> kindStmt(String kind) {
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("kind", kind);
        return s;
    }

    /**
     * Creates a {@code call} statement, embedding the global call ID when
     * the AST node was registered.  Falls back to a bare {@code "call"} kind
     * for unregistered (external / filtered) calls.
     */
    private Map<String, Object> callStmt(MethodCallExpr call) {
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("kind", "call");
        String callId = callIdMap.get(call);
        if (callId != null) s.put("call_id", callId);
        return s;
    }

    // ── Utilities ─────────────────────────────────────────────────────────────

    private static NodeList<Statement> toList(Statement stmt) {
        if (stmt instanceof BlockStmt bs) return bs.getStatements();
        NodeList<Statement> list = new NodeList<>();
        list.add(stmt);
        return list;
    }
}
