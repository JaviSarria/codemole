# Debugging Missing Calls in Sequence Diagrams

## Problem
Your sequence diagram shows `crudTicket()` but not the internal method calls like `getMapTicketToXML()`, `insertTicket()`, or `modificarTicket()`.

## Solution: Use the Debug Tool

I've added a debug tool to help diagnose what's happening. Here's how to use it:

### Step 1: Build the debug tool
```powershell
cd c:\Users\jsarria\workspace\rust\codemole
cargo build --example debug_calls --release
```

### Step 2: Run it on your service file

Find your `TicketCreationService.java` file and run:

```powershell
.\target\release\examples\debug_calls.exe "C:\path\to\TicketCreationService.java" crudTicket
```

Or use the helper script:

```powershell
.\debug.ps1 -JavaFile "C:\path\to\TicketCreationService.java" -MethodName crudTicket
```

### Step 3: Analyze the output

The tool will show you three sections:

#### Section 1: Method Source
Shows the actual method code being analyzed - verify it's correct.

#### Section 2: Detected Calls
Shows all method calls found, with status:
- `[KEPT]` - Will be followed in the diagram ✓
- `[SKIPPED]` - Filtered out (stdlib, framework, etc.) ✗

#### Section 3: Final Calls
Shows the methods that SHOULD appear in your diagram.

### Interpretation Examples

**Good output (methods should appear in diagram):**
```
--- Final Calls (AFTER filtering) ---
Kept for traversal:
  ✓ getMapTicketToXML
  ✓ insertTicket
  ✓ modificarTicket
  ✓ logError
```

**Problem: Methods filtered by skip-symbols**
```
Filtered out (in skip-symbols):
  ✗ stream      ← Java stream API
  ✗ collect     ← Java stream API
  ✗ filter      ← Java stream API
```
This is normal and OK - these are stdlib calls.

**Problem: Methods not detected at all**
```
--- Final Calls (AFTER filtering) ---
Kept for traversal:
  (no calls - all were filtered or skipped!)
```
This means the calls aren't being found. Possible reasons:
- They're in strings or comments
- They use unusual syntax patterns
- They're in a method you haven't scanned

## Common Issues and Solutions

### Issue: Methods not in "Kept" section

**Cause 1: Method calls are filtered by skip-symbols**
Solution: Edit `symbols.db` to add/remove symbols (see [Database Management](#database-management))

**Cause 2: Methods don't exist in any scanned file**
Solution: Ensure all Java files in your project are included in the path

**Cause 3: Method names are in the keyword list**
Solution: Check if your method name conflicts with Java keywords

### Issue: Correct calls in "Kept" but still not in diagram

This means calls are detected but not being resolved.

Possible causes:
1. **Methods in different package**: The methods might be in a different package that isn't in the call definition index
2. **Method visibility**: Check that methods are `public` or at least accessible
3. **Class hierarchy**: Might be in a parent class or interface

To diagnose:
1. Check if `crudTicket()` is from an interface vs implementation
2. Look at the full call graph size: `codemole ... --endpoint ...` shows "Call graph: X nodes, Y edges"
3. If Y=0, no calls are being followed at all - something is wrong

## Database Management

You can customize which symbols are skipped by editing the SQLite database:

```powershell
# Open the database with any SQLite client
sqlite3 "C:\Users\jsarria\workspace\rust\codemole\target\release\symbols.db"

# View all Java skip-symbols
SELECT symbol FROM skip_symbols 
WHERE language_id = (SELECT id FROM languages WHERE name = 'java')
ORDER BY symbol;

# Remove a symbol to allow it in diagrams
DELETE FROM skip_symbols 
WHERE language_id = (SELECT id FROM languages WHERE name = 'java')
AND symbol = 'collect';
```

## Still Having Issues?

Please share the output of:

```powershell
# 1. Debug tool output
.\target\release\examples\debug_calls.exe "path/to/TicketCreationService.java" crudTicket

# 2. The Call graph size
.\target\release\codemole.exe --lang java --endpoint TicketCreationXML --path "C:\path\to\project"
# Look for: "Call graph: X nodes, Y edges"

# 3. The sequence.md file that was generated
Get-Content "output\TicketCreationXML\diagrams\sequence.md"
```

Then I can help narrow down the exact issue.
