package com.codemole;

import com.github.javaparser.ast.NodeList;
import com.github.javaparser.ast.expr.*;

import java.util.Map;

/**
 * Detects Spring MVC / Spring Boot endpoint annotations and injection markers.
 *
 * <p>Supported endpoint annotations:
 * {@code @GetMapping}, {@code @PostMapping}, {@code @PutMapping},
 * {@code @DeleteMapping}, {@code @PatchMapping}, {@code @RequestMapping}.
 *
 * <p>Supported injection annotations:
 * {@code @Autowired}, {@code @Inject}, {@code @Resource}.
 */
public class SpringDetector {

    /** Maps annotation simple-name → HTTP method string. */
    private static final Map<String, String> HTTP_ANNOTATIONS = Map.of(
        "GetMapping",    "GET",
        "PostMapping",   "POST",
        "PutMapping",    "PUT",
        "DeleteMapping", "DELETE",
        "PatchMapping",  "PATCH",
        "RequestMapping","GET"   // default HTTP method for @RequestMapping
    );

    private SpringDetector() {}

    // ── Endpoint detection ────────────────────────────────────────────────────

    /** Returns {@code true} when the annotation list contains an HTTP-mapping annotation. */
    public static boolean isEndpoint(NodeList<AnnotationExpr> annotations) {
        return annotations.stream()
                .anyMatch(a -> HTTP_ANNOTATIONS.containsKey(a.getNameAsString()));
    }

    /**
     * Builds a {@code "METHOD /path"} route string from the method's annotations
     * combined with the class-level path prefix.
     *
     * @param annotations       method-level annotations
     * @param classLevelMapping class-level {@code @RequestMapping} value, or ""
     * @return route string, or {@code null} when no mapping annotation is found
     */
    public static String extractRoute(NodeList<AnnotationExpr> annotations,
                                      String classLevelMapping) {
        for (AnnotationExpr ann : annotations) {
            String httpMethod = HTTP_ANNOTATIONS.get(ann.getNameAsString());
            if (httpMethod != null) {
                String path        = extractPath(ann);
                String classPrefix = classLevelMapping == null ? "" : classLevelMapping;
                String fullPath    = normalisePath(classPrefix + path);
                return httpMethod + " " + fullPath;
            }
        }
        return null;
    }

    /**
     * Extracts the class-level {@code @RequestMapping} value from a type declaration's
     * annotation list.
     *
     * @return the path value, or "" when no class-level mapping exists
     */
    public static String extractClassMapping(NodeList<AnnotationExpr> annotations) {
        for (AnnotationExpr ann : annotations) {
            if (ann.getNameAsString().equals("RequestMapping")) {
                return extractPath(ann);
            }
        }
        return "";
    }

    // ── Injection detection ───────────────────────────────────────────────────

    /** Returns {@code true} when the annotation list contains a DI injection annotation. */
    public static boolean isInjected(NodeList<AnnotationExpr> annotations) {
        return annotations.stream().anyMatch(a ->
            a.getNameAsString().equals("Autowired") ||
            a.getNameAsString().equals("Inject")    ||
            a.getNameAsString().equals("Resource")
        );
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /**
     * Extracts the path value from a mapping annotation.
     *
     * <ul>
     *   <li>{@code @GetMapping} → {@code ""}</li>
     *   <li>{@code @GetMapping("/users")} → {@code "/users"}</li>
     *   <li>{@code @RequestMapping(value="/users", method=GET)} → {@code "/users"}</li>
     * </ul>
     */
    private static String extractPath(AnnotationExpr ann) {
        if (ann instanceof MarkerAnnotationExpr) {
            return "";
        }
        if (ann instanceof SingleMemberAnnotationExpr smae) {
            return stripQuotes(smae.getMemberValue().toString());
        }
        if (ann instanceof NormalAnnotationExpr nae) {
            for (MemberValuePair pair : nae.getPairs()) {
                String key = pair.getNameAsString();
                if (key.equals("value") || key.equals("path")) {
                    // value may be an array: {"a","b"} — take the first element
                    String raw = pair.getValue().toString();
                    if (raw.startsWith("{")) {
                        raw = raw.substring(1, raw.indexOf('}')).split(",")[0].trim();
                    }
                    return stripQuotes(raw);
                }
            }
        }
        return "";
    }

    private static String stripQuotes(String s) {
        return s.replace("\"", "").trim();
    }

    private static String normalisePath(String path) {
        // Collapse double slashes introduced by concatenation
        return path.replaceAll("//+", "/");
    }
}
