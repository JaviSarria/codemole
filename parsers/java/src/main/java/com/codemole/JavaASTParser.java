package com.codemole;

import com.github.javaparser.StaticJavaParser;
import com.github.javaparser.ast.CompilationUnit;
import com.github.javaparser.ast.body.*;
import com.github.javaparser.ast.expr.*;
import com.github.javaparser.symbolsolver.JavaSymbolSolver;
import com.github.javaparser.symbolsolver.resolution.typesolvers.CombinedTypeSolver;
import com.github.javaparser.symbolsolver.resolution.typesolvers.JavaParserTypeSolver;
import com.github.javaparser.symbolsolver.resolution.typesolvers.ReflectionTypeSolver;
import com.codemole.cfg.CfgBuilder;
import com.codemole.norm.NormAstBuilder;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;

import java.io.IOException;
import java.nio.file.*;
import java.util.*;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * Entry point for the codemole Java AST parser.
 *
 * <h2>Usage</h2>
 * {@code java -jar java-parser.jar <source-root>}
 *
 * <h2>Output</h2>
 * A single JSON document written to {@code stdout} that conforms to the
 * {@code Program} schema (version 1.0) defined in {@code src/ir/mod.rs}.
 * Warnings and diagnostics go to {@code stderr}.
 *
 * <h2>Algorithm</h2>
 * <ol>
 *   <li><b>Setup</b> — configure the JavaParser Symbol Solver.</li>
 *   <li><b>Pass 1</b> — walk all {@code *.java} files; register every module,
 *       type, and function signature; cache {@link CompilationUnit} objects.</li>
 *   <li><b>Hierarchy resolution</b> — resolve {@code extends} / {@code implements}
 *       simple names to type IDs now that all types are known.</li>
 *   <li><b>Pass 2</b> — re-use cached CUs; process every method body; emit
 *       calls and CFGs referencing their global IDs.</li>
 *   <li><b>Output</b> — serialise the accumulated {@link ProgramBuilder} state.</li>
 * </ol>
 */
public class JavaASTParser {

    public static void main(String[] args) {
        if (args.length < 1) {
            System.err.println("Usage: java-parser <source-root>");
            System.exit(1);
        }

        Path rootPath = Paths.get(args[0]).toAbsolutePath().normalize();

        // ── Symbol Solver ─────────────────────────────────────────────────────
        CombinedTypeSolver typeSolver = new CombinedTypeSolver(new ReflectionTypeSolver());
        try {
            typeSolver.add(new JavaParserTypeSolver(rootPath));
        } catch (Exception e) {
            System.err.println("[warn] TypeSolver: " + e.getMessage());
        }
        StaticJavaParser.getParserConfiguration()
                .setSymbolResolver(new JavaSymbolSolver(typeSolver));

        // ── Collect source files ──────────────────────────────────────────────
        List<Path> javaFiles = collectJavaFiles(rootPath);

        ProgramBuilder builder = new ProgramBuilder();

        // ── Pass 1: register structure ────────────────────────────────────────
        Map<Path, CompilationUnit> cuCache = new LinkedHashMap<>();
        for (Path file : javaFiles) {
            try {
                CompilationUnit cu = StaticJavaParser.parse(file);
                cuCache.put(file, cu);
                pass1Register(cu, rootPath, builder);
            } catch (Exception e) {
                System.err.println("[warn] Pass 1 – " + rootPath.relativize(file) + ": " + e.getMessage());
            }
        }

        // Resolve type hierarchy now that all types are known
        builder.resolveTypeHierarchy();

        // ── Pass 2: process method bodies ─────────────────────────────────────
        for (Map.Entry<Path, CompilationUnit> entry : cuCache.entrySet()) {
            try {
                String relFile = rootPath.relativize(entry.getKey()).toString().replace('\\', '/');
                pass2Process(entry.getValue(), relFile, builder);
            } catch (Exception e) {
                System.err.println("[warn] Pass 2 – " + entry.getKey().getFileName() + ": " + e.getMessage());
            }
        }

        // ── Output ────────────────────────────────────────────────────────────
        Map<String, Object> program = new LinkedHashMap<>();
        program.put("version",   "1.0");
        program.put("language",  "java");
        program.put("modules",   builder.getModules());
        program.put("types",     builder.getTypes());
        program.put("functions", builder.getFunctions());
        program.put("calls",     builder.getCalls());
        program.put("cfgs",      builder.getCfgs());

        Gson gson = new GsonBuilder().setPrettyPrinting().serializeNulls().create();
        System.out.println(gson.toJson(program));
    }

