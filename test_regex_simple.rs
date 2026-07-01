use regex::Regex;

fn main() {
    let re_mapping = Regex::new(
        r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*(?:\([^)]*?"([^"]*)")?"#,
    ).unwrap();

    let test_lines = vec![
        r#"@PostMapping(value = "TicketCreationXML", consumes = { MediaType.APPLICATION_XML_VALUE,"#,
        r#"@PostMapping("TicketCreationXML")"#,
        r#"@GetMapping("/api/users")"#,
        r#"@RequestMapping(value = "/items/{id}")"#,
    ];

    for line in test_lines {
        println!("Testing: {}", line);
        if let Some(cap) = re_mapping.captures(line) {
            let mapping_type = cap.get(1).map(|m| m.as_str()).unwrap_or("N/A");
            let endpoint = cap.get(2).map(|m| m.as_str()).unwrap_or("<EMPTY>");
            println!("  ✓ Matched - Type: {}, Endpoint: {}", mapping_type, endpoint);
        } else {
            println!("  ✗ NO MATCH");
        }
    }
}
