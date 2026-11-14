# UI-01 — Habitable Computing Design System Specification
**Intent-Native Operating System (INOS)**  
*Document Version: 1.0.0 — Gate UI-01 Foundation*  
*Status: Approved Baseline Specification*

---

## 1. Executive Summary & Design Philosophy

The purpose of the **Habitable Computing Foundation** is to establish an operating environment that is human-centered, serene, and grounded in architectural space. 

Past iterations of the INOS interface relied on high-contrast, dark slate-blue terminal styling (`#0F172A`, `#1E293B`, neon cyan borders) cluttered with low-level diagnostics, hardware ports, and raw registers. While effective for initial driver verification, that environment communicated *machine-centric complexity* rather than *human utility*.

### The Core Design Principle
> **The interface should never require users to understand how intelligent the computer is. It should help them understand what they can do, what will happen, and what has happened.**

This principle delineates the difference between an "AI-themed desktop" (characterized by conspicuous badges, futuristic HUDs, chat bubbles, and streaming tokens) and a **genuinely natural operating system** (where documents and human actions are primary, and intelligence acts as an unobtrusive, polite, and verified advisor).

---

## 2. Spatial Layout & Architectural Principles

Rather than packing widgets into an information-dense dashboard, the Habitable Computing environment treats the screen as an **architectural space** with generous breathing room, balanced proportions, and clear visual hierarchy.

```
+-------------------------------------------------------------------------------+ (1280x800 Framebuffer)
|  Top Surface: Breadcrumb, Document Identity, Serene Status, [Tab: Details]   | (Height: 44px)
+-------------------------------------------------------------------------------+
|                                                                               |
|       +---------------------------------------------------------------+       |
|       |                                                               |       |
|       |                   Centered Document Canvas                    |       |
|       |                  ("Paper on a wooden desk")                   |       |
|       |                                                               |       |
|       |   Document Title (Editorial Hierarchy, Scale 2)              |       |
|       |   Subtle dividing hairline                                    |       |
|       |                                                               |       |
|       |   Document Body Text (Proportional spacing, high contrast)    |       |
|       |   Lorem ipsum dolor sit amet...                               |       |
|       |                                                               |       |
|       |   [Intent Bar: "What would you like to do with this?"]        |       |
|       |                                                               |       |
|       +---------------------------------------------------------------+       |
|                                                                               |
+-------------------------------------------------------------------------------+
|  Bottom Shelf: Local Actions (F1 New | F2 Save | F3 Undo | F10 Power)         | (Height: 36px)
+-------------------------------------------------------------------------------+
```

### Architectural Principles:
1. **The Canvas as Primary Subject:** The document or working artifact occupies the visual center of mass. It floats like a crisp sheet of paper on a warm, serene desk.
2. **Generous Margins:** Side margins of 160–240px and vertical margins of 32–48px prevent the eye from feeling crowded. Content breathes naturally.
3. **Subtle Elevation & Inset Borders:** Soft 1px borders with delicate drop-shadow bevels create depth without relying on heavy skeuomorphism or harsh neon outlines.
4. **Dock and Shelves:** Navigation and system controls are anchored to the perimeter in quiet, understated shelves, leaving the central canvas undisturbed.

---

## 3. Typography Subsystem Architecture

Typography in UI-01 is elevated from a crude monospaced grid to a structured typographic subsystem supporting proportional layout, baseline alignment, and hierarchical roles.

### 3.1 Typographic Metrics & Font Structure
A first-class typeface representation requires:
- **Advance Width ($w_{adv}$):** Variable per-character width to eliminate awkward spacing in characters like `'i'`, `'l'`, `'w'`, and `'m'`.
- **Bounding Box ($w, h$):** Tight glyph bounds.
- **Baseline Alignment:** A shared baseline (8px or 16px) upon which all glyphs sit.
- **Line Height:** Proportional leading ($1.4\times$ to $1.6\times$ character height) ensuring comfortable reading cadence.

```rust
pub struct GlyphMetric {
    pub ascii: u8,
    pub width: u8,
    pub height: u8,
    pub advance: u8,
    pub x_offset: i8,
    pub y_offset: i8,
}

pub struct Typeface {
    pub name: &'static str,
    pub line_height: u8,
    pub baseline: u8,
    pub metrics: [GlyphMetric; 95], // ASCII 32..126
}
```

