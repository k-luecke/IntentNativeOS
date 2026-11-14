# INOS Autonomous Product Foundry (APF-002) — Engineering Specification
## Milestone: Generalized Native Desktop Usability & Reliability

**Document Identifier:** `APF-002-SPECIFICATION`  
**Classification:** Engineering Architecture & Acceptance Contract  
**Starting Baseline:** Commit `9b221e8` (`foundry/apf-001`)  
**Target Milestone:** `APF-002`  
**Status:** Preregistered & Approved Architecture Blueprint  

---

## 1. Executive Charter & Purpose

Milestone `APF-001` established that the native microkernel, Compositor, Ring 3 applications (Grid, Write), persistent storage (INOS-FS), and cross-application automation (Flow Engine, `INOS-AUTOMATION-ABI-001`) can successfully execute a scripted end-to-end productivity workflow.

**Milestone `APF-002` transitions INOS from a verified demonstration into a hardened, dependable desktop platform foundation.** Rather than adding peripheral applications prematurely, APF-002 hardens interactive usability, format conformance, security isolation, failure recovery, and genuine local model autonomy under adversarial conditions.

---

## 2. Invariant Architectural Principle: Capability-Constrained Autonomy

The Autonomous Product Foundry and local intelligence agents operate under strict, fail-closed microkernel security:

> [!CRITICAL]
> **Autonomy Invariant:** Local models and autonomous agents must **never** possess unrestricted OS or hardware privileges. The model can propose, reason, and assemble plans; the operating system kernel remains the sole, immutable authority governing execution.

```
+-----------------------------------------------------------------------------+
|                          1. USER / HUMAN INTENT                             |
|         "Generate quarterly performance report from metrics.csv"            |
+-----------------------------------------------------------------------------+
                                      │
                                      ▼
+-----------------------------------------------------------------------------+
|               2. RING 3 LOCAL INTELLIGENCE (UNPRIVILEGED)                  |
|  - SmolLM2 / OCaml Engine: Parses intent into candidate Action Plan IR     |
|  - Synthesizes DAG formula edits, Word layouts, or IntScript flows          |
+-----------------------------------------------------------------------------+
                                      │
                                      ▼
+-----------------------------------------------------------------------------+
|           3. DETERMINISTIC POLICY & KERNEL CAPABILITY VERIFIER              |
|  - Kernel verifies caller PID (`SYS_GET_CALLER_IDENTITY`)                  |
|  - Validates requested target paths against scoped capability rights        |
|  - Enforces fail-closed rejection on unauthorized resources                 |
+-----------------------------------------------------------------------------+
                                      │
                                      ▼
+-----------------------------------------------------------------------------+
|             4. LAYERED CONFIRMATION MODAL (WHEN REQUIRED)                   |
|  - Two-choice confirmation modal rendered directly by Compositor            |
|  - Default hardware focus: [2] Cancel (Safe Fail-Closed Default)            |
|  - Human hardware keypress / physical confirmation required for high-risk   |
+-----------------------------------------------------------------------------+
                                      │
                                      ▼
+-----------------------------------------------------------------------------+
|            5. SCOPED EXECUTION, DURABLE JOURNAL & RECEIPT                   |
|  - Microkernel executes authorized Ring 3 worker processes                  |
|  - Every mutating action logged atomically in Reversible Action Ledger WAL  |
|  - Visual Transaction Receipt Card emitted to Habitable Desktop             |
+-----------------------------------------------------------------------------+
```

---

## 3. Seven Core Acceptance Criteria for APF-002

| # | Domain / Area | Acceptance Standard | Verification Mechanism |
| :- | :--- | :--- | :--- |
| **1** | **Unscripted Interactive Behavior** | Real-time keyboard navigation (arrow keys, Backspace, Delete), multi-cell selection, cursor positioning, and error recovery in Grid and Write under non-deterministic input sequences. | Interactive input fuzzer and manual QEMU session runner generating randomized keystroke/pointer sequences without crashes or hang states. |
| **2** | **Standards-Conforming Format Validation** | Generated `.xlsx`, `.docx`, and `.pdf` files pass strict structural, relational, and rendering validation by independent host parsers, verifying internal XML DOM relationships (`_rels`), namespaces, and binary PDF cross-reference tables (`xref`). | Freestanding host audit tool (`tools/validate_formats`) verifying ZIP entries, OPC relationships, Deflate streams, and PDF 1.7 xref offsets. |
| **3** | **Compositor & Clipboard Isolation** | Per-surface event queues (16 entries) implement non-blocking drop-tail overflow handling with logging. Clipboard access is capability-mediated (`rights::CLIPBOARD_READ / WRITE`). Focused surface ownership is enforced before delivering keystrokes. | Microkernel security suite verifying that background surfaces cannot snoop keystrokes or read clipboard content without explicit capability tokens. |
| **4** | **Resource Exhaustion & Containment** | Microkernel survives user-space memory exhaustion, infinite loops, and stack overflows. Faulting Ring 3 processes terminate cleanly with diagnostic exit codes (e.g. 139) while the kernel and sibling applications continue running. | Memory pressure stress tests and intentional user-mode fault injection (`fault_app`) under QEMU. |
| **5** | **Workflow Rollback & Replay Safety** | Injected failure at any intermediate pipeline step (e.g., corrupted input file, crash during DOCX generation) triggers atomic rollback via the Reversible Action Ledger without leaving orphan files or corrupted state. | Failure injection test harness (`test_workflow_rollback_01.sh`) exercising crash points across the multi-app pipeline. |
| **6** | **Cold Reboot & Repeated Execution** | INOS-FS survives arbitrary cold shutdowns, WAL crash replays, and repeated consecutive pipeline executions without performance degradation or data corruption. | Multi-cycle QEMU power-loss cycle test (`test_multi_reboot_01.sh`). |
| **7** | **Genuine End-to-End Local Inference** | At least one complete productivity task orchestrated via unconstrained local model inference over the full 49,152 vocabulary logits, with multi-token generation evaluated by an independent grader without pre-filtered distractor candidates. | End-to-end task runner connecting the Ring 3 OCaml/C SmolLM2 engine directly to the IntScript pipeline. |

