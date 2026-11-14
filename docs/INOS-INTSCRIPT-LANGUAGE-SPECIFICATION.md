# INOS IntScript Language & Automation Platform Specification

**Standard Identifier:** `INOS-INTSCRIPT-LANG-SPEC-v3.0`  
**Milestone:** `APP-SCRIPT-003`  
**Package Identity:** `org.inos.intscript`  
**Shared Wire Protocol:** `INOS-AUTOMATION-ABI-001`  
**Preferred Script Extension:** `.ints` (with full `.js` backward compatibility)  

---

## 1. Executive Summary & Product Architecture

**INOS IntScript** is the native, deterministic automation language and runtime embedded in the Intent-Native Operating System (INOS). Designed to deliver a local, private alternative to cloud scripting environments (such as Google Apps Script), IntScript operates completely offline in Ring 3 user-space (`x86_64-unknown-none`) with zero Python dependencies, zero C runtime dependencies (`libc`-free), and zero neural drift.

### 1.1 Architectural Hierarchy

| Component | Nomenclature | Responsibility |
| :--- | :--- | :--- |
| **Public Product** | `INOS IntScript` | Full offline automation ecosystem for INOS |
| **Desktop Application** | `IntScript Studio` | Multi-tab compositor GUI IDE (editor, debugger, scheduler, ledger) |
| **Execution Runtime** | `IntScript Engine` | Sandboxed ECMAScript interpreter with deterministic quotas |
| **Automation Daemon** | `IntScript Scheduler` | Background event/timer daemon managing `/system/jobs.db` |
| **Package Distribution** | `intscript.inapp` | Cryptographically signed Ring 3 application archive |

```
+------------------------------------------------------------------------+
|                          IntScript Studio                              |
|   +-------------------+  +--------------------+  +------------------+  |
|   |   Script Editor   |  |   Step Debugger    |  |  Perms Inspector |  |
|   +-------------------+  +--------------------+  +------------------+  |
|   +-----------------------------------------------------------------+  |
|   |         IntScript Engine (Sandboxed ECMAScript Evaluator)       |  |
|   +-----------------------------------------------------------------+  |
|   |    inos.grid    |    inos.files    |    inos.ipc    |  inos.audit   |  |
+---+--------+--------+---------+--------+--------+-------+-------+------+--+
             |                  |                 |               |
   +---------v------------------v-----------------v---------------v------+
   |          Kernel Capability Broker & Syscall Gate (Ring 0)           |
   |           SYS_SET_SCRIPT_CONTEXT / SYS_MIGRATE_PACKAGE_IDENTITY     |
   +-------------------------------------+-------------------------------+
                                         |
                       +-----------------v-----------------+
                       |        IntScript Scheduler        |
                       |         /system/jobs.db           |
                       |    /system/script_receipts.db     |
                       |    /system/script_history.log     |
                       +-----------------------------------+
```

---

## 2. IntScript Language Subset & Conformance

IntScript implements a strict, deterministic subset of the ECMAScript standard. Scripts written with `.ints` or `.js` file extensions are parsed and executed identically by the engine.

### 2.1 Grammar and Lexical Structure

- **Keywords:** `const`, `let`, `function`, `return`, `if`, `else`, `while`, `for`, `break`, `true`, `false`, `null`.
- **Identifiers:** Standard alphanumeric identifiers matching `[a-zA-Z_][a-zA-Z0-9_]*`.
- **Comments:** Single-line comments starting with `//`.
- **String Literals:** Double-quoted strings with escape sequences (`\"`, `\n`, `\t`, `\\`).

### 2.2 Data Types & Expressions

