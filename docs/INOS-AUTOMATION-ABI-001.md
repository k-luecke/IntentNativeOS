# INOS-AUTOMATION-ABI-001: Native Application Automation Contract

**Version:** 1.0.0  
**Status:** Approved Standard  
**Authors:** INOS Applications & Scripting Architecture Group  
**Scope:** Automation ABI between the INOS Script Runtime and Native Applications (Grid, Archive, Files, Scheduler)

---

## 1. Overview & Architectural Principles

`INOS-AUTOMATION-ABI-001` specifies the interface contract between the INOS Script runtime engine (`org.inos.script`), native application engines, and local planning agents (`APF-001`).

### Principles
1. **Local-First & Offline:** Pure local execution without external network, cloud, or telemetry dependencies.
2. **Explicit Capability Authorization:** No implicit file or kernel access; all script operations must match an approved capability grant.
3. **Inspectable Execution Plans:** Scripts and agents emit deterministic execution plans before modifying data.
4. **Deterministic Sandboxing & Quotas:** Bounded execution quotas for instructions, heap memory, call stack, and event cascades.

---

## 2. Capability Manifest & Security Schema

Every automation script defines or is associated with a `ScriptManifest`:

```rust
pub struct ScriptManifest {
    pub script_id: [u8; 32],
    pub script_id_len: usize,
    pub name: [u8; 32],
    pub name_len: usize,
    pub version: u32,
    pub allowed_paths: [[u8; 64]; 8],
    pub allowed_paths_len: [usize; 8],
    pub path_count: usize,
    pub permissions: u32, // Bitmask: READ=1, WRITE=2, CREATE=4, SCHEDULE=8
    pub quotas: ResourceQuotas,
}

pub struct ResourceQuotas {
    pub max_instructions: u64, // e.g. 250,000 instructions
    pub max_memory_bytes: u64, // e.g. 512 KiB heap limit
    pub max_call_depth: u32,   // e.g. 16 stack frames
    pub timeout_ms: u64,       // e.g. 5,000 ms wall clock
}
```

---

## 3. Automation APIs

### 3.1 `inos.grid` (Spreadsheet Automation)
```typescript
interface InosGrid {
    open(path: string): Workbook;
    create(path: string): Workbook;
}

interface Workbook {
    sheet(index: number): Sheet;
    save(path?: string): void;
    range(cellRange: string): Range;
}

interface Sheet {
    get(cell: string): number | string;
    set(cell: string, value: number | string): void;
    setFormula(cell: string, formula: string): void;
    evaluate(cell: string): number | string;
}

interface Range {
    sum(): number;
    avg(): number;
    min(): number;
    max(): number;
}
```

### 3.2 `inos.files` & `inos.archive` (Filesystem & Package Management)
```typescript
interface InosFiles {
    read(path: string): string;
    write(path: string, content: string): void;
    exists(path: string): boolean;
    list(dir: string): string[];
}

interface InosArchive {
    create(zipPath: string, entries: Array<{ name: string, content: string, deflate: boolean }>): boolean;
    extract(zipPath: string, destDir: string): boolean;
    list(zipPath: string): Array<{ name: string, size: number, compressedSize: number, crc32: number }>;
}
```

### 3.3 `inos.schedule` (Event Delivery & Job Registration)
```typescript
interface InosSchedule {
    at(timestamp: number, scriptPath: string): number;
    interval(seconds: number, scriptPath: string): number;
    onEvent(eventType: "WorkbookChanged" | "FileCreated", target: string, scriptPath: string): number;
    cancel(jobId: number): boolean;
}
```

---

## 4. Agent Compatibility & Inspectable Execution Plans

Agents interact with INOS Script using structured, inspectable proposals:

```json
{
  "contract": "INOS-AUTOMATION-ABI-001",
  "agent_id": "org.inos.agent.planner",
  "provenance": "LocalModel-APF001",
  "plan": {
    "target_files": ["sales.xlsx"],
    "operations": [
      { "op": "read", "path": "sales.xlsx", "range": "B2:B10" },
      { "op": "compute", "func": "SUM", "range": "B2:B10" },
      { "op": "write", "path": "report.xlsx", "cell": "A1", "value": 1500 }
    ],
    "requested_permissions": ["READ:sales.xlsx", "WRITE:report.xlsx", "CREATE:report.xlsx"],
    "required_quotas": {
      "instructions": 15000,
      "memory_bytes": 65536
    }
  }
}
```