    // =========================================================================
    // Pass 1 — register all modules, types, functions
    // =========================================================================

    private static void pass1Register(CompilationUnit cu, Path root, ProgramBuilder builder) {
        String pkg      = cu.getPackageDeclaration().map(pd -> pd.getNameAsString()).orElse("");
        String moduleId = builder.registerModule(pkg);

        for (TypeDeclaration<?> typeDecl : cu.getTypes()) {
            registerTypeRecursive(typeDecl, pkg, moduleId, builder);
        }
    }

    private static void registerTypeRecursive(TypeDeclaration<?> typeDecl,
                                               String pkg, String moduleId,
                                               ProgramBuilder builder) {
        String simpleName    = typeDecl.getNameAsString();
        String qualifiedName = pkg.isEmpty() ? simpleName : pkg + "." + simpleName;

        String kind;
        String extendsName = null;
        List<String> implementsNames = new ArrayList<>();

        if (typeDecl instanceof ClassOrInterfaceDeclaration coid) {
            kind = coid.isInterface() ? "interface" : "class";
            if (!coid.getExtendedTypes().isEmpty()) {
                extendsName = coid.getExtendedTypes().get(0).getNameAsString();
            }
            implementsNames = coid.getImplementedTypes().stream()
                    .map(t -> t.getNameAsString())
                    .collect(Collectors.toList());
            // record for polymorphism resolution
            for (String iface : implementsNames) {
                builder.registerImplementation(qualifiedName, iface);
            }
        } else if (typeDecl instanceof EnumDeclaration) {
            kind = "enum";
        } else {
            kind = "class";
        }

        List<String> typeAnnotations = annotationNames(typeDecl.getAnnotations());
        builder.registerType(qualifiedName, simpleName, moduleId,
                kind, extendsName, implementsNames, typeAnnotations);

        // Register all methods so their IDs are available to callers in other types
        for (MethodDeclaration md : typeDecl.getMethods()) {
            List<String> params     = md.getParameters().stream()
                    .map(p -> p.getTypeAsString()).collect(Collectors.toList());
            String returnType       = "void".equals(md.getTypeAsString()) ? null : md.getTypeAsString();
            String visibility       = accessToString(md.getAccessSpecifier());
            List<String> methodAnns = annotationNames(md.getAnnotations());
            builder.registerFunction(qualifiedName, md.getNameAsString(),
                    moduleId, params, returnType, visibility, methodAnns);
        }

        // Nested types (inner classes, etc.)
        for (BodyDeclaration<?> member : typeDecl.getMembers()) {
            if (member instanceof TypeDeclaration<?> nested) {
                // Inner types are scoped under the outer type's qualified name
                registerTypeRecursive(nested, qualifiedName, moduleId, builder);
            }
        }
    }

    // =========================================================================
    // Pass 2 — extract calls and CFGs
    // =========================================================================

    private static void pass2Process(CompilationUnit cu, String relFile,
                                     ProgramBuilder builder) {
        String pkg = cu.getPackageDeclaration().map(pd -> pd.getNameAsString()).orElse("");

        for (TypeDeclaration<?> typeDecl : cu.getTypes()) {
            String classLevelMapping = SpringDetector.extractClassMapping(typeDecl.getAnnotations());
            processTypeRecursive(typeDecl, pkg, relFile, classLevelMapping, builder);
        }
    }

