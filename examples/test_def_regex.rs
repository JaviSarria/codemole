use regex::Regex;

fn main() {
    let line = "\tprivate Map<String, Object> getMapTicketToXML(TicketOPReq.PAYLOAD payload) {";
    
    println!("Line: {:?}\n", line);
    
    // Test 1: Current strict regex
    let re1 = Regex::new(
        r"(?:public|protected|private)\s+(?:static\s+)?(?:final\s+)?(?:synchronized\s+)?(?:abstract\s+)?(?:native\s+)?[\w<>\[\]]+\s+(\w+)\s*\(",
    ).unwrap();
    
    println!("Test 1 - Full strict regex:");
    if let Some(cap) = re1.captures(line) {
        println!("✓ MATCH: {}\n", &cap[1]);
    } else {
        println!("✗ NO MATCH\n");
    }
    
    // Test 2: Without optional modifiers
    let re2 = Regex::new(r"(?:public|protected|private)\s+[\w<>\[\]]+\s+(\w+)\s*\(").unwrap();
    println!("Test 2 - Without optional modifiers:");
    if let Some(cap) = re2.captures(line) {
        println!("✓ MATCH: {}\n", &cap[1]);
    } else {
        println!("✗ NO MATCH\n");
    }
    
    // Test 3: With . in the return type regex
    let re3 = Regex::new(r"(?:public|protected|private)\s+(?:[\w<>\[\]\.]+\s+)+(\w+)\s*\(").unwrap();
    println!("Test 3 - With . for qualified types:");
    if let Some(cap) = re3.captures(line) {
        println!("✓ MATCH: {}\n", &cap[1]);
    } else {
        println!("✗ NO MATCH\n");
    }
    
    // Test 4: More permissive - match anything until method name
    let re4 = Regex::new(r"(?:public|protected|private)\s+.*?(\w+)\s*\(").unwrap();
    println!("Test 4 - Permissive (.*?):");
    if let Some(cap) = re4.captures(line) {
        println!("✓ MATCH: {}\n", &cap[1]);
    } else {
        println!("✗ NO MATCH\n");
    }
}
