use clap::{ArgGroup, Parser};
use std::process;

mod db;
mod finder;
mod parser;
mod diagram;
mod output;

/// codemole — traces an API endpoint through your codebase and generates diagrams.
#[derive(Parser, Debug)]
#[clap(group(
    ArgGroup::new("mode")
        .required(true)
        .args(["endpoint", "funcion"])
))]
#[command(
        name = "codemole",
        version = "0.1.1",
        about = "Traces an API endpoint through your source code and generates sequence and class/flow diagrams.",
        long_about = r#"codemole takes a framework language and either an endpoint path or a function name,
finds the handler or function in your source code, traverses its call graph, and outputs Mermaid diagrams (.md)
plus native SVG files — no external tools required.

Supported languages/frameworks:
    java    -> Spring Boot (@GetMapping, @PostMapping, @RequestMapping, ...)
    python  -> FastAPI (@app.get, @router.post, ...)
    go      -> Gin (r.GET, r.POST, group.DELETE, ...)

Modes and new flags:
    --endpoint: trace an HTTP handler (endpoint mode).
    --funcion: trace a specific function/method by name (function mode).
    Companion flags for function mode: --clase, --paquete, --modulo (language-dependent).

Examples (endpoint mode):
    codemole --lang java   --endpoint /api/users --path ./my-spring-project
    codemole --lang python --endpoint /items/{id} --path ./my-fastapi-project
    codemole --lang go     --endpoint /health --path ./my-gin-project

Examples (function mode):
    codemole --lang java --funcion getUserById --clase UserController --paquete com.example.users --path ./my-spring-project
    codemole --lang python --funcion get_item --clase ItemService --modulo services.items --path ./my-fastapi-project
    codemole --lang go --funcion GetHealth --modulo handlers --path ./my-gin-project"#
)]
struct Cli {
    /// Language / framework: java | python | go
    #[arg(long, value_parser = ["java", "python", "go"])]
    lang: String,

    /// HTTP endpoint to trace, e.g. /api/users or /items/{id} (endpoint mode).
    /// Cannot be combined with --funcion, --clase, --paquete or --modulo.
    #[arg(long, conflicts_with_all = ["funcion", "clase", "paquete", "modulo"])]
    endpoint: Option<String>,

    /// Function/method name to trace (function mode).
    /// Java:   also requires --clase and --paquete.
    /// Python: also requires --clase and --modulo.
    /// Go:     also requires --modulo.
    #[arg(long, conflicts_with = "endpoint")]
    funcion: Option<String>,

    /// Class name — required with --funcion for Java and Python.
    #[arg(long, requires = "funcion")]
    clase: Option<String>,

    /// Package name — required with --funcion for Java.
    #[arg(long, requires = "funcion")]
    paquete: Option<String>,

    /// Module name — required with --funcion for Python and Go.
    #[arg(long, requires = "funcion")]
    modulo: Option<String>,

    /// Root directory of the source code to analyse (default: current directory)
    #[arg(long, default_value = ".")]
    path: String,

    /// Base output directory (default: OS temp dir).
    /// A sub-folder named after the endpoint or function is always created inside.
    #[arg(long, default_value_t = default_output_path())]
    output: String,

    /// Path to the skip-symbols SQLite database.
    /// Created and seeded with defaults on first run.
    /// Use any SQLite tool to add/remove symbols without recompiling.
    #[arg(long, default_value_t = default_db_path())]
    db: String,

    /// Path to the java-parser JAR (used for CFG-enriched Java sequence diagrams).
    /// Not required for endpoint discovery but used to enrich sequence output for Java when present.
    #[arg(long, default_value_t = default_jar_path())]
    java_parser: String,
}

fn default_output_path() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

fn default_db_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("symbols.db")))
        .and_then(|p| p.to_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| "./symbols.db".to_string())
}

fn default_jar_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("java-parser.jar")))
        .and_then(|p| p.to_str().map(|s| s.to_owned()))
        .unwrap_or_else(|| "./java-parser.jar".to_string())
}

fn main() {
    let cli = Cli::parse();

    // 1. Initialise the skip-symbol database
    let conn = match db::init(&cli.db) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: cannot open database '{}': {}", cli.db, e);
            process::exit(1);
        }
    };
    let skip_symbols = db::load_skip_symbols(&conn, &cli.lang);
    println!(
        "Loaded {} skip-symbols for '{}' from '{}'",
        skip_symbols.len(),
        cli.lang,
        cli.db
    );

    // 2. Validate parameters and find the entry point
    let (entry, label) = resolve_entry_point(&cli);

    println!(
        "Found '{}' → {}.{} ({}:{})",
        label, entry.class, entry.method, entry.file, entry.line
    );
    if let Some(ref iface) = entry.interface_class {
        println!("  (implementation of interface {})", iface);
    }

    // 3. Traverse the call graph with BFS
    let graph = parser::build_call_graph(&cli.lang, &cli.path, &entry, skip_symbols);

    println!(
        "Call graph: {} nodes, {} edges",
        graph.nodes.len(),
        graph.edges.len()
    );

    // 4. Generate diagrams and SVG
    // Build the output sub-folder: base_dir / label_slug
    let slug = label
        .trim_matches('/')
        .replace('/', "_")
        .replace('{', "")
        .replace('}', "")
        .replace('.', "_");
    let out_dir = std::path::Path::new(&cli.output).join(&slug);
    let out_dir_str = out_dir.to_string_lossy();

    output::write_diagrams(&cli.lang, &label, &cli.path, &graph, &out_dir_str, &cli.java_parser);

    println!("Output written to '{}':", out_dir.display());
    println!("  sequence.puml    sequence.svg    sequenceViewer.html");
    println!("  classflow.dot    classflow.svg   classflowViewer.html");
    println!("  component.dot    component.svg   componentViewer.html");
    println!("  dependency.dot   dependency.svg  dependencyViewer.html");
}

