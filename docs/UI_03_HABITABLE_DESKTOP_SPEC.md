# INOS Specification: Habitable Desktop Integration (Gate UI-03)

## 1. Architectural Overview

Gate UI-03 integrates the approved **Habitable Computing** visual identity into the native Intent-Native OS (INOS) desktop environment. Building upon Gate UI-02's spatial multi-document foundation, UI-03 eliminates conventional operating-system chrome (top/bottom taskbars) in favor of a quiet, architectural, document-centered space:

```
+------------------------------------------------------------------------------------+
|                                                                                    |
|   +-----------------------+                                                        |
|   | (-) (-) (+) draft.txt |     +-------------------------+                        |
|   | Plain Text Document   |     | (-) (-) (+) notes.txt   |                        |
|   |-----------------------|     | Notes & Arch References |                        |
|   | project draft         |     |-------------------------|                        |
|   |                       |     | habitable workspace     |                        |
|   |                       |     | - spatial continuity|   |                        |
|   |                       |     +-------------------------+                        |
|   +-----------------------+                                                        |
|                                                                                    |
|                  +------------------------------------------+                      |
|                  | (O) Search, run command, or describe ... |                      |
|                  +------------------------------------------+                      |
+------------------------------------------------------------------------------------+
```

---

## 2. Approved Visual Identity & Design System

### 2.1 Color Palette & Materials
The UI-03 palette achieves high legibility (document body text contrast exceeds 8.4:1) while establishing an architectural, tranquil ambiance:

| Element | Hex Value | RGB Channels | Purpose / Opacity |
| :--- | :--- | :--- | :--- |
| **Desktop Background** | `#232421` | `(35, 36, 33)` | Dark charcoal, deep architectural grounding |
| **Window Frame** | `#49473F` | `(73, 71, 63)` | Muted walnut-slate, rendered with alpha blending (92–95%) |
| **Window Header** | `#656158` | `(101, 97, 88)` | Warm stone title bar header |
| **Document Surface** | `#B5AEA0` | `(181, 174, 160)` | Subdued ivory writing sheet (100% opaque, anti-fatigue) |
| **Document Text Ink** | `#292823` | `(41, 40, 35)` | Deep charcoal ink for high contrast reading |
| **Header / Metadata Ink**| `#DDD8CE` | `(221, 216, 206)`| Cream metadata and document title labels |
| **Active Focus Border** | `#89966E` | `(137, 150, 110)`| Muted sage outline indicating primary input focus |
| **Inactive Window Border**| `#383630` | `(56, 54, 48)` | Subdued frame delimiter |
| **Control - Close** | `#8B4F47` | `(139, 79, 71)` | Muted clay-red pill control |
| **Control - Minimize** | `#8B7043` | `(139, 112, 67)`| Muted walnut-ochre pill control |
| **Control - Maximize** | `#6B8258` | `(107, 130, 88)`| Muted sage green pill control |

### 2.2 Corner Radii & Geometry
- **Window Outer Frames**: 10–12px rounded corners (`draw_rounded_rect_alpha`).
- **Internal Document Sheets**: 7–9px rounded corners (`draw_rounded_rect_alpha`).
- **Pill Controls**: Elongated horizontal ovals (width 14px, height 8px) with 3px spacing.
- **Floating Intent Bar**: 32px pill height, 540px width, docked at bottom center (`y = 744`), 16px corner radius.
- **Circular Launcher**: 20×20 circular pill widget (`(388, 750)` to `(408, 770)`) rendered inside the intent bar with sage accent (`#89966E`).

---

## 3. Subsystem Architecture

### 3.1 Football-Shaped Window Controls (`kernel/src/graphics/spatial.rs`)
Window title bars feature three elongated horizontal oval pill controls positioned in the upper-left of each window:
- **Close Pill** (`#8B4F47`): Left pill. Invoking close transitions window state to `WindowState::Closed`. The document itself remains persistently preserved on disk (`INOS-FS`), and can be reopened at any time.
- **Minimize Pill** (`#8B7043`): Middle pill. Transitions window state to `WindowState::Minimized`. The window is hidden from workspace compositing and registered in the circular launcher menu.
- **Maximize / Restore Pill** (`#6B8258`): Right pill. Toggles between:
  - `WindowState::Maximized { prev_bounds }`: Expands window to fill desktop space (`(10, 10)` to `(1270, 730)`), keeping the floating intent bar (`y = 744..776`) accessible and unoccluded.
  - `WindowState::Normal`: Restores the window to its exact pre-maximize bounds and z-order.

### 3.2 Shell Modernization & Floating Intent Bar
- **Taskbar Elimination**: The legacy top and bottom taskbars are permanently retired from normal desktop view.
- **Floating Intent Bar (`FloatingIntentBar`)**:
  - Encapsulates system query, command input, and intent dispatch.
  - Docked by default at bottom center (`x = 370, y = 744, width = 540, height = 32`).
  - Supports movable positioning and collapsed state (`is_collapsed`).
  - Renders an integrated circular launcher pill on the left.
- **Deterministic Circular Launcher (`LauncherMenu`)**:
  - Restores minimized documents and launches native tools deterministically without model inference or network reliance.
  - Displays registered system actions (e.g. `Restore draft.txt`, `Open Terminal`, `Diagnostics (F12)`).

### 3.3 Compositor Equivalence & Dirty-Region Audit
The compositor implements strict pixel-for-pixel mathematical equivalence between fast incremental dirty-region blitting and clean full-frame rendering:
- **Audit Primitive (`audit_dirty_region_equivalence`)**:
  - Runs across representative interaction states (minimized, restored, dragged, layered).
  - Renders workspace via incremental dirty-region updates into the live framebuffer.
  - Computes pixel-by-pixel difference against an off-screen reference frame rendered via full-frame composition.
  - Verification threshold: Exactly **0 mismatched pixels** required.

---

## 4. Input Ownership & Focus Model

1. **Tab Key**: Reserved strictly for focus cycling across visible windows, controls, and floating intent fields. Tab is never usurped by command strings.
2. **F12 Key**: Reserved exclusively as a global toggle for the Engineer Diagnostics View.
3. **Modal Focus Trap**: Confirmation modals (e.g., plan approval) capture all keyboard and pointer input fail-closed. Underlying document surfaces reject input until modal resolution (Confirm/Cancel).
4. **Mouse Drag & Caret**: Clicking window content sets the text insertion caret; dragging the title bar repositions the window with strict clamping to desktop bounds (`[10, 1270] × [10, 730]`).

---

## 5. Cold-Reboot Persistence & WAL

All document edits, window arrangements, and intent executions are backed by the atomic Write-Ahead Log (WAL) and INOS-FS on primary ATA storage:
- Document bodies are committed synchronously prior to receipt emission.
- Machine cold reboots replay uncommitted operations safely and reload active documents bit-for-bit intact without data loss.