    private static void processTypeRecursive(TypeDeclaration<?> typeDecl,
                                              String pkg, String relFile,
                                              String classLevelMapping,
                                              ProgramBuilder builder) {
        String simpleName    = typeDecl.getNameAsString();
        String qualifiedName = pkg.isEmpty() ? simpleName : pkg + "." + simpleName;

        // field-name → declared-type map (for call resolution when Symbol Solver fails)
        Map<String, String> fieldTypeMap = buildFieldTypeMap(typeDecl);

        for (MethodDeclaration md : typeDecl.getMethods()) {
            String callerId = builder.getFunctionId(qualifiedName, md.getNameAsString());
            if (callerId == null) continue;

            int line = md.getBegin().map(p -> p.line).orElse(0);

            // ── Spring endpoint ───────────────────────────────────────────────
            boolean isEndpoint = SpringDetector.isEndpoint(md.getAnnotations());
            String  route      = isEndpoint
                    ? SpringDetector.extractRoute(md.getAnnotations(), classLevelMapping)
                    : null;
            builder.updateFunction(callerId, isEndpoint, route, relFile, line);

            // ── Calls (identity map preserves AST node identity for CFG) ──────
            //
            // We intentionally EXCLUDE calls that are nested inside:
            //   • LambdaExpr      — e.g. forEach(() -> addNewRunningCategory())
            //   • anonymous ObjectCreationExpr — e.g. new Runnable() { run() { ... } }
            //
            // Such calls are syntactically part of the enclosing method but are
            // logically attributed to the lambda/anonymous-class body, NOT to the
            // enclosing method's direct call graph.  Registering them here would
            // falsely show them as direct callers on the sequence diagram.
            //
            // The IdentityHashMap preserves AST node identity so CfgBuilder can
            // look up the same node by reference later.
            Map<MethodCallExpr, String> callIdMap = new IdentityHashMap<>();
            md.findAll(MethodCallExpr.class).stream()
                    .filter(call -> isDirectCallInMethod(call, md))
                    .forEach(call -> processCall(call, callerId, qualifiedName, pkg,
                            fieldTypeMap, builder, callIdMap));

            // ── CFG ───────────────────────────────────────────────────────────
            md.getBody().ifPresent(body -> {
                CfgBuilder cfgBuilder = new CfgBuilder(callerId, callIdMap);
                Map<String, Object> cfg = cfgBuilder.build(body.getStatements());
                if (cfg != null) builder.addCfg(cfg);
            });

            // ── Normalized AST body ───────────────────────────────────────────
            md.getBody().ifPresent(body -> {
                Map<String, Object> bodyAst = NormAstBuilder.buildBody(body.getStatements());
                builder.setFunctionBodyAst(callerId, bodyAst);
            });
        }

        // Recurse into nested types
        for (BodyDeclaration<?> member : typeDecl.getMembers()) {
            if (member instanceof TypeDeclaration<?> nested) {
                processTypeRecursive(nested, qualifiedName, relFile, classLevelMapping, builder);
            }
        }
    }

    // =========================================================================
    // Call resolution
    // =========================================================================

    private static void processCall(MethodCallExpr call, String callerId,
                                    String ownerType, String pkg,
                                    Map<String, String> fieldTypeMap,
                                    ProgramBuilder builder,
                                    Map<MethodCallExpr, String> callIdMap) {
        int    callLine    = call.getBegin().map(p -> p.line).orElse(0);
        String methodName  = call.getNameAsString();
        String calleeType  = null;
        String callType    = "direct";
        List<String> possibleTargets = new ArrayList<>();

        // 1. Symbol Solver (best effort)
        try {
            var resolved = call.resolve();
            calleeType = resolved.declaringType().getQualifiedName();
            callType   = call.getScope().isPresent() ? "virtual" : "direct";
        } catch (Exception e) {
            // 2. Fallback: scope-based type resolution
            if (call.getScope().isPresent()) {
                String scope = scopeRootName(call.getScope().get().toString());
                calleeType   = fieldTypeMap.getOrDefault(scope, null);
                if (calleeType == null) {
                    // scope may itself be a type name (static call)
                    calleeType = builder.resolveTypeId(scope, pkg);
                }
                callType = "virtual";
            } else {
                calleeType = ownerType;
            }
        }

        if (calleeType == null) return;

        // 3. Interface call → collect possible_targets
        if ("interface".equals(builder.getTypeKind(calleeType))) {
            callType = "interface";
            String ifaceSimple = calleeType.contains(".")
                    ? calleeType.substring(calleeType.lastIndexOf('.') + 1)
                    : calleeType;
            for (String impl : builder.getImplementors(ifaceSimple)) {
                String targetId = builder.getFunctionId(impl, methodName);
                if (targetId != null) possibleTargets.add(targetId);
            }
        }

        // 4. Look up callee function ID
        String calleeId = builder.getFunctionId(calleeType, methodName);
        if (calleeId == null) {
            // External symbol — skip (no external entry in our function list)
            return;
        }

        String callId = builder.registerCall(callerId, calleeId, callType,
                callLine, possibleTargets);
        callIdMap.put(call, callId);
    }

