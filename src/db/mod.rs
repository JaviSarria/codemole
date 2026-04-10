/// Database module — SQLite-backed skip-symbol registry.
///
/// Schema (normalised, 3NF):
///
///   languages        id, name                       ("java", "python", "go")
///   skip_categories  id, name                       ("keywords", "stdlib", …)
///   skip_symbols     id, language_id, category_id, symbol
///
/// The DB is created and seeded with built-in defaults on the very first run.
/// After that users can manage symbols with any SQLite tool (DB Browser,
/// sqlite3 CLI, etc.) without recompiling the tool.
use std::collections::HashSet;
use rusqlite::{Connection, Result, params};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Open (or create) the DB at `db_path`, initialise the schema and seed
/// built-in defaults if the `skip_symbols` table is empty.
pub fn init(db_path: &str) -> Result<Connection> {
    // Create parent dirs if needed
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(db_path)?;

    // Enable foreign keys
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    create_schema(&conn)?;

    // Seed only when the table is empty (first run)
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM skip_symbols",
        [],
        |row| row.get(0),
    )?;
    if count == 0 {
        seed_defaults(&conn)?;
        eprintln!(
            "info: DB initialised with default skip-symbols at '{}'",
            db_path
        );
    }

    Ok(conn)
}

/// Load the set of symbol names to skip during BFS for the given language.
pub fn load_skip_symbols(conn: &Connection, lang: &str) -> HashSet<String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.symbol
             FROM skip_symbols s
             JOIN languages l ON l.id = s.language_id
             WHERE l.name = ?1",
        )
        .expect("prepare skip_symbols query");

    stmt.query_map(params![lang], |row| row.get::<_, String>(0))
        .expect("query skip_symbols")
        .filter_map(|r| r.ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

fn create_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS languages (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT    NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS skip_categories (
            id   INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT    NOT NULL UNIQUE
        );

        CREATE TABLE IF NOT EXISTS skip_symbols (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            language_id INTEGER NOT NULL
                REFERENCES languages(id) ON DELETE CASCADE ON UPDATE CASCADE,
            category_id INTEGER NOT NULL
                REFERENCES skip_categories(id) ON DELETE CASCADE ON UPDATE CASCADE,
            symbol      TEXT NOT NULL,
            UNIQUE(language_id, symbol)
        );

        CREATE INDEX IF NOT EXISTS idx_skip_lang ON skip_symbols(language_id);
    ")
}

// ---------------------------------------------------------------------------
// Seed helpers
// ---------------------------------------------------------------------------

fn seed_defaults(conn: &Connection) -> Result<()> {
    // Insert languages
    for lang in &["java", "python", "go"] {
        conn.execute(
            "INSERT OR IGNORE INTO languages(name) VALUES (?1)",
            params![lang],
        )?;
    }

    // Insert categories
    for cat in &[
        "keywords",
        "stdlib_constructors",
        "object_methods",
        "string_methods",
        "collection_methods",
        "stream_functional",
        "io_methods",
        "math_methods",
        "number_parsing",
        "system_thread",
        "logging",
        "framework_helpers",
        "builder_pattern",
        "datetime_methods",
        "reflection",
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO skip_categories(name) VALUES (?1)",
            params![cat],
        )?;
    }

    seed_java(conn)?;
    seed_python(conn)?;
    seed_go(conn)?;

    Ok(())
}

