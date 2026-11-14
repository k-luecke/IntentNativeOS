# INOS Application Ecosystem: Target Matrix & Functional Contracts (APF-001)

**Document Identifier:** `APF-001-APPLICATION-MATRIX`  
**Baseline Revision:** Commit `2afe360` (`app-cap-04`)  
**Standard of Parity:** Workflows are measured against concrete reference implementations using real document formats, genuine interactive controls, reliable persistence across cold reboot, and interoperability tests.

---

## 1. Native Application Parity Matrix

| Native Application | Functional Reference | Current Baseline | Target Milestone Scope | Status |
| :--- | :--- | :--- | :--- | :---: |
| **INOS Files + Archive** | Windows Explorer, 7-Zip | `archive_app` baseline: RFC 1951 Deflate, ZIP central directory, single/multi-file pack/unpack, CRC32, cold-reboot persistence. | Directory tree navigation, live search, preview panes, metadata inspection, batch copy/move, scoped permission elevation, ZIP creation/extraction, Zip-Slip rejection, bomb defense. | In Progress (Engines built; shell integration pending) |
| **INOS Grid** | Microsoft Excel, LibreOffice Calc | `grid_app` baseline: 2D sparse matrix, arithmetic evaluation, DAG dependency recalculation, `#CIRC!` cycle prevention, RFC 4180 CSV, OpenXML `.xlsx` generation. | Multi-sheet support, range formatting (numeric, currency, percent, date), formula library (SUM, AVERAGE, MIN, MAX, IF, VLOOKUP/XLOOKUP), cell styles, sorting, filtering, external XLSX import/export round-trip. | In Progress (Core DAG & XLSX exporter complete) |
| **INOS Write** | Microsoft Word, LibreOffice Writer | `notes_app` baseline: raw multiline text buffer, file load/save, cursor positioning, cold reboot persistence. | Full rich-text document engine, paragraph & character styling (bold, italic, underline, fonts, sizes, colors), document outline, tables, inline images, page layout & margins, pagination, headers/footers, undo/redo tree, DOCX import/export. | Planned (Phase 2B) |
| **INOS PDF** | Adobe Acrobat Core Workflows | None (Freestanding parser stub). | Compliant PDF 1.7 / 2.0 object parser, font decoding (Type 1, TrueType/OpenType), vector rendering pipeline, text extraction & search, page navigation, pan/zoom, form field interaction, annotations (highlight, notes), split/merge, linearized export. | Planned (Phase 2C) |
| **INOS Canvas** | Affinity Photo, Photoshop Core | None (Compositor 2D blit primitives only). | Layer-based raster/vector editing engine, layer blending modes, raster masks, selection tools (rect, lasso, magic wand), procedural brushes, vector shapes & bezier paths, affine transformations (scale, rotate, shear), typography engine, PSD / PNG / SVG import/export. | Planned (Phase 2D) |
| **INOS Mail** | Microsoft Outlook, Mozilla Thunderbird | None (`net_app` raw TCP socket baseline). | Multi-account management, local RFC 5322 MIME message parser/generator, local message index & full-text search, conversation threading, attachment handling with scoped sandbox extraction, drafts engine, background IMAP / SMTP synchronizer. | Planned (Phase 2E) |
| **INOS Plan** | Microsoft Outlook Calendar, Todoist | None (Raw RTC driver baseline). | Interactive calendar views (day, week, month, multi-month), task lists with hierarchical subtasks and priorities, due dates & local alarm notifications, recurring event recurrence rules (RFC 5545 iCalendar `RRULE`), timezone conversions, event conflict detection. | Planned (Phase 2F) |
| **INOS Intent** | Windows Copilot, Raycast, macOS Spotlight | OCaml Intent Runtime v0.2, typed IR, clarification gate, deterministic registry, model runner IPC. | System-wide intent bar, multi-app workflow composition, deterministic tool registry, parameter extraction, natural language task synthesis, reversible execution preview, rollback receipt generation. Fully operational without LLM. | In Progress (OCaml runtime & deterministic engine online) |
| **System Utilities** | Task Manager, Settings, Console | `spin_app`, `calc_app`, basic console. | System Monitor (real-time CPU, RAM, disk, IPC telemetry), App Manager (cryptographic package verification, install, isolate, revoke), Settings (display, input, sound, storage), Terminal (native Ring 3 shell, piping, redirection). | In Progress (Package manager & monitor primitives online) |

---

## 2. Functional Feature Contracts

### 2.1 INOS Files + Archive
- **Contract F-01 (Filesystem Navigation):** Deterministic path resolution, breadcrumb navigation, symlink loop detection, root jail containment.
- **Contract F-02 (Archive Interchange):** Create standard `.zip` archives unpackable by standard unzip, 7-Zip, and Windows Explorer; extract archives created by third-party tools.
- **Contract F-03 (Security Hardening):** Strict prevention of directory traversal attacks (`../`, absolute paths in headers), zip bomb decompression quotas (uncompressed byte limits, compression ratio bounds <= 100:1).
- **Contract F-04 (Atomic File Mutation):** File rename, copy, move operations execute with WAL-backed atomicity in INOS-FS.

