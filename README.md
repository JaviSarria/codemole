# codemole

**codemole** is a CLI and desktop tool that traces an API endpoint or a function/method through your source code and generates sequence and class/flow diagrams.

---

## How it works

1. **Find** — Locates the entry point in the codebase. codemole supports two modes:
   - Endpoint mode: locate an HTTP handler using framework-specific patterns (Spring annotations, FastAPI decorators, Gin route registrations).
   - Function mode: locate a function or method by name inside a given class/package/module (see "Function mode" below).
2. **Parse** — For Java projects, launches an external Java parser (`java -jar java-parser.jar <source-root>`) as a subprocess. The parser performs AST analysis and writes a structured JSON document to stdout. codemole reads and deserializes this JSON into a unified Intermediate Representation (IR).
3. **Traverse** — Builds a call graph from the IR and performs a BFS walk starting from the matched handler/function, skipping symbols loaded from an SQLite database (stdlib, framework helpers).
4. **Generate** — Produces a Mermaid sequence diagram source and Graphviz DOT sources for class/flow/component/dependency diagrams.
5. **Render** — Uses the built-in pure-Rust SVG renderer for sequence diagrams; calls `dot` (Graphviz) for class/flow diagrams when available, falling back to the built-in renderer otherwise. Also writes self-contained HTML viewers with pan/zoom support.

For Java projects that use interfaces (Spring `@Controller` + implementation class, OpenAPI-generated stubs, Feign clients, etc.), codemole detects that the matched class is an interface, finds the concrete implementation, and builds the call graph from the real business logic. The same resolution is attempted when a function inside an interface is targeted.

---

## Supported languages and frameworks

| Language | Framework | Patterns recognized |
|----------|-----------|---------------------|
| `java`   | Spring Boot | `@GetMapping`, `@PostMapping`, `@PutMapping`, `@DeleteMapping`, `@PatchMapping`, class-level `@RequestMapping` prefix |
| `python` | FastAPI     | `@app.get/post/put/delete/patch(...)`, `@router.*(...)` |
| `go`     | Gin         | `r.GET/POST/PUT/DELETE/PATCH(...)`, named group routes |

---

## Prerequisites