1. **Numbers:** 64-bit integer and IEEE 754 float representation. Supports standard precedence arithmetic (`+`, `-`, `*`, `/`, `%`).
2. **Strings:** Immutable UTF-8 string slices. Automatic string concatenation via `+` operator when either operand is a string.
3. **Booleans:** `true` and `false`. Logical operators (`&&`, `||`, `!`) and relational comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`).
4. **Arrays:** Ordered dynamic vectors (`[elem1, elem2, ...]`), indexed with `array[idx]`.
5. **Objects:** Dynamic key-value mappings supporting dot notation (`obj.prop`) and method calls (`obj.method(arg)`).

### 2.3 Functions & Variable Scope

- Block scoping for `let` and `const`.
- Functions are first-class declarations with lexical argument binding and explicit `return`.
- Recursion depth is strictly bounded by hardware call-depth quotas (anti-stack-overflow).

---

## 3. Standard Automation API (`inos.*`)

The `inos` root namespace provides deterministic, capability-guarded host interfaces.

### 3.1 Spreadsheet Automation (`inos.grid`)

| Method | Signature | Description |
| :--- | :--- | :--- |
| `open(path)` | `(path: string) -> Workbook` | Opens an existing XLSX workbook from INOS-FS. |
| `wb.getSheet(name)` | `(name: string) -> Sheet` | Selects a worksheet by name (e.g., `"Sheet1"`). |
| `sheet.set(cell, val)` | `(cell: string, val: num\|str) -> void` | Writes a numeric or text literal to a cell. |
| `sheet.setFormula(cell, f)`| `(cell: string, f: string) -> void` | Assigns an Excel-compatible formula (e.g. `"=B2-C2"`). |
| `sheet.evaluate(cell)` | `(cell: string) -> number` | Evaluates cell formulas and returns computed results. |
| `sheet.getRange(range).sum()`| `(range: string) -> number` | Computes aggregate sum over a range (e.g. `"D2:D10"`). |
| `wb.save(path)` | `(path: string) -> void` | Encodes and commits an OpenXML XLSX archive to disk. |

### 3.2 File System (`inos.files`)

- `inos.files.read(path: string) -> string`: Reads a scoped file authorized in the script manifest.
- `inos.files.write(path: string, content: string) -> void`: Atomically writes content to a scoped file.
- `inos.files.exists(path: string) -> boolean`: Checks if target file exists.

### 3.3 Inter-Process Communication (`inos.ipc`)

- `inos.ipc.discover(package_id: string) -> array<string>`: Queries exposed operations on target application.
- `inos.ipc.call(package_id: string, op: string, args...) -> any`: Dispatches typed `AutomationIpcWire` request across Ring 3 channels.

### 3.4 Audit & Telemetry (`inos.audit`)

- `inos.audit.log(msg: string) -> void`: Appends timestamped audit message to `/system/script_history.log`.

---

## 4. Capability Broker & Sandboxing Security

IntScript adheres to the **Principle of Least Privilege**. Authority is segregated across four distinct tiers:

```
+----------------------------------------------------------------------+
|                     Tier 1: Package Manager                          |
|  - Creates & installs .inapp archives (Privileged ring 3 authority)  |
+-----------------------------------+----------------------------------+
                                    |
+-----------------------------------v----------------------------------+
|                  Tier 2: IntScript Scheduler                         |
|  - Access to /system/jobs.db, /system/script_receipts.db             |
|  - Spawns background tasks; CANNOT install arbitrary .inapp packages |
+-----------------------------------+----------------------------------+
                                    |
+-----------------------------------v----------------------------------+
|                  Tier 3: IntScript Studio IDE                        |
|  - Interactive editor, runs under org.inos.intscript identity         |
|  - Reads/writes .ints, registers jobs, requests scoped capabilities  |
+-----------------------------------+----------------------------------+
                                    |
