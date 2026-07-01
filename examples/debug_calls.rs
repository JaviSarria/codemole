/// Debug helper to print what calls are being extracted from a method body
use regex::Regex;
use std::fs;
use std::collections::HashSet;

fn main() {
    if std::env::args().len() < 2 {
        eprintln!("Usage: cargo run --example debug_calls <java-file> <method-name> [--show-skip]");
        eprintln!("Example: cargo run --example debug_calls src/MyService.java crudTicket");
        eprintln!("Use --show-skip to see which calls are filtered out by skip-symbols");
        return;
    }

    let file_path = std::env::args().nth(1).unwrap();
    let method_name = std::env::args().nth(2).unwrap();
    let show_skip = std::env::args().nth(3).map(|s| s == "--show-skip").unwrap_or(false);

    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            return;
        }
    };

    let lines: Vec<&str> = content.lines().collect();

    // Find method definition
    let re_def = Regex::new(
        r"(?:public|protected|private)\s+(?:static\s+)?(?:final\s+)?(?:synchronized\s+)?(?:abstract\s+)?(?:native\s+)?.*?(\w+)\s*\(",
    ).unwrap();

    let mut method_line_idx = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(cap) = re_def.captures(line) {
            if cap[1] == method_name {
                method_line_idx = Some(i);
                break;
            }
        }
    }

    if method_line_idx.is_none() {
        eprintln!("Method '{}' not found", method_name);
        return;
    }

    let start = method_line_idx.unwrap();
    println!("Found method '{}' at line {}\n", method_name, start + 1);
    println!("--- Method Source ---");
    
    // Extract body (find matching braces)
    let mut depth = 0i32;
    let mut body_lines = Vec::new();
    let mut started = false;
    
    for line in &lines[start..] {
        for c in line.chars() {
            match c {
                '{' => { depth += 1; started = true; }
                '}' => { depth -= 1; }
                _ => {}
            }
        }
        body_lines.push(*line);
        println!("{}", line);
        if started && depth == 0 {
            break;
        }
    }

    println!("\n--- Java Skip Symbols (from skip-symbols.db) ---");
    print_skip_symbols();

    println!("\n--- Detected Calls (BEFORE filtering) ---");
    
    let re_call = Regex::new(r"\b(\w+)\s*\(").unwrap();
    let re_qualified = Regex::new(r"\b([a-z]\w*)\.([a-zA-Z]\w*)\s*\(").unwrap();

    let mut all_calls = std::collections::HashSet::new();
    let mut qualified_calls = Vec::new();
    let mut unqualified_calls = Vec::new();

    for line in &body_lines {
        // Skip comments
        let line = if let Some(idx) = line.find("//") {
            &line[..idx]
        } else {
            line
        };

        // Extract qualified calls (obj.method())
        for cap in re_qualified.captures_iter(line) {
            let method = &cap[2];
            all_calls.insert(method.to_string());
            qualified_calls.push(method.to_string());
        }

        // Extract unqualified calls (method())
        for cap in re_call.captures_iter(line) {
            let method = &cap[1];
            if !["if", "for", "while", "switch", "catch", "synchronized", "try"].contains(&method) {
                all_calls.insert(method.to_string());
                unqualified_calls.push(method.to_string());
            }
        }
    }

    let skip_symbols = get_java_skip_symbols();
    
    println!("\nQualified calls (obj.method()):");
    for call in &qualified_calls {
        let status = if skip_symbols.contains(call.as_str()) { 
            "  [SKIPPED]" 
        } else { 
            "  [KEPT]" 
        };
        println!("{} {}", status, call);
    }
    
    println!("\nUnqualified calls (method()):");
    for call in &unqualified_calls {
        let status = if skip_symbols.contains(call.as_str()) { 
            "  [SKIPPED]" 
        } else { 
            "  [KEPT]" 
        };
        println!("{} {}", status, call);
    }

    println!("\n--- Final Calls (AFTER filtering) ---");
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    
    for call in &all_calls {
        if skip_symbols.contains(call.as_str()) {
            skipped.push(call.clone());
        } else if call != &method_name {
            kept.push(call.clone());
        }
    }
    
    if kept.is_empty() {
        println!("(no calls - all were filtered or skipped!)");
    } else {
        println!("Kept for traversal:");
        for call in &kept {
            println!("  ✓ {}", call);
        }
    }
    
    if !skipped.is_empty() {
        println!("\nFiltered out (in skip-symbols):");
        for call in &skipped {
            println!("  ✗ {}", call);
        }
    }
}

fn get_java_skip_symbols() -> HashSet<&'static str> {
    vec![
        // keywords
        "if", "else", "for", "while", "do", "switch", "case", "try", "catch",
        "finally", "return", "throw", "new", "assert", "synchronized",
        "instanceof", "super", "this",
        // object_methods
        "toString", "equals", "hashCode", "getClass", "notify", "notifyAll",
        "wait", "clone", "finalize",
        // string_methods
        "length", "charAt", "indexOf", "lastIndexOf", "substring", "split",
        "trim", "strip", "stripLeading", "stripTrailing", "replace",
        "replaceAll", "replaceFirst", "matches", "startsWith", "endsWith",
        "contains", "toUpperCase", "toLowerCase", "toCharArray", "intern",
        "formatted", "isBlank", "isEmpty", "compareTo", "compareToIgnoreCase",
        "concat", "format",
        // collection_methods
        "get", "put", "add", "addAll", "remove", "removeAll", "clear", "size",
        "containsKey", "containsValue", "keySet", "values", "entrySet", 
        "iterator", "listIterator", "subList", "toArray", "sort",
        "stream", "parallelStream", "forEach", "removeIf",
        "computeIfAbsent", "computeIfPresent", "getOrDefault", "putIfAbsent",
        "merge", "compute",
        // stream_functional
        "collect", "map", "flatMap", "filter", "reduce", "findFirst",
        "findAny", "anyMatch", "allMatch", "noneMatch", "count", "min", "max",
        "sum", "average", "distinct", "sorted", "peek", "limit", "skip",
        "toList", "ofNullable", "of", "empty", "orElse", "orElseGet",
        "orElseThrow", "ifPresent", "ifPresentOrElse", "isPresent",
        // io_methods
        "read", "write", "flush", "close", "readLine", "println", "print",
        "printf", "readAllBytes", "readString", "writeString", "transferTo",
        "next", "hasNext",
    ].into_iter().collect()
}

fn print_skip_symbols() {
    let skip = get_java_skip_symbols();
    let mut sorted: Vec<_> = skip.iter().collect();
    sorted.sort();
    
    println!("Total skip symbols: {}", skip.len());
    println!("Categories:");
    println!("  - keywords (if, for, while, etc.)");
    println!("  - stdlib constructors (String, ArrayList, etc.)");
    println!("  - object methods (toString, equals, etc.)");
    println!("  - string methods (charAt, split, etc.)");
    println!("  - collection methods (add, get, stream, etc.)");
    println!("  - stream/functional (collect, map, filter, etc.)");
    println!("  - io methods (read, write, println, etc.)");
    println!("\nRun with --show-skip to see the full list in call analysis.");
}
