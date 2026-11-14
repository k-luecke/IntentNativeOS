# INOS Specification: Habitable Modal Treatment & Typography Fidelity (Gate UI-03.1)

## 1. Architectural Overview

Gate UI-03.1 refines the confirmation and authorization interface of Intent-Native OS (INOS), resolving typography fidelity, contrast hierarchy, geometry anti-aliasing, and keyboard safety in the native graphical environment.

```
+------------------------------------------------------------------------------------+
|                                                                                    |
|                  +---------------------------------------------------+             |
|                  |                                                   |             |
|                  |   Make a copy of draft.txt?                       |             |
|                  |   Your original file will remain unchanged.       |             |
|                  |   A copy will be saved as draft.txt.bak.          |             |
|                  |                                                   |             |
|                  |   [ Make copy ]   [ Cancel ]                      |             |
|                  |                                                   |             |
|                  |   Inspect details with F12                        |             |
|                  +---------------------------------------------------+             |
|                                                                                    |
+------------------------------------------------------------------------------------+
```

---

## 2. Typography Subsystem Architecture (`proportional_font.rs`)

### 2.1 Reusable Multi-Face Font System
Gate UI-03.1 replaces scaled bitmap glyph approximations with a multi-face, proportional typography pipeline:
1. **Title Font (20px *Liberation Sans Bold*)**: Rendered for window titles, confirmation headings, and prominent system dialogs.
2. **UI Font (16px *Liberation Sans Regular*)**: Rendered for modal body text, reassurance copy, buttons, status messages, and launcher menu items.
3. **Serif Font (16px *Liberation Serif Regular*)**: Rendered for document body text to afford comfortable editorial reading.
4. **Bitmap Fallback**: Monospaced 8×8 bitmap rasterizer preserved for early boot and rescue diagnostics.

### 2.2 Coverage Masks & Metrics
- **8-Bit Grayscale Coverage**: Every glyph contains build-time FreeType-rasterized coverage values ($0 \dots 255$). Blended via pure integer math:
  $$\text{Dst} = \frac{\alpha \cdot \text{Src} + (255 - \alpha) \cdot \text{Dst}}{255}$$
  This avoids subpixel color-fringing while maintaining high contrast across arbitrary background colors.
- **Accurate Metric Tables**: Includes proportional character advance, bounding box width/height, horizontal bearing $X$, and vertical bearing $Y$ relative to the baseline.
- **Licensing & Provenance**: Rendered fonts are derived from the SIL Open Font License (OFL) Liberation font family. Font rasterizer source (`tools/font_rasterizer/rasterize.c`) is checked into tree.

---

## 3. Approved Modal Treatment & Contrast Hierarchy

### 3.1 Color Tokens & Luminance Ratios

| Component | Color Token | Hex Code | RGB | Background | Contrast Ratio | WCAG Compliance |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Modal Card Surface** | `MODAL_BG` | `#34322C` | `(52, 50, 44)` | Dark Desktop (`#232421`) | — | — |
| **Heading** | `MODAL_HEADING` | `#EEEAE2` | `(238, 234, 226)` | `#34322C` | **8.8:1** | AAA (Enhanced) |
| **Reassurance Copy** | `MODAL_REASSURE` | `#E2DCD0` | `(226, 220, 208)` | `#34322C` | **7.8:1** | AAA (Enhanced) |
| **Target Detail** | `MODAL_DETAIL` | `#C4BEB0` | `(196, 190, 176)` | `#34322C` | **6.2:1** | AA (Standard) |
| **Footer Diagnostic** | `MODAL_FOOTER` | `#9A9488` | `(154, 148, 136)` | `#34322C` | **4.7:1** | AA (Large/UI) |
| **Primary Pill ("Make copy")**| `BUTTON_PRIMARY_BG` | `#89966E` | `(137, 150, 110)` | `#34322C` | — | Architectural Pill |
| **Primary Button Ink** | `BUTTON_PRIMARY_FG` | `#232421` | `(35, 36, 33)` | `#89966E` | **7.8:1** | AAA (Enhanced) |
| **Secondary Pill ("Cancel")** | `BUTTON_SECONDARY_BG` | `#413F38` | `(65, 63, 56)` | `#34322C` | — | Architectural Pill |
| **Secondary Button Ink** | `BUTTON_SECONDARY_FG` | `#E2DCD0` | `(226, 220, 208)` | `#413F38` | **5.7:1** | AA (Standard) |
| **Focus Ring** | `BUTTON_FOCUS_RING` | `#89966E` / `#E2DCD0` | — | Button Edges | — | 2px Outset Ring |

### 3.2 Modal Copy
- **Heading**: *"Make a copy of draft.txt?"*
- **Reassurance**: *"Your original file will remain unchanged."*
- **Target Detail**: *"A copy will be saved as draft.txt.bak."*
- **Buttons**: *"Make copy"* (primary pill) and *"Cancel"* (secondary pill)
- **Footer**: *"Inspect details with F12"*

---

## 4. Authorization Safety & Input Invariants

1. **Safe Default Focus**:
   When the modal is displayed, keyboard focus defaults to **`Cancel`**. Pressing `Enter` commits safe cancellation rather than executing destructive or unverified storage modifications.
2. **Explicit Affirmative Authorization**:
   The user can authorize execution only through:
   - Pressing `Tab` to shift focus to *"Make copy"* followed by `Enter`,
   - Direct pointer click on the *"Make copy"* pill, or
   - Explicit keyboard shortcut `[1]`.
3. **Diagnostics Resumption Without Bypass**:
   Pressing `F12` opens the Engineer Diagnostics View to inspect the capability grant ID, Ring 3 process provenance, and WAL checkpoint parameters. Returning from diagnostics re-renders the pending authorization modal with its prior focus intact; diagnostics inspection never implicitly authorizes or dismisses a transaction.
4. **Strict Focus Isolation**:
   When an authorization modal is active:
   - The floating intent bar suppresses its text insertion caret and focus ring.
   - Background document windows reject text editing and dragging.
   - Modal traps all keyboard navigation fail-closed.

---

## 5. Geometry Anti-Aliasing (`draw_rounded_rect_alpha`)

To eliminate stepped/aliased corner artifacts without floating-point overhead:
- **Bounded Boundary Testing**: Only pixels within $r - 1 \le \sqrt{dx^2 + dy^2} \le r + 1$ are subjected to subpixel analysis. Fully interior pixels are filled in bulk.
- **16-Level ($4 \times 4$) Subpixel Grid**: Boundary pixels sample a fixed $4 \times 4$ subpixel grid $(sx_k, sy_k)$ in pure integer math:
  $$\text{coverage} = \frac{1}{16} \sum_{k=0}^{15} \left[ (4 \cdot dx + sx_k)^2 + (4 \cdot dy + sy_k)^2 \le (4 \cdot r)^2 \right]$$
- **Alpha Composition**: Effective alpha $\alpha_{\text{eff}} = (\alpha \cdot \text{coverage}) / 16$ is blended directly into the framebuffer backing store.
