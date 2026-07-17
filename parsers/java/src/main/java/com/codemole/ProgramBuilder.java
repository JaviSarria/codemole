package com.codemole;

import java.util.*;

/**
 * Stateful accumulator for the flat {@code Program} JSON document.
 *
 * <p>Responsibilities:
 * <ul>
 *   <li>Assign stable, monotonically-increasing string IDs to every entity.</li>
 *   <li>Maintain lookup maps so that pass-2 code can retrieve IDs by semantic key.</li>
 *   <li>Track which classes implement which interfaces for polymorphism resolution.</li>
 * </ul>
 *
 * <p>All {@code register*} methods are idempotent on the ID: calling them twice
 * with the same key returns the same ID and does not add a duplicate entry to the
 * output list.
 */
public class ProgramBuilder {

    private int counter = 0;

    // ── ID registries ─────────────────────────────────────────────────────────

    /** Package name → module ID */
    private final Map<String, String> moduleIds    = new LinkedHashMap<>();
    /** Qualified type name → type ID */
    private final Map<String, String> typeIds      = new LinkedHashMap<>();
    /** "qualifiedType.methodName" → function ID */
    private final Map<String, String> functionIds  = new LinkedHashMap<>();
    /** Qualified type name → "class" | "interface" | "enum" */
    private final Map<String, String> typeKinds    = new HashMap<>();
    /** Interface simple-name → list of implementing qualified class names */
    private final Map<String, List<String>> implementorsMap = new HashMap<>();

    // ── Mutable function metadata (updated in pass 2) ─────────────────────────

    /** Function ID → the function map (allows in-place updates) */
    private final Map<String, Map<String, Object>> functionById = new LinkedHashMap<>();

    // ── Output lists (order preserved) ───────────────────────────────────────

    private final List<Map<String, Object>> modules   = new ArrayList<>();
    private final List<Map<String, Object>> types     = new ArrayList<>();
    private final List<Map<String, Object>> functions = new ArrayList<>();
    private final List<Map<String, Object>> calls     = new ArrayList<>();
    private final List<Map<String, Object>> cfgs      = new ArrayList<>();

    // ── ID generation ─────────────────────────────────────────────────────────

    private String nextId(String prefix) {
        return prefix + (++counter);
    }

    // ── Module ────────────────────────────────────────────────────────────────

    public String registerModule(String pkg) {
        return moduleIds.computeIfAbsent(pkg, k -> {
            String id = nextId("m");
            Map<String, Object> m = new LinkedHashMap<>();
            m.put("id",   id);
            m.put("name", pkg.isEmpty() ? "(default)" : pkg);
            m.put("path", pkg.replace('.', '/'));
            modules.add(m);
            return id;
        });
    }

    // ── Type ──────────────────────────────────────────────────────────────────

    /**
     * Registers a type declaration.
     *
     * @param qualifiedName fully-qualified class / interface name
     * @param simpleName    unqualified name (stored in the JSON)
     * @param moduleId      owning module ID
     * @param kind          "class" | "interface" | "enum"
     * @param extendsName   simple name of extended class (may be null)
     * @param implementsNames simple names of implemented interfaces
     * @param annotations   annotation names on the declaration
     */
    public String registerType(String qualifiedName, String simpleName, String moduleId,
                               String kind, String extendsName,
                               List<String> implementsNames, List<String> annotations) {
        if (typeIds.containsKey(qualifiedName)) {
            return typeIds.get(qualifiedName);
        }

        String id = nextId("t");
        typeIds.put(qualifiedName, id);
        typeKinds.put(qualifiedName, kind);

        Map<String, Object> t = new LinkedHashMap<>();
        t.put("id",          id);
        t.put("name",        simpleName);
        t.put("module_id",   moduleId);
        t.put("kind",        kind);
        // extends / implements resolved lazily after all types are registered
        t.put("extends",    extendsName);
        t.put("implements", new ArrayList<>(implementsNames));
        t.put("annotations", new ArrayList<>(annotations));

        types.add(t);
        return id;
    }

    /**
     * Second-pass resolution of {@code extends} / {@code implements} type IDs.
     * Must be called once all types have been registered.
     */
    @SuppressWarnings("unchecked")
    public void resolveTypeHierarchy() {
        for (Map<String, Object> t : types) {
            String pkg = packageOf((String) t.get("name"), t);

            // resolve extends
            String extendsSimple = (String) t.get("extends");
            if (extendsSimple != null) {
                String resolved = resolveTypeId(extendsSimple, pkg);
                t.put("extends", resolved); // null if not found (external)
            }

            // resolve implements list
            List<String> implNames = (List<String>) t.get("implements");
            List<String> implIds   = new ArrayList<>();
            for (String name : implNames) {
                String resolved = resolveTypeId(name, pkg);
                if (resolved != null) implIds.add(resolved);
            }
            t.put("implements", implIds);
        }
    }

    // ── Function ──────────────────────────────────────────────────────────────