/// Insert a batch of symbols for a given language and category.
fn insert_symbols(
    conn: &Connection,
    lang: &str,
    category: &str,
    symbols: &[&str],
) -> Result<()> {
    let lang_id: i64 = conn.query_row(
        "SELECT id FROM languages WHERE name = ?1",
        params![lang],
        |row| row.get(0),
    )?;
    let cat_id: i64 = conn.query_row(
        "SELECT id FROM skip_categories WHERE name = ?1",
        params![category],
        |row| row.get(0),
    )?;

    for sym in symbols {
        conn.execute(
            "INSERT OR IGNORE INTO skip_symbols(language_id, category_id, symbol)
             VALUES (?1, ?2, ?3)",
            params![lang_id, cat_id, sym],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Java seed data
// ---------------------------------------------------------------------------

fn seed_java(conn: &Connection) -> Result<()> {
    insert_symbols(conn, "java", "keywords", &[
        "if", "else", "for", "while", "do", "switch", "case", "try", "catch",
        "finally", "return", "throw", "new", "assert", "synchronized",
        "instanceof", "super", "this",
    ])?;

    insert_symbols(conn, "java", "stdlib_constructors", &[
        "StringBuilder", "StringBuffer", "String", "Integer", "Long", "Double",
        "Float", "Boolean", "Character", "Byte", "Short", "BigDecimal",
        "BigInteger", "Date", "LocalDate", "LocalDateTime", "LocalTime",
        "ZonedDateTime", "OffsetDateTime", "Instant", "Duration", "Period",
        "Calendar", "GregorianCalendar", "ArrayList", "LinkedList", "HashMap",
        "LinkedHashMap", "TreeMap", "HashSet", "LinkedHashSet", "TreeSet",
        "ArrayDeque", "PriorityQueue", "Optional", "AtomicInteger", "AtomicLong",
        "AtomicBoolean", "Thread", "Runnable", "Exception", "RuntimeException",
        "IllegalArgumentException", "IllegalStateException",
        "NullPointerException", "UnsupportedOperationException",
        "IOException", "FileNotFoundException", "ObjectMapper", "TypeReference",
        "ResponseEntity", "HttpStatus",
    ])?;

    insert_symbols(conn, "java", "object_methods", &[
        "toString", "equals", "hashCode", "getClass", "notify", "notifyAll",
        "wait", "clone", "finalize",
    ])?;

    insert_symbols(conn, "java", "string_methods", &[
        "length", "charAt", "indexOf", "lastIndexOf", "substring", "split",
        "trim", "strip", "stripLeading", "stripTrailing", "replace",
        "replaceAll", "replaceFirst", "matches", "startsWith", "endsWith",
        "contains", "toUpperCase", "toLowerCase", "toCharArray", "intern",
        "formatted", "isBlank", "isEmpty", "compareTo", "compareToIgnoreCase",
        "concat", "format",
    ])?;

    insert_symbols(conn, "java", "collection_methods", &[
        "get", "put", "add", "addAll", "remove", "removeAll", "clear", "size",
        "contains", "containsKey", "containsValue", "keySet", "values",
        "entrySet", "iterator", "listIterator", "subList", "toArray", "sort",
        "stream", "parallelStream", "forEach", "removeIf",
        "computeIfAbsent", "computeIfPresent", "getOrDefault", "putIfAbsent",
        "merge", "compute",
    ])?;

    insert_symbols(conn, "java", "stream_functional", &[
        "collect", "map", "flatMap", "filter", "reduce", "findFirst",
        "findAny", "anyMatch", "allMatch", "noneMatch", "count", "min", "max",
        "sum", "average", "distinct", "sorted", "peek", "limit", "skip",
        "toList", "ofNullable", "of", "empty", "orElse", "orElseGet",
        "orElseThrow", "ifPresent", "ifPresentOrElse", "isPresent",
    ])?;

    insert_symbols(conn, "java", "io_methods", &[
        "read", "write", "flush", "close", "readLine", "println", "print",
        "printf", "readAllBytes", "readString", "writeString", "transferTo",
        "next", "hasNext",
    ])?;

    insert_symbols(conn, "java", "math_methods", &[
        "abs", "round", "ceil", "floor", "sqrt", "pow", "log", "log10",
        "exp", "sin", "cos", "tan", "random", "signum", "copySign",
    ])?;

    insert_symbols(conn, "java", "number_parsing", &[
        "parseInt", "parseLong", "parseDouble", "parseFloat", "parseByte",
        "parseShort", "valueOf", "intValue", "longValue", "doubleValue",
        "floatValue", "toBinaryString", "toHexString", "toOctalString",
        "compareTo", "compare",
    ])?;

    insert_symbols(conn, "java", "system_thread", &[
        "currentTimeMillis", "nanoTime", "arraycopy", "exit", "gc",
        "sleep", "join", "start", "run", "interrupt", "isInterrupted",
        "yield", "getName", "setName", "getId",
    ])?;

    insert_symbols(conn, "java", "reflection", &[
        "newInstance", "getDeclaredMethod", "getDeclaredField", "invoke",
        "cast", "isInstance", "isAssignableFrom",
    ])?;

    insert_symbols(conn, "java", "builder_pattern", &[
        "append", "insert", "delete", "deleteCharAt", "reverse", "setCharAt",
        "setLength", "capacity", "ensureCapacity", "build", "builder",
        "toBuilder", "or",
    ])?;

    insert_symbols(conn, "java", "datetime_methods", &[
        "now", "parse", "plus", "minus", "with", "getYear", "getMonth",
        "getDayOfMonth", "getHour", "getMinute", "getSecond", "toLocalDate",
        "toLocalTime", "toInstant", "atZone", "atOffset", "isBefore",
        "isAfter", "isEqual", "between", "until", "from",
    ])?;

    insert_symbols(conn, "java", "logging", &[
        "error", "warn", "info", "debug", "trace", "log", "severe", "warning",
        "fine", "finer", "finest", "entering", "exiting", "throwing",
    ])?;

    insert_symbols(conn, "java", "framework_helpers", &[
        "getBody", "getStatusCode", "getHeaders", "ok", "status", "badRequest",
        "notFound", "accepted", "noContent", "created", "found", "header",
        "body", "getBean", "getApplicationContext",
        "save", "saveAll", "findById", "findAll", "findAllById", "deleteById",
        "deleteAll", "existsById", "count",
        "getMessage", "getCause", "getLocalizedMessage", "printStackTrace",
        "getStackTrace", "initCause",
        "max", "min",
    ])?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Python seed data
// ---------------------------------------------------------------------------

fn seed_python(conn: &Connection) -> Result<()> {
    insert_symbols(conn, "python", "keywords", &[
        "if", "else", "elif", "for", "while", "with", "try", "except",
        "finally", "return", "yield", "raise", "assert", "lambda", "pass",
        "break", "continue", "import", "from", "as", "in", "not", "and",
        "or", "is", "del", "global", "nonlocal", "class", "def", "async",
        "await",
    ])?;

    insert_symbols(conn, "python", "stdlib_constructors", &[
        "list", "dict", "set", "tuple", "str", "int", "float", "bool",
        "type", "object", "Exception", "ValueError", "TypeError", "KeyError",
        "IndexError", "AttributeError", "RuntimeError", "StopIteration",
        "OSError", "IOError",
    ])?;

    insert_symbols(conn, "python", "io_methods", &[
        "print", "open", "read", "write", "close", "input",
    ])?;

    insert_symbols(conn, "python", "collection_methods", &[
        "append", "extend", "insert", "remove", "pop", "clear", "copy",
        "index", "sort", "reverse", "update", "keys", "values", "items",
        "get", "setdefault", "fromkeys", "add", "discard", "difference",
        "union", "intersection", "symmetric_difference", "issubset",
        "issuperset",
    ])?;

    insert_symbols(conn, "python", "string_methods", &[
        "join", "split", "splitlines", "strip", "lstrip", "rstrip", "replace",
        "startswith", "endswith", "upper", "lower", "title", "capitalize",
        "casefold", "encode", "decode", "find", "index", "count",
        "format_map", "expandtabs", "center", "ljust", "rjust", "zfill",
        "isdigit", "isalpha", "isalnum", "isspace", "isupper", "islower",
        "istitle", "format",
    ])?;

    insert_symbols(conn, "python", "stdlib_constructors", &[
        "len", "range", "enumerate", "zip", "map", "filter",
        "isinstance", "issubclass", "hasattr", "getattr", "setattr",
        "delattr", "callable", "iter", "next", "repr", "hash", "id",
        "dir", "vars", "locals", "globals", "sorted", "reversed", "any",
        "all", "sum", "min", "max", "abs", "round", "hex", "oct", "bin",
        "chr", "ord", "pow", "divmod",
    ])?;

    insert_symbols(conn, "python", "object_methods", &[
        "super", "classmethod", "staticmethod", "property",
        "NotImplemented", "Ellipsis",
    ])?;

    insert_symbols(conn, "python", "framework_helpers", &[
        "HTTPException", "JSONResponse", "Response", "Request", "Depends",
        "Query", "Path", "Body", "Header", "Cookie", "File", "Form",
        "status_code", "detail",
    ])?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Go seed data
// ---------------------------------------------------------------------------

fn seed_go(conn: &Connection) -> Result<()> {
    insert_symbols(conn, "go", "keywords", &[
        "if", "else", "for", "range", "switch", "case", "select", "go",
        "defer", "return", "break", "continue", "goto", "fallthrough",
    ])?;

    insert_symbols(conn, "go", "stdlib_constructors", &[
        "make", "new", "len", "cap", "append", "copy", "delete", "close",
        "panic", "recover", "print", "println", "real", "imag", "complex",
    ])?;

    insert_symbols(conn, "go", "io_methods", &[
        "Open", "Create", "ReadFile", "WriteFile", "MkdirAll", "Mkdir",
        "Remove", "RemoveAll", "Rename", "Stat", "Lstat", "IsNotExist",
        "IsExist", "ReadAll", "WriteString", "Copy",
        "NewReader", "NewWriter", "ReadString", "Flush",
        "NewScanner", "Scan", "Text", "Bytes",
    ])?;

    insert_symbols(conn, "go", "logging", &[
        "Println", "Printf", "Fprintf", "Sprintf", "Errorf", "Sscanf",
        "Scanf", "Fprintln", "Fprint", "Print", "Sprint", "Sprintln",
        "Scan", "Fscan",
        "Fatal", "Fatalf", "Fatalln", "Log", "Logf", "Logln",
        "Panic", "Panicf", "Panicln", "New", "Error",
    ])?;

    insert_symbols(conn, "go", "string_methods", &[
        "Contains", "HasPrefix", "HasSuffix", "Join", "Split", "SplitN",
        "TrimSpace", "Trim", "TrimLeft", "TrimRight", "TrimPrefix",
        "TrimSuffix", "ToUpper", "ToLower", "Replace", "ReplaceAll",
        "Index", "LastIndex", "Count", "Repeat", "EqualFold", "Fields",
        "Map", "NewReplacer", "Title", "ContainsAny", "IndexByte",
        "IndexRune",
    ])?;

    insert_symbols(conn, "go", "number_parsing", &[
        "Itoa", "Atoi", "ParseInt", "ParseFloat", "ParseBool", "ParseUint",
        "FormatInt", "FormatFloat", "FormatBool", "FormatUint",
        "AppendInt", "AppendFloat",
    ])?;

    insert_symbols(conn, "go", "datetime_methods", &[
        "Now", "Since", "Until", "Sleep", "After", "Tick", "NewTicker",
        "NewTimer", "Stop", "Reset", "Parse", "Format", "Date", "Unix",
        "Add", "Sub",
    ])?;

    insert_symbols(conn, "go", "math_methods", &[
        "Abs", "Ceil", "Floor", "Round", "Sqrt", "Pow", "Log", "Log2",
        "Log10", "Mod", "Max", "Min", "Inf", "IsNaN", "IsInf", "Hypot",
    ])?;

    insert_symbols(conn, "go", "system_thread", &[
        "Background", "TODO", "WithCancel", "WithDeadline", "WithTimeout",
        "WithValue", "Value", "Done", "Err", "Deadline",
        "Lock", "Unlock", "RLock", "RUnlock", "Done", "Wait",
        "Store", "Load", "Swap", "CompareAndSwap",
    ])?;

    insert_symbols(conn, "go", "io_methods", &[
        "Marshal", "MarshalIndent", "Unmarshal", "NewDecoder", "NewEncoder",
        "Decode", "Encode", "Token",
    ])?;

    insert_symbols(conn, "go", "framework_helpers", &[
        "JSON", "String", "HTML", "XML", "File", "Data", "Redirect",
        "Status", "Header", "Cookie", "SetCookie", "Next", "Abort",
        "AbortWithStatus", "AbortWithStatusJSON", "ShouldBindJSON",
        "ShouldBind", "BindJSON", "Bind", "Param", "Query", "DefaultQuery",
        "PostForm", "DefaultPostForm", "Set", "Get", "MustGet",
    ])?;

    Ok(())
}