/// Resolves CLI arguments into an `EntryPoint` and a human-readable label.
/// Clap has already enforced that exactly one of --endpoint/--funcion is present
/// and that --clase/--paquete/--modulo are not combined with --endpoint.
fn resolve_entry_point(cli: &Cli) -> (finder::EntryPoint, String) {
    if cli.funcion.is_some() {
        let funcion = cli.funcion.as_deref().unwrap();
        resolve_function_entry(cli, funcion)
    } else {
        let endpoint = cli.endpoint.as_deref().unwrap();
        let entry = match finder::find_endpoint(&cli.lang, endpoint, &cli.path) {
            Some(e) => e,
            None => {
                eprintln!(
                    "error: endpoint '{}' not found in '{}' for lang '{}'",
                    endpoint, cli.path, cli.lang
                );
                process::exit(1);
            }
        };
        (entry, endpoint.to_string())
    }
}

/// Handles the --funcion path: validates required companion params per language
/// and delegates to `finder::find_function`.
fn resolve_function_entry(cli: &Cli, funcion: &str) -> (finder::EntryPoint, String) {
    match cli.lang.as_str() {
        "java" => {
            let clase = cli.clase.as_deref().unwrap_or_else(|| {
                eprintln!("error: --clase is required with --funcion for lang 'java'");
                process::exit(1);
            });
            let paquete = cli.paquete.as_deref().unwrap_or_else(|| {
                eprintln!("error: --paquete is required with --funcion for lang 'java'");
                process::exit(1);
            });
            if cli.modulo.is_some() {
                eprintln!("error: --modulo is not valid for lang 'java'; use --paquete instead");
                process::exit(1);
            }
            let entry = match finder::find_function(&cli.lang, funcion, Some(clase), Some(paquete), &cli.path) {
                Some(e) => e,
                None => {
                    eprintln!(
                        "error: function '{}' not found in class '{}' (package '{}') within '{}'",
                        funcion, clase, paquete, cli.path
                    );
                    process::exit(1);
                }
            };
            let label = format!("{}.{}#{}", paquete, clase, funcion);
            (entry, label)
        }
        "python" => {
            let clase = cli.clase.as_deref().unwrap_or_else(|| {
                eprintln!("error: --clase is required with --funcion for lang 'python'");
                process::exit(1);
            });
            let modulo = cli.modulo.as_deref().unwrap_or_else(|| {
                eprintln!("error: --modulo is required with --funcion for lang 'python'");
                process::exit(1);
            });
            if cli.paquete.is_some() {
                eprintln!("error: --paquete is not valid for lang 'python'; use --modulo instead");
                process::exit(1);
            }
            let entry = match finder::find_function(&cli.lang, funcion, Some(clase), Some(modulo), &cli.path) {
                Some(e) => e,
                None => {
                    eprintln!(
                        "error: function '{}' not found in class '{}' (module '{}') within '{}'",
                        funcion, clase, modulo, cli.path
                    );
                    process::exit(1);
                }
            };
            let label = format!("{}.{}#{}", modulo, clase, funcion);
            (entry, label)
        }
        "go" => {
            let modulo = cli.modulo.as_deref().unwrap_or_else(|| {
                eprintln!("error: --modulo is required with --funcion for lang 'go'");
                process::exit(1);
            });
            if cli.clase.is_some() {
                eprintln!("error: --clase is not valid for lang 'go'");
                process::exit(1);
            }
            if cli.paquete.is_some() {
                eprintln!("error: --paquete is not valid for lang 'go'; use --modulo instead");
                process::exit(1);
            }
            let entry = match finder::find_function(&cli.lang, funcion, None, Some(modulo), &cli.path) {
                Some(e) => e,
                None => {
                    eprintln!(
                        "error: function '{}' not found in module '{}' within '{}'",
                        funcion, modulo, cli.path
                    );
                    process::exit(1);
                }
            };
            let label = format!("{}#{}", modulo, funcion);
            (entry, label)
        }
        _ => {
            eprintln!("error: unsupported lang '{}'", cli.lang);
            process::exit(1);
        }
    }
}