    public String registerFunction(String qualifiedTypeName, String methodName,
                                   String moduleId, List<String> params,
                                   String returnType, String visibility,
                                   List<String> annotations) {
        String key = qualifiedTypeName + "." + methodName;
        if (functionIds.containsKey(key)) {
            return functionIds.get(key);
        }

        String id      = nextId("f");
        String typeId  = typeIds.get(qualifiedTypeName);

        functionIds.put(key, id);

        Map<String, Object> f = new LinkedHashMap<>();
        f.put("id",            id);
        f.put("name",          methodName);
        f.put("module_id",     moduleId);
        f.put("owner_type_id", typeId);           // null if type not found
        f.put("params",        new ArrayList<>(params));
        f.put("return_type",   returnType);        // null ↔ void
        f.put("visibility",    visibility);
        f.put("is_entrypoint", false);
        f.put("route",         null);
        f.put("annotations",   new ArrayList<>(annotations));
        f.put("location",      null);

        functions.add(f);
        functionById.put(id, f);
        return id;
    }

    /**
     * Updates a function's entrypoint metadata and source location.
     * Safe to call multiple times (last write wins).
     */
    public void updateFunction(String functionId, boolean isEntrypoint, String route,
                               String file, int line) {
        Map<String, Object> f = functionById.get(functionId);
        if (f == null) return;

        f.put("is_entrypoint", isEntrypoint);
        f.put("route",         route);

        if (file != null && line > 0) {
            Map<String, Object> loc = new LinkedHashMap<>();
            loc.put("file", file);
            loc.put("line", line);
            f.put("location", loc);
        }
    }

    // ── Call ──────────────────────────────────────────────────────────────────

    public String registerCall(String callerId, String calleeId, String callType,
                               int line, List<String> possibleTargets) {
        String id = nextId("c");

        Map<String, Object> c = new LinkedHashMap<>();
        c.put("id",         id);
        c.put("caller_id",  callerId);
        c.put("callee_id",  calleeId);
        c.put("call_type",  callType);
        c.put("line",       line > 0 ? line : null);

        if (possibleTargets != null && !possibleTargets.isEmpty()) {
            c.put("possible_targets", new ArrayList<>(possibleTargets));
        }

        calls.add(c);
        return id;
    }

    // ── Normalized AST body ───────────────────────────────────────────────────

    /**
     * Attaches the normalized AST body to an already-registered function.
     *
     * @param functionId the function's global ID (e.g. {@code "f3"})
     * @param bodyAst    a {@code Map} representing {@code NStmt::Block}
     */
    public void setFunctionBodyAst(String functionId, Map<String, Object> bodyAst) {
        Map<String, Object> f = functionById.get(functionId);
        if (f != null) {
            f.put("body_ast", bodyAst);
        }
    }

    // ── CFG ───────────────────────────────────────────────────────────────────

    public void addCfg(Map<String, Object> cfg) {
        cfgs.add(cfg);
    }

    // ── Polymorphism ──────────────────────────────────────────────────────────

    public void registerImplementation(String qualifiedClassName, String interfaceSimpleName) {
        implementorsMap
            .computeIfAbsent(interfaceSimpleName, k -> new ArrayList<>())
            .add(qualifiedClassName);
    }

    /** Returns all qualified class names that implement {@code interfaceSimpleName}. */
    public List<String> getImplementors(String interfaceSimpleName) {
        return implementorsMap.getOrDefault(interfaceSimpleName, Collections.emptyList());
    }

    // ── Lookups ───────────────────────────────────────────────────────────────

    public String getFunctionId(String qualifiedType, String methodName) {
        return functionIds.get(qualifiedType + "." + methodName);
    }

    public String getTypeId(String qualifiedName) {
        return typeIds.get(qualifiedName);
    }

    public String getTypeKind(String qualifiedName) {
        return typeKinds.getOrDefault(qualifiedName, "class");
    }

    /**
     * Tries to resolve a simple or qualified type name to a registered type ID.
     * Searches: exact match → package-qualified → simple-name suffix match.
     */
    public String resolveTypeId(String name, String pkg) {
        // 1. Already qualified
        String direct = typeIds.get(name);
        if (direct != null) return direct;

        // 2. Package-qualified
        if (!pkg.isEmpty()) {
            String qualified = pkg + "." + name;
            String q = typeIds.get(qualified);
            if (q != null) return q;
        }

        // 3. Simple-name suffix match (handles imports)
        for (Map.Entry<String, String> e : typeIds.entrySet()) {
            if (e.getKey().endsWith("." + name)) return e.getValue();
        }

        return null;
    }

    // ── Output accessors ──────────────────────────────────────────────────────

    public List<Map<String, Object>> getModules()   { return modules; }
    public List<Map<String, Object>> getTypes()     { return types; }
    public List<Map<String, Object>> getFunctions() { return functions; }
    public List<Map<String, Object>> getCalls()     { return calls; }
    public List<Map<String, Object>> getCfgs()      { return cfgs; }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /** Best-effort: infer the package of a type from its registered module. */
    @SuppressWarnings("unchecked")
    private String packageOf(String simpleName, Map<String, Object> typeMap) {
        // Walk typeIds to find the qualified name for this type map entry
        for (Map.Entry<String, String> e : typeIds.entrySet()) {
            if (e.getValue().equals(typeMap.get("id"))) {
                String qn = e.getKey();
                int dot = qn.lastIndexOf('.');
                return dot > 0 ? qn.substring(0, dot) : "";
            }
        }
        return "";
    }
}
