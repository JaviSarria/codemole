use regex::Regex;

fn main() {
    let line = "\tprivate Map<String, Object> getMapTicketToXML(TicketOPReq.PAYLOAD payload) {";
    let re = Regex::new(
        r"(?:public|protected|private)\s+(?:static\s+)?(?:final\s+)?(?:synchronized\s+)?(?:abstract\s+)?(?:native\s+)?[\w<>\[\]]+\s+(\w+)\s*\(",
    ).unwrap();
    
    println!("Line: {:?}", line);
    println!("Testing full regex:");
    
    if let Some(cap) = re.captures(line) {
        println!("✓ MATCH found!");
        println!("  Method name: {}", &cap[1]);
    } else {
        println!("✗ NO MATCH");
        println!("\nTrying simpler regex without modifiers:");
        let re2 = Regex::new(r"(?:public|protected|private)\s+[\w<>\[\]]+\s+(\w+)\s*\(").unwrap();
        if let Some(cap) = re2.captures(line) {
            println!("✓ Simple regex MATCH!");
            println!("  Method name: {}", &cap[1]);
        } else {
            println!("✗ Simple regex NO MATCH either");
        }
    }
}