### 3.2 Typographic Hierarchy
| Role | Typeface Style | Scale | Line Height | Usage |
| :--- | :--- | :--- | :--- | :--- |
| **Display / Title** | Editorial Serif / Bold Sans | 16px (Scale 2) | 26px | Document title, modal headers, confirmation verbs |
| **Body Reading** | Interface Sans | 8–10px (Scale 1) | 16px | Document content, explanations, modal prose |
| **Affordances / Buttons**| Medium Interface Sans | 8px (Scale 1) | 14px | Keycap badges (`[1]`, `[F2]`), action labels |
| **Secondary Metadata** | Quiet Monospace / Micro | 8px (Scale 1) | 12px | Timestamps, file sizes, receipt IDs, WAL status |

---

## 4. Color Palette & Accessibility Contrast

Warmth in Habitable Computing is not achieved merely by turning the screen beige; it is rooted in natural contrast, harmonious organic hues, and strict readability compliance (WCAG AA standard: minimum 4.5:1 contrast for body text, 3:1 for large display titles).

### 4.1 Habitable Palette Definition (32-bit ARGB)
```rust
// Primary Canvas & Atmosphere
pub const COLOR_PAPER_BG: Color        = Color { r: 245, g: 242, b: 235, a: 255 }; // #F5F2EB - Warm Linen Desk
pub const COLOR_CANVAS_SHEET: Color    = Color { r: 252, g: 251, b: 249, a: 255 }; // #FCFBF9 - Crisp Ivory Paper
pub const COLOR_CARD_BG: Color         = Color { r: 255, g: 255, b: 255, a: 255 }; // #FFFFFF - Clean Card Surface
pub const COLOR_SHELF_BG: Color        = Color { r: 238, g: 234, b: 226, a: 255 }; // #EEEAE2 - Soft Stone Shelf

// Typography & Ink (High Contrast)
pub const COLOR_INK_PRIMARY: Color     = Color { r: 36,  g: 34,  b: 32,  a: 255 }; // #242220 - Deep Charcoal Ink (14.2:1 contrast on Sheet)
pub const COLOR_INK_MUTED: Color       = Color { r: 104, g: 100, b: 94,  a: 255 }; // #68645E - Warm Muted Stone (4.8:1 contrast on Sheet)
pub const COLOR_BORDER_HAIRLINE: Color = Color { r: 221, g: 216, b: 206, a: 255 }; // #DDD8CE - Soft Linen Border
pub const COLOR_BORDER_SHADOW: Color   = Color { r: 200, g: 194, b: 184, a: 255 }; // #C8C2B8 - Drop-shadow edge

// Intent, Affirmation & Caution
pub const COLOR_TERRACOTTA: Color      = Color { r: 196, g: 90,  b: 60,  a: 255 }; // #C45A3C - Warm Clay / Intent Focus
pub const COLOR_SAGE_GREEN: Color      = Color { r: 54,  g: 114, b: 78,  a: 255 }; // #36724E - Forest Sage / Non-destructive Safe
pub const COLOR_WARM_AMBER: Color      = Color { r: 212, g: 130, b: 46,  a: 255 }; // #D4822E - Amber / Reversible Notice
pub const COLOR_BRICK_RED: Color       = Color { r: 184, g: 62,  b: 50,  a: 255 }; // #B83E32 - Brick / Destructive Warning
pub const COLOR_DEEP_NAVY: Color       = Color { r: 42,  g: 58,  b: 76,  a: 255 }; // #2A3A4C - Slate Navy / Details Badge
```

---

## 5. Interaction State Machine & Model-Free Baseline

The operating system interface must be **genuinely interactive and state-driven**, not a collection of static presentation screens. Most critically, **everyday desktop operations require zero model initialization.**

```
                           +----------------------+
                           |                      |
                           |  [MODEL-FREE BOOT]   |
                           |                      |
                           +----------+-----------+
                                      |
                                      v
                             +------------------+
                             |                  |<-----------------------+
                +----------->| EDITING_DOCUMENT |                        |
                |            |  (Active Canvas) |                        |
                |            +--------+---------+                        |
                |                     |                                  |
                |                     | [Enter] / Intent Key             |
                |                     v                                  |
                |            +------------------+                        |
                |            | AWAITING_INTENT  |                        |
                |            |  (Intent Capsule)|                        |
                |            +--------+---------+                        |
                |                     |                                  |
                |                     | Candidate Proposed               |
                |                     v                                  |
                |            +------------------+                        |
                | [Esc]/[2]  | CONFIRMING_PLAN  |                        |
                +------------+  (Modal Dialog)  |                        |
                | Cancel     +--------+---------+                        |
                |                     |                                  |
                |                     | [1] Confirm Execution            |
                |                     v                                  |
                |            +------------------+                        |
                |   [F3]     |  RECEIPT_ACTIVE  |                        |
                +------------+  (Receipt Card)  |------------------------+
                   Undo      +------------------+     [Enter] Done
```