---

## 4. Subsystem Architectures & Work Packages

### 4.1 Work Package 1: Compositor Security & Capability-Mediated Clipboard
- **Capability Tokens for Clipboard:**
  Define `rights::CLIPBOARD_READ (0x10)` and `rights::CLIPBOARD_WRITE (0x20)` in `kernel/src/capability/mod.rs`.
  Compositor operations `SetClipboard` and `GetClipboard` require valid capability tokens issued to the calling PID.
- **Surface Focus & Input Routing:**
  Keystrokes are delivered strictly to the currently focused surface (`active_surface_id`). Unfocused surfaces cannot receive keyboard input events.
- **Event Queue Overflow Hygiene:**
  When a per-surface event queue reaches 16 unconsumed events, incoming non-critical events (pointer moves) are dropped with an overflow warning flag, preventing queue starvation or memory leak.

### 4.2 Work Package 2: Comprehensive Format Interoperability Suite
- **Freestanding Host Format Validator (`tools/format_validator`):**
  A pure Rust CLI tool (zero Python) that examines raw disk images and extracted files:
  - **OpenXML XLSX:** Unpacks OPC container, parses `[Content_Types].xml`, `_rels/.rels`, `xl/workbook.xml`, and `xl/worksheets/sheet1.xml`. Verifies cell coordinate indices (`r="B5"`), formula syntax (`<f>SUM(B2:B4)</f>`), and calculated values (`<v>75</v>`).
  - **OpenXML DOCX:** Unpacks OPC container, validates `word/document.xml`, verifying table row/column structures, heading styles, and paragraph runs.
  - **PDF 1.7:** Parses binary header (`%PDF-1.7`), object streams, page tree `/Type /Pages`, page catalog `/Type /Catalog`, stream length indicators, xref table byte offsets, and trailer dictionary `/Size /Root %%EOF`.

### 4.3 Work Package 3: Unscripted Desktop Interaction & Fuzzing
- **Hardware Input Simulation Harness:**
  Extend test scripts to inject stochastic keystrokes, navigation commands, and edits:
  - Arrow navigation, cell editing, formula changes, invalid syntax entry (e.g. `=SUM(` without closing parenthesis), and verification that `#VALUE!` or `#SYNTAX!` error states render cleanly without application panics.
  - Multi-window focus switching via Tab/hardware hotkeys, verifying compositor z-order restacking and background surface clipping.

### 4.4 Work Package 4: Reversible Action Ledger & Injected Failure Rollback
- **Rollback Protocol (`INOS-AUTOMATION-ABI-001`):**
  Each mutating operation registers an inverse compensatory action:
  - `file.create(path)` $\rightarrow$ compensatory action: `file.delete(path)`.
  - `file.write(path, data)` $\rightarrow$ compensatory action: restore previous snapshot from WAL.
  - Injected crash before receipt generation triggers the Flow Engine rollback supervisor, which executes registered compensations in reverse chronological order and asserts a clean, uncorrupted storage state.

### 4.5 Work Package 5: Genuine Unforced Model Inference Autonomy
- **Full-Vocabulary Decoding Engine:**
  - Execute SmolLM2-135M forward pass without candidate distractor sub-selection.
  - Autoregressively generate variable-length tokens using greedy argmax or temperature-scaled sampling over the complete 49,152 vocabulary logits.
  - Integrate dynamic Structured Capsule Projection (SCP) as a grammatical constraint layer that prunes impossible syntax tokens based on active schema state machines.
  - Feed the generated plan directly into the Ring 3 IntScript interpreter, validating that an unprompted model-generated command can execute across native apps under capability containment.

---

## 5. Development Invariants & Strict Rules

1. **Zero Python:** Strictly maintained across all code, tools, packagers, and test harnesses.
2. **Ring 3 Capability Isolation:** All productivity apps and model inference engines run in user mode (CPL=3) with isolated page tables.
3. **Fail-Closed Security:** Unauthorized or unmapped syscalls, capability requests, or IPC messages are rejected fail-closed.
4. **Reproducible QEMU Verification:** All acceptance tests must execute cleanly under standard QEMU virtualized reference hardware with 16MB raw disk images.
5. **Durable Persistence:** Every committed milestone must survive a simulated cold power cycle.
