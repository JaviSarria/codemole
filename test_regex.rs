// Test regex patterns for debugging endpoint matching
use regex::Regex;

fn main() {
    // The current regex pattern
    let re_mapping = Regex::new(
        r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*(?:\([^)]*?"([^"]*)"[^)]*?\)|\(\s*\))?"#,
    )
    .unwrap();

    // Test cases from the user's code
    let test_cases = vec![
        r#"@PostMapping(value = "TicketCreationXML", consumes = { MediaType.APPLICATION_XML_VALUE,"#,
        r#"@PostMapping(value = "TicketCreationXML")"#,
        r#"@PostMapping("TicketCreationXML")"#,
        r#"@RequestMapping("/api/tickets")"#,
        r#"@PostMapping(value="/items/{id}")"#,
        r#"@GetMapping"#,
    ];

    println!("Testing regex pattern:");
    println!("{}", r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*(?:\([^)]*?"([^"]*)"[^)]*?\)|\(\s*\))?"#);
    println!("\n");

    for test in test_cases {
        println!("Input: {}", test);
        if let Some(cap) = re_mapping.captures(test) {
            let mapping_type = cap.get(1).map(|m| m.as_str()).unwrap_or("N/A");
            let endpoint = cap.get(2).map(|m| m.as_str()).unwrap_or("<EMPTY>");
            println!("  ✓ Matched - Type: {}, Endpoint: {}", mapping_type, endpoint);
        } else {
            println!("  ✗ NO MATCH");
        }
        println!();
    }
}