+-----------------------------------v----------------------------------+
|                  Tier 4: Sandboxed Scripts                           |
|  - STRICTLY isolated by ScriptIsolationContext                      |
|  - Zero inheritance from Studio; only explicitly declared paths      |
+----------------------------------------------------------------------+
```

### 4.1 Script Isolation Context (`SYS_SET_SCRIPT_CONTEXT`)

Every individual script executes inside a restricted kernel context:
```rust
#[repr(C)]
pub struct ScriptIsolationContext {
    pub script_id: [u8; 32],
    pub path_count: u32,
    pub permissions: u32,
    pub allowed_paths: [[u8; 64]; 8],
}
```
If a script attempts to read or write any path outside `allowed_paths`, the kernel immediately denies the operation fail-closed (`CapError::InsufficientRights`) and emits security denial audit markers.

### 4.2 Instruction Quotas & Anti-Recursion Loop Defense

- **Instruction Quota:** Halts execution if instruction count exceeds `ScriptQuotas::max_instructions` (default: 250,000 instructions).
- **Call Depth Limit:** Halts execution if call stack exceeds 64 nested activation frames.
- **Event Cycle Suppression:** Detects recursive cascading triggers (e.g. script writes to `report.xlsx`, triggering another job on `report.xlsx`), breaking cascading cycles at depth 1.

---

## 5. IntScript Scheduler & Durable Ledger

The IntScript Scheduler manages persistent background execution across cold reboots.

### 5.1 Jobs Database (`/system/jobs.db`)

Binary format guarded by magic header `INOS_JOB_DB_v1.0`:
- **Triggers:**
  - `TRIGGER_MANUAL` (0): Explicit on-demand script dispatch.
  - `TRIGGER_INTERVAL` (1): Periodic execution spaced by `interval_seconds`.
  - `TRIGGER_TIMESTAMP` (2): Scheduled single execution at designated epoch `schedule_time`.
  - `TRIGGER_FILE_CHANGED` (3): Monitored filesystem file change notification.
  - `TRIGGER_WORKBOOK_CHANGED` (4): Monitored spreadsheet / workbook modification notification.
- **Missed Run Policy:** `MISSED_POLICY_RUN_IMMEDIATE` (0) ensures that due or missed scheduled jobs run promptly upon cold reboot; `MISSED_POLICY_SKIP` (1) skips missed iterations.
- **Script Ownership:** Each record tracks the authorized package owner (`script_id`).

### 5.2 Durable Receipts Ledger & Idempotent At-Least-Once Delivery (`/system/script_receipts.db`)

Binary format guarded by magic header `INOS_RCPT_DB_v10`:
- Tracks `receipt_id`, `job_id`, `status` (`STATUS_SUCCESS = 2`, `STATUS_FAILED = 3`, etc.), `exit_code`, `timestamp`, `instructions_executed`, and output hash `output_hash`.
- **Deduplication vs. Exactly-Once Boundary:**
  - The IntScript scheduler implements **at-least-once job delivery with durable receipt deduplication**.
  - On cold reboot or process restart, the scheduler inspects `/system/script_receipts.db` before dispatching pending jobs. If a job has already committed a successful receipt, it is deduplicated and execution is skipped.
  - *Transactional Crash Coordination:* To prevent duplicate side effects if a crash occurs between external file mutation (e.g. workbook write) and receipt commit, application side effects, receipt recording, and job status commits are coordinated via atomic WAL synchronization (`sync_metadata`), ensuring crash-consistent recovery without state corruption.

---

## 6. Authenticated Identity Migration Protocol

To transition installations from the legacy `org.inos.script` identity to `org.inos.intscript`, INOS provides the `SYS_MIGRATE_PACKAGE_IDENTITY` syscall (Syscall #29).

### 6.1 Security Invariants & Capability Enforcement

1. **Kernel-Verified Caller Identity:** Migration authority is never inferred from PID alone (PID 1 confers zero migration privilege). Caller must be an authenticated process instance explicitly registered with `is_package_manager == true` and authenticated identity `org.inos.pkgmgr`.
2. **Dynamic Capability Token (`rights::MIGRATE_ADMIN`):** Callers must present a valid, cryptographically unpredictable capability token issued by the kernel specifically for target `"system:package_migration"` holding `rights::MIGRATE_ADMIN` (`1 << 8`).
3. **No Self-Issuance:** Ordinary applications (e.g. IntScript Studio, Notes, Calc) and unauthenticated processes cannot self-issue or manufacture migration capabilities. Knowledge of obsolete constants (such as deprecated `0x494E4F534D494752`) confers zero privilege and fails closed (`PermissionDenied`).
4. **Context Restriction:** Scripts running within a sandboxed `ScriptIsolationContext` are strictly forbidden from initiating migration.
5. **Privilege Non-Expansion:** Migrating identity cannot request rights beyond the existing package grant. Requests for package installer authority (`0x8000`) or root filesystem modification are rejected fail-closed.
6. **Single-Use & Replay Defense:** Capability tokens are bound to specific transaction descriptors and current process instances. Upon transaction commit, the capability token is revoked immediately, preventing replay attacks. Terminating the process revokes all tokens.
7. **Permanent Legacy Retirement:** Upon commit, legacy `org.inos.script` permissions are marked permanently retired (`LEGACY_SCRIPT_RETIRED`), preventing concurrent dual-execution loopholes.
8. **Transactional Rollback:** Interrupted migrations in Prepared state recover cleanly via rollback with zero data loss and zero duplicate job executions.

### 6.2 Package Integrity & Cryptographic Signing Architecture

- **Current Implementation (SHA-256 Digest Verification):**
  Application packages (`.inapp`) generated by `tools/inapp_packager` compute cryptographic SHA-256 digests over the package header and executable ELF code. The kernel package verifier (`process::package::verify_and_authenticate_package`) validates the code hash against registered installation manifests before permitting execution.
- **Forward Signature Roadmap (Asymmetric Publisher Keys):**
  Production releases will incorporate asymmetric public-key cryptography (Ed25519 / RSA-PSS) wherein packages carry detached or embedded publisher signatures verified against trusted root certificate authorities, providing origin authenticity and tamper-evidence independent of kernel installation registries.

---

## 7. Compliance Verification Matrix

| Acceptance Gate | Verification Criterion | Status |
| :--- | :--- | :--- |
| **Gate 1** | IntScript Studio launches under verified `org.inos.intscript` identity | Verified |
| **Gate 2** | `.ints` script executes authorized Grid workflow producing valid XLSX | Verified |
| **Gate 3** | Existing `.js` scripts execute with full language subset compatibility | Verified |
| **Gate 4** | Obsolete secret constants (`0x494E4F534D494752`) confer zero privilege and fail closed | Verified |
| **Gate 5** | Self-issuance defense prevents ordinary applications from manufacturing `MIGRATE_ADMIN` | Verified |
| **Gate 6** | Sandboxed script isolation and unauthorized callers rejected fail-closed | Verified |
| **Gate 7** | Attempts to expand permissions through migration are rejected fail-closed | Verified |
| **Gate 8** | Dynamic capability token verified; token replay rejected fail-closed | Verified |
| **Gate 9** | Legacy `org.inos.script` privileges permanently retired post-migration | Verified |
| **Gate 10**| Cold reboot deduplication via `/system/script_receipts.db` ensures at-least-once idempotency | Verified |
| **Gate 11**| Output workbook valid in external Info-ZIP and LibreOffice Calc | Verified |
| **Gate 12**| Zero regressions across APP-SCRIPT-001/002/003, Grid, Archive, and Capability suites | Verified |
