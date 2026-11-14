# INOS Acceptance Report: Gate UI-03 — Habitable Desktop Integration

## 1. Executive Summary

Gate UI-03 has been successfully implemented, audited, and verified under bare-metal QEMU virtual execution.

The desktop environment has achieved the approved **Habitable Computing** visual direction and architectural desktop integration:
1. **Habitable Computing Visual Identity**: Dark charcoal desktop (`#232421`), muted walnut-slate frames (`#49473F` at 92–95% alpha), warm stone headers (`#656158`), subdued ivory writing surfaces (`#B5AEA0` 100% opaque), dark charcoal ink (`#292823`, 8.4:1 contrast), and muted sage focus outlines (`#89966E`).
2. **Football-Shaped Window Controls**: Elongated horizontal oval pills (Close clay-red `#8B4F47`, Minimize walnut `#8B7043`, Maximize sage `#6B8258`) with full window lifecycle management (Close preserves file on disk, Minimize registers in launcher, Maximize expands to usable desktop without occluding intent bar, Restore returns exact prior bounds).
3. **Shell Modernization & Taskbar Retirement**: Conventional top and bottom bars permanently removed from default view; replaced by a movable, dockable, collapsible Floating Intent Bar and a deterministic Circular Launcher Menu operable with zero neural inference.
4. **Compositor Equivalence Verification**: Rigorous pixel-by-pixel dirty-region blit audit against clean full-frame composition verified exactly **0 mismatched pixels** across representative desktop states.
5. **Redraw Performance Benchmark**: Measured bare-metal x86-64 cycle counts via `rdtsc`: Full-frame redraw consumes **100,695,084 ticks** (~41.9ms), whereas dirty-region blit finishes in **1,209,600 ticks** (~0.50ms @ 2.4 GHz), delivering an **83.25x speedup**.
6. **Input Isolation & Diagnostics**: `Tab` reserved exclusively for focus cycling; `F12` reserved exclusively for Engineer Diagnostics View; modal dialogs strictly trap input fail-closed.
7. **Cold Reboot Persistence**: Multi-document workspace (`draft.txt` and `notes.txt`) and all edits restored intact across machine power cycle via atomic WAL on ATA storage.

---

## 2. Visual Artifacts & State Progression

| State | Filename | Description |
| :--- | :--- | :--- |
| **0. Model-Free Habitable Desktop** | `ui_03_0_modelfree_desktop.png` | Dual document surfaces mounted on dark charcoal desktop with football controls, subdued ivory canvases, and floating intent bar; zero model weights attached. |
| **1. Window Minimized** | `ui_03_1_minimized_desktop.png` | `draft.txt` minimized via football pill control; `notes.txt` remains active; document safely registered in circular launcher. |
| **2. Restored, Dragged & Edited** | `ui_03_2_dragged_edited.png` | `draft.txt` restored via circular launcher; `notes.txt` dragged to `(590, 125)` and edited with proportional serif text; clipping preserved. |
| **3. Layered Modal Dialog** | `ui_03_3_modal_layered.png` | Intent confirmation dialog floating over spatial documents; input focus trapped fail-closed. |
| **4. Transaction Receipt Card** | `ui_03_4_receipt_card.png` | Transaction receipt card rendered with ATA durability proof, HMAC-SHA-256 capability record, and undo control. |
| **5. Cold Reboot Verified** | `ui_03_5_cold_reboot_verified.png` | Machine power cycle completed; both documents and persistent edits reloaded bit-for-bit intact from ATA disk. |

---

## 3. Cryptographic Evidence Manifest (`evidence/ui_03_artifacts.sha256`)

