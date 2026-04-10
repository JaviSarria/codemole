use clap::Parser;
use std::process;

mod db;
mod finder;
mod parser;
mod diagram;
mod output;

/// code-mole — traces an API endpoint through your codebase and generates diagrams.
#[derive(Parser, Debug)]
#[command(
    name = "code-mole",
    version = "0.1.0",
    about = "Traces an API endpoint through your codebase and generates sequence and class/flow diagrams.",
    long_about = "code-mole takes a framework language and an endpoint path, finds the handler \
in your source code, traverses its call graph, and outputs Mermaid diagrams (.md) \
plus native SVG files — no external tools required.\n\n\
Supported languages/frameworks:\n  \
java    → Spring Boot (@GetMapping, @PostMapping, @RequestMapping, ...)\n  \
python  → FastAPI (@app.get, @router.post, ...)\n  \
go      → Gin (r.GET, r.POST, group.DELETE, ...)\n\n\
Examples:\n  \
code-mole --lang java   --endpoint /api/users --path ./my-spring-project\n  \
code-mole --lang python --endpoint /items/{id} --path ./my-fastapi-project\n  \
code-mole --lang go     --endpoint /health --path ./my-gin-project"
)]
struct Cli {
    /// Language / framework: java | python | go
    #[arg(long, value_parser = ["java", "python", "go"])]
    lang: String,

    /// API endpoint to trace (e.g. /api/users or /items/{id})
    #[arg(long)]
    endpoint: String,

    /// Root directory of the source code to analyse (default: current directory)
    #[arg(long, default_value = ".")]
    path: String,

    /// Base output directory (default: OS temp dir).
    /// A sub-folder named after the endpoint is always created inside.
    #[arg(long, default_value_t = default_output_path())]
    output: String,

    /// Path to the skip-symbols SQLite database.
    /// Created and seeded with defaults on first run.
    /// Use any SQLite tool to add/remove symbols without recompiling.
    #[arg(long, default_value_t = default_db_path())]
    db: String,
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

    // 2. Find the endpoint handler in the codebase
    let entry = match finder::find_endpoint(&cli.lang, &cli.endpoint, &cli.path) {
        Some(e) => e,
        None => {
            eprintln!(
                "error: endpoint '{}' not found in '{}' for lang '{}'",
                cli.endpoint, cli.path, cli.lang
            );
            process::exit(1);
        }
    };

    println!(
        "Found endpoint '{}' → {}.{} ({}:{})",
        cli.endpoint, entry.class, entry.method, entry.file, entry.line
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
    // Build the output sub-folder: base_dir / endpoint_slug
    // e.g. /tmp + /api/users/{id} → /tmp/api_users_id
    let endpoint_slug = cli.endpoint
        .trim_matches('/')
        .replace('/', "_")
        .replace('{', "")
        .replace('}', "");
    let out_dir = std::path::Path::new(&cli.output).join(&endpoint_slug);
    let out_dir_str = out_dir.to_string_lossy();

    output::write_diagrams(&cli.lang, &cli.endpoint, &graph, &out_dir_str);

    println!("Output written to '{}':", out_dir.display());
    println!("  sequence.puml  sequence.svg  sequenceViewer.html");
    println!("  classflow.dot  classflow.svg classflowViewer.html");
}