### 5.1 Keyboard Interaction Map
| Keycode / Key | Active State | Resulting Action |
| :--- | :--- | :--- |
| **Typing (A–Z, 0–9, etc.)** | `EDITING_DOCUMENT` | Inserts character into active document buffer |
| **Backspace** | `EDITING_DOCUMENT` | Removes preceding character |
| **F1** | Any | Creates a new document buffer |
| **F2** | `EDITING_DOCUMENT` | Flushes document buffer to disk via INOS-FS persistent inode |
| **Tab / 'd'** | Any | Toggles **Engineer Diagnostics View** (CPL=3, HMAC, LBA, telemetry) |
| **Esc / '2'** | `CONFIRMING_PLAN` | Cancels plan proposal without capability grant; returns to canvas |
| **'1' / Enter** | `CONFIRMING_PLAN` | Authorizes proposal, mints HMAC grant, executes atomic WAL transaction |
| **F3 / 'u'** | `RECEIPT_ACTIVE` | Safely undos committed transaction, rolls back disk state, updates canvas |
| **F10** | Any | Flushes caches and issues clean hardware ACPI/QEMU reboot |

### 5.2 Model-Free Guarantee
If the system boots without model weights attached:
1. The kernel skips neural weight ingestion without hanging or warning dialogs.
2. The habitable desktop appears immediately with an empty or existing document canvas.
3. The user can type, edit, save (`F2`), and navigate files deterministically.
4. Model intelligence is strictly optional assistance; the computer remains a dependable writing and computing tool.

---

## 6. Component Anatomy & Natural Human Communication

### 6.1 Natural Confirmation Modal
Rather than technical jargon (`INTENT PROPOSAL — VERIFICATION REQUIRED`, `CANDIDATE: OP_DUPLICATE_SAFE`), the modal communicates in plain human terms:

```
+-----------------------------------------------------------------------+
|  Make a copy of draft.txt?                                            |
|                                                                       |
|  Your original file will stay where it is.                            |
|  A copy will be saved as draft.txt.bak.                               |
|                                                                       |
|  +-----------------------------------------------------------------+  |
|  | [1] Make copy                               (Safe, recommended) |  |
|  +-----------------------------------------------------------------+  |
|  | [2] Cancel                                     (Keep as-is)     |  |
|  +-----------------------------------------------------------------+  |
|                                                                       |
|  [Tab: Details & Security Proofs]                                     |
+-----------------------------------------------------------------------+
```

### 6.2 Transaction Receipt Card
Following confirmed execution, a serene receipt card confirms what occurred and makes the undo action obvious:
```
+-----------------------------------------------------------------------+
|  Copy created: draft.txt.bak                                          |
|                                                                       |
|  Your original file draft.txt was kept intact.                        |
|  Status: Saved durably to disk.                                       |
|                                                                       |
|  Press [F3] at any time to undo this copy safely.                     |
+-----------------------------------------------------------------------+
```

### 6.3 Engineer Diagnostics View (Strictly Separated)
When the user or auditor presses `Tab` or selects `Details...`, the everyday view seamlessly transitions into the technical telemetry view:
- Ring 3 User-space Execution Status (`CPL=3, W^X Enforced`)
- Model Architecture & Parameters (`SmolLM2-135M-Instruct, 30 layers, 9 heads`)
- Token Logit Traces & Generated Token IDs (`[9654, 47921, 30, 9047, 23597]`)
- Explicit Provenance Ledger (`src: MODEL_GENERATED, dst: EXPLICIT_CONVENTION`)
- HMAC-SHA-256 Capability Grant ID & Nonce (`Grant: 1001, Nonce: 0xF106_0135`)
- Storage Geometry (`ATA Drive 1, Sector LBA 12..15, WAL Checkpoint: Active`)

---

## 7. Verification Invariants for Gate UI-01

To fulfill Gate UI-01, the implementation must satisfy six rigorous acceptance invariants:
1. **Model-Free Operation:** A full lifecycle (create, type, save, reboot) executes cleanly without model weights loaded.
2. **First-Class Typography:** Text rendered with proportional advance metrics, baseline alignment, and typographic hierarchy.
3. **Contrast Compliance:** All text colors meet WCAG AA contrast against their background surfaces ($> 4.5:1$).
4. **Natural Human Phrasing:** Confirmation and receipt components use natural English sentences rather than machine syntax.
5. **Authentic Screendump Checksums:** Every captured framebuffer screendump produces a genuinely unique SHA-256 digest reflecting distinct UI state transitions.
6. **Backend Contract Preservation:** All F106.1 security boundaries (HMAC capability verification, single-use ledger, 7 causal counterfactual tests, WAL crash recovery) remain 100% intact.