| Dependency | Required for | Minimum version |
|------------|-------------|-----------------|
| [Rust](https://rustup.rs/) + Cargo | Building codemole | 1.81 |
| [Java JDK](https://adoptium.net/) | Analyzing Java projects (`--lang java`) | 17 |
| [Maven](https://maven.apache.org/) | Building the Java parser jar (one-time) | 3.6 |
| [Graphviz](https://graphviz.org/) | Class/flow diagram SVG (optional) | any recent |

Java and Maven are only needed if you intend to analyze Java source code. Graphviz is optional (a built-in Rust renderer is used as fallback).

---

## Installation

See the `scripts/` folder for convenience installers. Typical steps:

1. Clone repository.
2. (Optional) Build Java parser: `cd parsers/java && mvn package -DskipTests` to produce the fat JAR.
3. Build the CLI and desktop binaries: `cargo build --release --bins`.
4. Optional: build the debug helper example: `cargo build --release --example debug_calls`.

---

## Usage

codemole now supports two entry points:

- CLI mode: pass arguments to `codemole`
- Desktop frontend: run `codemole` without arguments, or launch `codemole-desktop` directly

### CLI mode

```text
codemole --lang <LANG> [--endpoint <PATH> | --funcion <NAME>]
         [--path <DIR>] [--output <DIR>] [--db <FILE>]
         [--java-parser <JAR>] [--clase <NAME>] [--paquete <NAME>] [--modulo <NAME>]
```

The CLI requires exactly one tracing mode:

- `--endpoint` to trace an HTTP handler
- `--funcion` to trace a specific function or method

If you run from Cargo in development, pass CLI arguments after `--`:

```bash
cargo run --bin codemole -- --lang java --endpoint /api/users --path ./my-spring-project
```

### Desktop frontend

When `codemole` is executed without arguments, it tries to launch the companion desktop binary `codemole-desktop` from the same directory.

```bash
# Build both binaries
cargo build --release --bins

# Launch the desktop frontend directly in development
cargo run --bin codemole-desktop

# Launch the desktop frontend from the built binary pair
./target/release/codemole
```

Inside the desktop app you can:

- Select language: `java`, `python`, `go`
- Select tracing mode: endpoint or function
- Apply one of the built-in templates. These labels appear exactly as shown in the UI:
  - `Spring - Endpoint`
  - `Spring - Funcion`
  - `FastAPI - Endpoint`
  - `FastAPI - Funcion`
  - `Gin - Endpoint`
  - `Gin - Funcion`
- Fill the mode-specific fields:
  - Java function mode: `--clase` and `--paquete`
  - Python function mode: `--clase` and `--modulo`
  - Go function mode: `--modulo`
- Configure optional paths for `--output`, `--db`, and `--java-parser`
- Run codemole and inspect stdout/stderr in the integrated log panel

If you pass arguments to `codemole`, it stays in CLI mode and does not open the desktop frontend.

### Options

| Flag | Description | Notes |
|------|-------------|-------|
| `--lang` | Language/framework: `java` \| `python` \| `go` | required |
| `--endpoint` | API endpoint to trace, e.g. `/api/users` | Mutually exclusive with `--funcion`. Use for endpoint-mode tracing |
| `--funcion` | Function or method name to trace | Mutually exclusive with `--endpoint`. Use for function-mode tracing |
| `--clase` | Class name (used with `--funcion` for Java and Python) | Required for Java and Python when using `--funcion` |
| `--paquete` | Package name (used with `--funcion` for Java) | Required for Java when using `--funcion` |
| `--modulo` | Module name (used with `--funcion` for Python and Go) | Required for Python and Go when using `--funcion` |
| `--path` | Root directory of the source code to analyze | `.` (current dir) |
| `--output` | Base output directory; a sub-folder named after the endpoint or function is created inside | OS temp dir |
| `--db` | Path to the skip-symbols SQLite database (created on first run) | `<exe-dir>/symbols.db` |
| `--java-parser` | Path to `java-parser.jar` (used for CFG-enriched Java sequence diagrams) | `./parsers/java/target/java-parser.jar` |

### Examples

Endpoint mode:

```bash
# Java / Spring Boot (endpoint mode)
codemole --lang java \
         --endpoint /api/users \
         --path ./my-spring-project \
         --java-parser ./parsers/java/target/java-parser.jar \
         --output ./diagrams

# Python / FastAPI (endpoint mode)
codemole --lang python --endpoint /items/{id} --path ./my-fastapi-project --output ./diagrams

# Go / Gin (endpoint mode)
codemole --lang go --endpoint /health --path ./my-gin-project --output ./diagrams
```

Function mode (finds a specific function/method instead of an HTTP handler):

```bash
# Java: require --clase and --paquete
codemole --lang java --funcion getUserById --clase UserController --paquete com.example.users --path ./my-spring-project

# Python: require --clase and --modulo
codemole --lang python --funcion get_item --clase ItemService --modulo services.items --path ./my-fastapi-project

# Go: require --modulo (module/directory name)
codemole --lang go --funcion GetHealth --modulo handlers --path ./my-gin-project
```

Path parameters are treated as wildcards in endpoint mode — `/items/{id}` matches `/items/{item_id}` in FastAPI or `/items/:id` in Gin.

Output is written to `<output>/<slug>/` where `slug` is derived from the endpoint or the function label.

---

## Debugging and troubleshooting

### Debug tool for method call analysis

The `debug_calls` example helps diagnose why a method call is missing from the generated sequence diagram, why a method definition is not being detected, or why call order looks wrong.

Build it once:

```bash
cargo build --release --example debug_calls
```

Run it against a Java source file and method name:

```bash
./target/release/examples/debug_calls.exe <java-file> <method-name>
```

Example:

```bash
./target/release/examples/debug_calls.exe src/main/java/com/example/MyService.java crudTicket
```

On Windows you can also use the helper script:

```powershell
.\debug.ps1 -JavaFile "C:\path\to\TicketCreationService.java" -MethodName crudTicket
```

### What the debug tool shows

The output is split into three main sections:

1. **Method Source**
  Confirms the exact method body that codemole matched. This is the first thing to verify when the wrong method seems to be analyzed.
2. **Detected Calls**
  Shows the calls found in source order and whether each one is kept or skipped.
  - `[KEPT]`: eligible for traversal and diagram generation
  - `[SKIPPED]`: filtered out by the skip-symbol database or keyword rules
3. **Final Calls (AFTER filtering)**
  Shows the calls that should actually appear in the diagram.

Expected output for a method like `crudTicket()` usually looks like this:

```text
--- Final Calls (AFTER filtering) ---
Kept for traversal:
  ✓ getMapTicketToXML
  ✓ insertTicket
  ✓ modificarTicket
```

### Typical interpretation

If a method appears in **Final Calls**, codemole has detected it and will attempt to resolve it in the definition index.

If a method appears in **Filtered out**, that is often normal for standard library or framework helpers such as `put`, `get`, `stream`, `collect`, or `filter`.

If the tool reports no useful calls, check whether the method body contains only skipped helpers, comments, or patterns that are not real direct calls.

### Common problems

**Method not found**

Possible causes:

- The definition regex does not match the signature
- The method is in a different file than expected
- The method name appears in a log string or comment and the wrong location is being matched

Checks:

- Verify the method has an explicit access modifier such as `public`, `protected`, or `private`
- Confirm the reported **Method Source** block is the real implementation
- Re-run the tool against the concrete implementation class if an interface is involved

**Calls missing from Final Calls**

Possible causes:

- The call was filtered by skip-symbols
- The call is nested inside arguments and is not treated as a direct top-level call
- The syntax does not match the current extraction patterns

Checks:

- Look for the call in **Detected Calls**
- If it is marked `[SKIPPED]`, inspect the skip-symbol database
- If it does not appear at all, verify the source line is a direct call in the matched method body

**Calls detected but still absent from the diagram**

This usually means detection worked but resolution failed later.

Possible causes:

- The target definition is outside the scanned source root
- The method exists in another package/module that was not indexed
- The method is defined in an interface or parent type and resolution picked the wrong symbol

Checks:

- Run codemole normally and inspect the `Call graph: X nodes, Y edges` line
- If `Y` is unexpectedly low, definitions are not being resolved after detection
- Verify the project root passed via `--path` includes every relevant source file

### Database management

codemole stores skip-symbols in SQLite so you can tweak traversal behavior without recompiling.

```powershell
# Open the database with your preferred SQLite client
sqlite3 ".\target\release\symbols.db"

# Show Java skip-symbols
SELECT symbol FROM skip_symbols
WHERE language_id = (SELECT id FROM languages WHERE name = 'java')
ORDER BY symbol;

# Remove a symbol so it can appear in diagrams
DELETE FROM skip_symbols
WHERE language_id = (SELECT id FROM languages WHERE name = 'java')
AND symbol = 'collect';
```

### Information to collect when debugging

If you still need help, capture these three artifacts:

```powershell
# 1. Raw debug output for the method
.\target\release\examples\debug_calls.exe "path\to\TicketCreationService.java" crudTicket

# 2. Normal codemole execution to inspect graph size
.\target\release\codemole.exe --lang java --endpoint TicketCreationXML --path "C:\path\to\project"

# 3. Generated sequence source
Get-Content "output\TicketCreationXML\diagrams\sequence.md"
```

Those three outputs are usually enough to distinguish between detection issues, skip-symbol filtering, and definition-resolution failures.

---

### Function mode — validation rules and behavior

- If `--funcion` is provided, `--endpoint` must not be present (they are mutually exclusive).
- Java:
  - When `--endpoint` is used, `--funcion`, `--clase` and `--paquete` must not be provided.
  - When `--funcion` is used, `--clase` and `--paquete` are required and `--modulo` is invalid.
  - `find_function` locates a Java file whose `package` declaration matches `--paquete`, finds the `class` named by `--clase`, and returns the requested method definition.
- Python:
  - When `--endpoint` is used, `--funcion`, `--clase` and `--modulo` must not be provided.
  - When `--funcion` is used, both `--clase` and `--modulo` are required.
  - `find_function` locates the module by file stem matching `--modulo`, finds the class definition named by `--clase` (top-level class) and the `def` inside it.
- Go:
  - When `--endpoint` is used, `--funcion` and `--modulo` must not be combined with endpoint route parameters (finder enforces separation).
  - When `--funcion` is used, `--modulo` is required and `--clase`/`--paquete` are invalid.
  - `find_function` searches Go files inside the directory matching `--modulo` and matches either free functions (`func Name(`) or receiver methods (`func (r *Type) Name(`).

Everything else in the processing pipeline (call graph construction, BFS traversal, diagram generation and rendering) remains unchanged.

---

## Output files

Each run writes the following files inside the output sub-folder (diagram sources are stored in a `diagrams/` subfolder):

| File | Contents |
|------|----------|
| `diagrams/sequence.md` | Mermaid `sequenceDiagram` source |
| `diagrams/classflow.dot` | Graphviz DOT digraph — class diagram (Java) or call-flow (Python/Go) |
| `diagrams/component.dot` | Graphviz DOT digraph — component/module diagram |
| `diagrams/dependency.dot` | Graphviz DOT digraph — package dependency diagram |
| `diagrams/dependency_metrics.txt` | Dependency coupling/cohesion metrics |
| `sequence.svg` | Rendered SVG — sequence diagram (built-in renderer) |
| `classflow.svg` | Rendered SVG — class diagram or call-flow (`dot` if available, else built-in) |
| `component.svg` | Rendered SVG — component diagram |
| `dependency.svg` | Rendered SVG — dependency diagram |
| `sequenceViewer.html` | Self-contained HTML viewer with embedded SVG and pan/zoom |
| `classflowViewer.html` | Self-contained HTML viewer with embedded SVG and pan/zoom |
| `componentViewer.html` | Self-contained HTML viewer with embedded SVG and pan/zoom |
| `dependencyViewer.html` | Self-contained HTML viewer with embedded SVG and pan/zoom |
| `svg-pan-zoom.min.js` | Pan/zoom library used by the viewers |

---

## Project structure

```
codemole/
├── Cargo.toml
├── viewer/
│   ├── viewer.html              # HTML viewer template (embedded at compile time)
│   └── svg-pan-zoom.min.js      # Pan/zoom library (embedded at compile time)
├── parsers/
│   └── java/
│       ├── pom.xml              # Maven fat-jar project (JavaParser + Gson)
│       └── src/main/java/com/codemole/
│           ├── JavaASTParser.java   # Entry point — two-pass orchestrator
│           ├── ProgramBuilder.java  # Stateful IR accumulator (Program v1.0)
│           ├── SpringDetector.java  # Spring MVC annotation detection
│           └── cfg/
│               └── CfgBuilder.java  # Per-method CFG construction
├── scripts/
│   ├── install.sh / install.ps1         # Dependencies only (Graphviz, Java, Maven, jar)
│   └── install-all.sh / install-all.ps1 # Full install (above + Rust + codemole binary)
├── src/
│   ├── main.rs                  # CLI entry; launches desktop frontend when run without args
│   ├── bin/
│   │   └── desktop.rs           # Native desktop frontend built with eframe/egui
│   ├── ir/
│   │   └── mod.rs               # Unified IR — Program (JSON contract) + Graph (runtime)
│   ├── db/
│   │   └── mod.rs               # SQLite skip-symbols DB
│   ├── finder/
│   │   ├── mod.rs               # EntryPoint type + factory + find_function dispatcher
│   │   ├── spring.rs            # Java/Spring finder (+ find_function)
│   │   ├── fastapi.rs           # Python/FastAPI finder (+ find_function)
│   │   └── gin.rs               # Go/Gin finder (+ find_function)
│   ├── parser/
│   │   ├── mod.rs               # Parser module exports
│   │   ├── java_ir.rs           # Thin bridge: launch jar → deserialize JSON → Graph
│   │   └── language.rs          # LanguageAnalyzer trait + Python/Go impls
│   ├── diagram/
│   │   ├── mod.rs
│   │   ├── sequence.rs          # Mermaid sequence diagram source builder
│   │   └── classflow.rs         # Graphviz DOT class diagram / call-flow
│   └── output/
│       ├── mod.rs               # File writer + external renderer calls
│       └── svg.rs               # Native fallback SVG renderer
├── debug.ps1                    # PowerShell helper for debug_calls
├── examples/
│   └── debug_calls.rs           # Debug helper to inspect detected calls for a Java method
└── README.md
```

---

## Limitations

- Regex-based parsing — does not handle heavily minified, generated, or macro-expanded code.
- Indirect calls (reflection, dynamic dispatch beyond interface resolution) are not traced.
- Multi-module Maven/Gradle projects: implementations in a different module/jar than the interface may fall back to the interface location.
- Python: only top-level `def`/`async def` functions and class methods defined in top-level classes are traced by function-mode heuristics.
- Go: most free functions and simple receiver methods are matched; complex build systems or generated code may not be discovered by the simple directory-based module match.