    // =========================================================================
    // Utility helpers
    // =========================================================================

    /**
     * Returns {@code true} when {@code call} is a <em>direct</em> call within
     * {@code method} — i.e. it is <strong>not</strong> nested inside a
     * {@link LambdaExpr} or an anonymous {@link ObjectCreationExpr} that is
     * itself inside the method body.
     *
     * <h3>Why this matters for the sequence diagram</h3>
     * {@code findAll(MethodCallExpr.class)} recurses into lambdas.  Without this
     * filter, a call like:
     * <pre>
     *   checkLastExecution(() -&gt; addNewRunningCategory());
     * </pre>
     * would register {@code addNewRunningCategory} as a <em>direct</em> callee of
     * the enclosing method, causing it to appear <em>before</em>
     * {@code checkLastExecution} on the sequence diagram.
     */
    private static boolean isDirectCallInMethod(MethodCallExpr call, MethodDeclaration method) {
        com.github.javaparser.ast.Node parent = call.getParentNode().orElse(null);
        while (parent != null && parent != method) {
            if (parent instanceof LambdaExpr) {
                return false;
            }
            if (parent instanceof ObjectCreationExpr oce
                    && oce.getAnonymousClassBody().isPresent()) {
                return false;
            }
            parent = parent.getParentNode().orElse(null);
        }
        return true;
    }

    private static List<Path> collectJavaFiles(Path root) {
        try (Stream<Path> stream = Files.walk(root)) {
            return stream
                    .filter(p -> p.toString().endsWith(".java"))
                    .sorted()
                    .collect(Collectors.toList());
        } catch (IOException e) {
            System.err.println("[error] Cannot walk source tree: " + e.getMessage());
            System.exit(1);
            return Collections.emptyList();
        }
    }

    /** Builds a field-name → declared-type map for quick lookup. */
    private static Map<String, String> buildFieldTypeMap(TypeDeclaration<?> typeDecl) {
        Map<String, String> map = new HashMap<>();
        for (FieldDeclaration fd : typeDecl.getFields()) {
            String typeName = fd.getElementType().asString();
            for (VariableDeclarator vd : fd.getVariables()) {
                map.put(vd.getNameAsString(), typeName);
            }
        }
        return map;
    }

    /**
     * Extracts the root identifier from a potentially chained scope expression.
     * E.g. {@code "userService.repo"} → {@code "userService"}.
     */
    private static String scopeRootName(String scope) {
        int dot = scope.indexOf('.');
        return dot > 0 ? scope.substring(0, dot) : scope;
    }

    private static String accessToString(com.github.javaparser.ast.AccessSpecifier access) {
        return switch (access) {
            case PUBLIC    -> "public";
            case PRIVATE   -> "private";
            case PROTECTED -> "protected";
            default        -> "package";
        };
    }

    private static List<String> annotationNames(
            com.github.javaparser.ast.NodeList<? extends AnnotationExpr> annotations) {
        return annotations.stream()
                .map(AnnotationExpr::getNameAsString)
                .collect(Collectors.toList());
    }
}