### 2.2 INOS Grid
- **Contract G-01 (Calculation Engine):** Topological sort on cell dependency DAG. Immediate cascading recalculation. Detection of self-referential or cyclic references producing `#CIRC!` error without infinite loops.
- **Contract G-02 (Data Interchange):** RFC 4180 CSV import and export with quote escaping, multi-line cell text, comma/semicolon delimiter detection.
- **Contract G-03 (OpenXML Conformance):** Produce valid `.xlsx` packages containing `[Content_Types].xml`, `_rels/.rels`, `xl/workbook.xml`, `xl/worksheets/sheet1.xml`, and `xl/styles.xml` that open in Excel and LibreOffice without repair warnings.
- **Contract G-04 (Grid Interaction):** Keyboard arrow navigation, range selection (`Shift+Arrows`), formula bar editing, row/column resizing, dirty cell rendering optimization.

### 2.3 INOS Write
- **Contract W-01 (Text Representation):** Piece table or rope data structure capable of handling large documents (>100,000 characters) with \(O(\log N)\) insert/delete performance.
- **Contract W-02 (Formatting Model):** Span-based style runs for character formats (bold, italic, underline, strike, font family, font size, foreground/background color); block attributes for paragraphs (alignment, line spacing, margins, lists).
- **Contract W-03 (Layout & Pagination):** Real-time text measurement and line breaking; paginated view with configurable paper sizes (A4, Letter) and printable margins; header and footer regions.
- **Contract W-04 (Interoperability):** Lossless plain text and Markdown import/export; OpenXML DOCX round-trip parser and generator for core document styles, tables, and runs.

### 2.4 INOS PDF
- **Contract P-01 (Parser Conformance):** Lexing and parsing of PDF cross-reference tables (both classical xref and cross-reference streams), indirect object trees, dictionary/stream structures.
- **Contract P-02 (Stream Decoders):** FlateDecode (using freestanding RFC 1951 Deflate engine), ASCIIHexDecode, ASCII85Decode, RunLengthDecode.
- **Contract P-03 (Rendering Pipeline):** Vector graphics operator interpreter (`m`, `l`, `c`, `re`, `W*`, `f`, `S`), coordinate system transformations, font glyph rendering.
- **Contract P-04 (Document Operations):** Page extraction, document concatenation/merge, page rotation, text layer search with coordinate bounding boxes.

### 2.5 INOS Canvas
- **Contract C-01 (Layer Engine):** Multilayer compositing engine with opacity, visibility toggling, layer ordering, and standard blend modes (Normal, Multiply, Screen, Overlay, Darken, Lighten).
- **Contract C-02 (Raster Tools):** Memory-efficient tiled raster canvas, Bresenham line and anti-aliased circular brush stampers, flood fill with thresholding.
- **Contract C-03 (Vector Tools):** Cubic bezier curves, path construction, stroked outlines, fill operations, path boolean operations (union, difference).
- **Contract C-04 (Export & Storage):** Fast lossless raw project format with incremental tile saving; standard PNG and SVG exporter.

### 2.6 INOS Mail
- **Contract M-01 (Message Storage):** Maildir or compact append-only SQLite/WAL-style message store indexed by Message-ID, thread ID, sender, recipient, timestamp, and read status.
- **Contract M-02 (MIME Engine):** Complete multipart/mixed, multipart/alternative, multipart/related parser; base64 and quoted-printable decoders; UTF-8, ISO-8859-1 charset transcoding.
- **Contract M-03 (Security Isolation):** HTML email rendering strictly sandboxed without remote asset auto-fetching (preventing tracking pixels and CSRF); attachments isolated behind capability checks.
- **Contract M-04 (Network Transport):** Native TLS-secured IMAP (fetch, sync flags) and SMTP (send, auth) clients operating through scoped socket capabilities.

### 2.7 INOS Plan
- **Contract L-01 (Temporal Engine):** Gregorian calendar math, leap years, day-of-week calculation, timezone offset adjustments, UTC normalization.
- **Contract L-02 (Recurrence Evaluator):** RFC 5545 RRULE expansion engine supporting daily, weekly (with day masks), monthly (by day of month or by weekday occurrence), yearly recurrences, exception dates (EXDATE).
- **Contract L-03 (Conflict Detection):** Interval tree overlap detector identifying overlapping events and proposed alternative open slots.
- **Contract L-04 (Alarms & Reminders):** Persistent event queue registered with the system alarm broker to trigger UI notifications at designated pre-event offsets.

---

## 3. Explicit List of Non-Goals & Unsupported Complexities

To avoid decorative scope creep while delivering rigorous depth, the following are designated out-of-scope for APF-001:
- Real-time multi-user Google Docs-style collaborative editing over WebSockets.
- Cloud document synchronization or vendor lock-in storage providers (OneDrive, iCloud, Google Drive).
- 3D rendering pipelines, ray-tracing, or complex GPU mesh shaders in Canvas.
- Complex Exchange / ActiveDirectory proprietary RPC protocol support (standard IMAP/SMTP/iCal is the standard).
- Visual Basic for Applications (VBA) macro runtime or Excel COM automation.
