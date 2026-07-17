#[cfg(test)]
mod regex_tests {
    use regex::Regex;

    #[test]
    fn test_multiline_annotation() {
        let re_mapping = Regex::new(
            r#"@(Get|Post|Put|Delete|Patch|Request)Mapping\s*(?:\([^)]*?"([^"]*)")?"#,
        ).unwrap();

        let test_cases = vec![
            (r#"@PostMapping(value = "TicketCreationXML", consumes = { MediaType.APPLICATION_XML_VALUE,"#, Some("TicketCreationXML")),
            (r#"@PostMapping("TicketCreationXML")"#, Some("TicketCreationXML")),
            (r#"@GetMapping("/api/users")"#, Some("/api/users")),
            (r#"@RequestMapping(value = "/items/{id}")"#, Some("/items/{id}")),
            (r#"    @PostMapping(value = "TicketCreationXML", consumes = { MediaType.APPLICATION_XML_VALUE,"#, Some("TicketCreationXML")),
        ];

        for (line, expected) in test_cases {
            println!("Testing: {}", line);
            if let Some(cap) = re_mapping.captures(line) {
                let endpoint = cap.get(2).map(|m| m.as_str());
                println!("  Found: {:?}, Expected: {:?}", endpoint, expected);
                assert_eq!(endpoint, expected);
            } else {
                panic!("No match for: {}", line);
            }
        }
    }
}