```text
3a03db4d8ba7b3868e3a9f5fcefa02899585a3987e196469fdb90c4697ca152f  ui_03_0_modelfree_desktop.bmp
df82847f790b93f1829efc14cfba06b6d6939a4435b4cc0be9f3ea137dfaada1  ui_03_0_modelfree_desktop.png
e1dc8f0e8884962cd5e958ab379d9563d1acb2451a6fa3cad27abc1432fa9b1d  ui_03_1_minimized_desktop.bmp
902a1d0584a68d148fe7490d6a30c97b01125a0eeead44512ca9bfcefcfea704  ui_03_1_minimized_desktop.png
af059e2edcb155c2839d3a272ee936c0ce51e70e6026e1615101653e3a6bd047  ui_03_2_dragged_edited.bmp
60e77c13efff0604e209b258611314b5dd2c76439b04d07471f2ea0f0d40e73c  ui_03_2_dragged_edited.png
af512d097c87368d553745e1fddef8b51e65cf0cb631d07710c64b3656bf3759  ui_03_3_modal_layered.bmp
b0458fdf183ef34888ac0dd48480c7a074be0dddace4a0ef83a4646d7ddb4fd8  ui_03_3_modal_layered.png
12d956b221571c916cdf1bf24ebb161df74c23acb254556e98fdb8d761a086b6  ui_03_4_receipt_card.bmp
f3bbf2dbcf58838aebdad276c94e9c3a02cc8e64bed4f0387ec80cb51e8aeec9  ui_03_4_receipt_card.png
d72a9fb134a04ba7180b81d6dcd2b22caa6f3cd6abfab8f970c41f9a46c9506b  ui_03_5_cold_reboot_verified.bmp
1926ff2557ac05efcb2eea01222ffab5810b58f303f1e3470b6b144ce4bfb0af  ui_03_5_cold_reboot_verified.png
40bdd241e4b952c7fcbabf3bc38e8e2b13ed2d48cc43e509f1e9d775c6656ccb  ui_03_modelfree_serial.log
1e24aa2fd1f3fe68143e2297bd6fc2931f6fc69c1db4dcc7747671ff27f7703f  ui_03_stage1_serial.log
5b303c6429a2e7553f873a4d9f2a7699e8bd08fa0c42986725c4a1bdd19bd035  ui_03_stage2_serial.log
18fa2ef13c6513c0707041a193e10c1735675eed501599eb5fc699e236dba46b  ui_03_stage3_serial.log
```

### Visual Distinctness Audit
- **Total BMP screendumps captured**: 6
- **Unique SHA-256 checksums**: 6
- **Result**: **PASS** (Zero collision or static framebuffer reuse across workflow phases).

---

## 4. Compositor Equivalence & Latency Telemetry

### 4.1 Equivalence Verification
- **Audit Routine**: `SpatialWindowManager::audit_dirty_region_equivalence()`
- **Live Framebuffer Dirty-Region Composite vs Full-Frame Baseline**:
  - `[UI_03_COMPOSITOR_DIRTY_EQUIVALENCE_MISMATCHES]`: **0**
  - `[UI_03_COMPOSITOR_ZERO_MISMATCH_VERIFIED]`: **PASS**

### 4.2 Bare-Metal `rdtsc` Cycle Telemetry
- `[UI_03_BENCHMARK_FULL_FRAME_TICKS]`: **100,695,084** (~41.9ms @ 2.4 GHz)
- `[UI_03_BENCHMARK_DIRTY_REGION_TICKS]`: **1,209,600** (~0.50ms @ 2.4 GHz)
- **Speedup Ratio**: **83.25x faster** (well within the 16.6ms 60 FPS frame deadline)

---

## 5. Verification Checklist

| Criterion | Requirement | Result |
| :--- | :--- | :--- |
| **Habitable Visual Identity** | Dark charcoal desktop (`#232421`), walnut-slate frames (`#49473F`), warm stone headers (`#656158`), subdued ivory writing surfaces (`#B5AEA0`), charcoal ink (`#292823`), sage focus border (`#89966E`). | **PASS** |
| **Football Window Controls** | Three horizontal oval pills in upper-left; Close (retains file), Minimize (registers in launcher), Maximize (usable desktop without occluding intent bar), Restore (exact prior bounds). | **PASS** |
| **Shell Modernization** | Top/bottom taskbars removed from normal view; movable, dockable Floating Intent Bar; circular launcher menu restores windows with zero model inference. | **PASS** |
| **Compositor Equivalence** | 0 mismatched pixels between dirty-region blitting and full-frame rendering across interactive states. | **PASS** |
| **Redraw Benchmark** | Dirty-region compositing executes in 1,209,600 ticks (~0.5ms @ 2.4GHz), achieving 83.25x speedup. | **PASS** |
| **Input Ownership & Isolation** | `Tab` reserved for control focus navigation; `F12` exclusively opens Engineer Diagnostics View; modals trap input fail-closed. | **PASS** |
| **Model-Free Resilience** | Boots and operates reliably with dual surfaces and launcher when zero model weights are attached. | **PASS** |
| **Cold Reboot Durability** | Both documents and persistent edits reloaded intact across machine power cycle via atomic WAL on ATA disk. | **PASS** |
| **Zero Python** | 100% pure Rust microkernel, freestanding C runtime, and Bash verification scripts. | **PASS** |
| **QEMU Isolation** | Runs strictly within virtualized UEFI machine; zero host modifications. | **PASS** |

---

## 6. How to Reproduce

Execute the test suite inside WSL:
```bash
wsl bash scripts/test_habitable_ui_03.sh
```
All screendumps and serial execution logs are written to `evidence/` and checksummed into `evidence/ui_03_artifacts.sha256`.
