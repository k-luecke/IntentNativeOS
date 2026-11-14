# INOS Script: Language Compatibility Specification & Conformance Guide

**Specification:** INOS-SCRIPT-LANG-001  
**Program:** Native Automation Platform (APP-SCRIPT-002)  
**Standard:** JavaScript-Compatible Automation Subset  
**Runtime:** Freestanding Ring 3 `#![no_std]` + `alloc` Micro-Engine  

---

## 1. Scope & Design Philosophy

INOS Script implements a deterministic, sandboxed, JavaScript-compatible automation subset designed for high-integrity, offline-first application automation in Ring 3 userspace. 

It is intentionally designed as an automation language subset, **not** a full ECMAScript 2026 engine. It guarantees:
1. **Deterministic Execution:** No nondeterministic async event loops, microtask re-entrancy, or unmetered background threads.
2. **Strict Resource Bounding:** Hard instruction quotas, heap memory quotas, and recursion call stack depth ceilings.
3. **Capability Sandboxing:** Scripts execute under an unforgeable `ScriptManifest` grant. Permissions attach to script identities rather than process IDs.
4. **Zero-Overhead Host Interop:** Direct binding to native applications (Grid, Files, Archive, Scheduler) through versioned IPC conforming to `INOS-AUTOMATION-ABI-001`.

---

## 2. Supported Language Subset

### 2.1 Lexical Elements
- **Whitespace & Comments:** Single-line comments (`// ...`) and multi-line comments (`/* ... */`).
- **Identifiers:** Alphanumeric starting with `[a-zA-Z_]`.
- **Keywords:** `const`, `let`, `var`, `if`, `else`, `while`, `for`, `function`, `return`, `break`, `continue`, `true`, `false`, `null`.
- **Literals:**
  - Integer literals: `0`, `42`, `-15000`
  - String literals: Single-quoted (`'hello'`) and double-quoted (`"world"`) with escape sequences (`\n`, `\t`, `\"`, `\'`, `\\`)
  - Boolean literals: `true`, `false`
  - Null literal: `null`
  - Array literals: `[1, 2, "three", true]`

### 2.2 Operators & Expressions
- **Arithmetic:** `+` (addition & string concatenation), `-` (subtraction & unary negation), `*` (multiplication), `/` (integer division), `%` (modulo).
- **Comparison:** `==` (equality with type coercion), `!=` (inequality), `<`, `<=`, `>`, `>=`.
- **Logical:** `&&` (logical AND), `||` (logical OR), `!` (logical NOT).
- **Assignment:** `=` (variable and property assignment), `+=`, `-=`.
- **Member Access:** Dot notation (`obj.prop`, `obj.method()`) and bracket notation for arrays (`arr[0]`).

### 2.3 Statements & Control Flow
- **Variable Declarations:** `const x = 10;`, `let y = "test";`.
- **Conditional Branching:** `if (cond) { ... } else if (cond2) { ... } else { ... }`.
- **Iteration:**
  - `while (cond) { ... }`
  - `for (let i = 0; i < n; i = i + 1) { ... }`
- **Functions:** Named functions with parameters and return values:
  ```javascript
  function calculateMargin(revenue, cost) {
      return revenue - cost;
  }
  ```

---

## 3. Automation Builtin Objects (`inos.*`)

### 3.1 `inos.grid`
- `inos.grid.open(path)`: Opens an existing workbook from INOS-FS.
- `inos.grid.create(path)`: Creates a new workbook.
- `inos.grid.save(path)`: Commits workbook to disk via atomic WAL.
- `sheet.get(cell)`: Reads cell numeric value or string.
- `sheet.set(cell, value)`: Sets cell numeric value or string.
- `sheet.setFormula(cell, formula)`: Sets formula (e.g. `"=B2-C2"`, `"=SUM(A1:A3)"`).
- `sheet.evaluate(cell)`: Evaluates formula dependency graph and returns result.
- `sheet.getRange(range).sum()` / `.avg()` / `.min()` / `.max()`: Range aggregation.

### 3.2 `inos.files`
- `inos.files.read(path)`: Reads text file content within authorized path scope.
- `inos.files.write(path, data)`: Writes text content within authorized path scope.
- `inos.files.exists(path)`: Checks if file exists.

### 3.3 `inos.schedule`
- `inos.schedule.interval(seconds, scriptPath)`: Registers recurring interval job.
- `inos.schedule.at(timestamp, scriptPath)`: Registers timestamp trigger.
- `inos.schedule.onEvent(eventType, targetPath, scriptPath)`: Registers event trigger (e.g. `WorkbookChanged`, `FileChanged`).
- `inos.schedule.cancel(jobId)`: Cancels pending job.

### 3.4 `inos.audit`
- `inos.audit.log(message)`: Emits tamper-evident log to execution audit ledger.

### 3.5 `inos.ipc` (Cross-Process Application Integration)
- `inos.ipc.discover(appId)`: Queries target application for supported operations.
- `inos.ipc.call(appId, operation, args)`: Dispatches typed IPC request conforming to `INOS-AUTOMATION-ABI-001`.

---

## 4. Sandboxing, Quotas & Safety Guarantees

| Safety Boundary | Ceiling / Default | Enforcement Mechanism |
| :--- | :--- | :--- |
| **Instruction Quota** | 250,000 ops (customizable) | Instruction counter decremented per AST node evaluation; triggers `EvalError::InstructionQuotaExceeded` |
| **Call Stack Depth** | 16 frames | Evaluator recursion depth counter; triggers `EvalError::CallDepthExceeded` |
| **Heap Memory** | 512 KiB | Enforced by `BoundedHeapAllocator` |
| **Capability Scope** | Declared in `ScriptManifest` | Path and rights checked fail-closed before any I/O or IPC operation |
| **Event Recursion** | Depth = 1 ceiling | Cycles suppressed with `[CYCLE_SUPPRESSED]` audit marker |
| **Process Identity** | Dynamic registration | Decoupled from hardcoded PIDs; verified against kernel registry |
