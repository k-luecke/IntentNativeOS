# Gate G3 Live Integration Audit & Fidelity Assessment

**Conducted at:** 2026-10-09 (Autonomous Engineering Supervisor)  
**Target:** x86-64 QEMU / OVMF Reference Platform (`build/esp/EFI/BOOT/BOOTX64.EFI`)

---

## 1. Executive Summary & Classification Matrix

| Subsystem / Capability | Audit Finding | Classification | Gap ID |
| :--- | :--- | :--- | :--- |
| **1. OCaml User-Space Process Execution** | Ring 3 executes a freestanding Rust binary (`user_init`). OCaml runtime (`ocamlrun` or native OCaml) is NOT yet cross-compiled to freestanding `x86_64-unknown-none`. Wire protocol and message framing are implemented via Rust IPC bridge. | **Simulated Protocol Implementation** | `GAP-G3-01` |
| **2. Keyboard & Framebuffer Hardware Devices** | PS/2 input decoding and compositor work correctly on an internal linear RGB buffer (`G3_FRAMEBUFFER`). However, QEMU runs with `-display none`, and physical QEMU display devices (UEFI GOP, VirtIO-GPU, Bochs VBE) are not yet bound. Events are injected programmatically. | **Simulated / Synthetic in-kernel** | `GAP-G3-02` |
| **3. Filesystem Persistence across QEMU Restarts** | INOS-FS format, inode, directory, read/write, and WAL crash recovery logic verified on an in-memory block device (`MemoryBlockDevice`). A persistent virtual disk image file (`.img`) mounted across two independent QEMU processes was not attached. | **Simulated / In-Memory** | `GAP-G3-03` |
| **4. Real Model A/B Inference** | Model A and B arbitration verifies plan contracts, structural invariants, and token/memory telemetry. Predefined and deterministic evaluation logic was used; no live local LLM inference engine (Ollama/llama.cpp) was active during the boot run. | **Simulated** | `GAP-G3-04` |
| **5. Interactive Human Desktop & File Undo** | The complete user workflow (browse file $\rightarrow$ intent submit $\rightarrow$ clarify $\rightarrow$ A/B plan $\rightarrow$ sandboxed transform $\rightarrow$ receipt $\rightarrow$ undo) executed automatically as a kernel verification pipeline emitting serial markers; no interactive human GUI loop was exposed to the host. | **Simulated / Automated Pipeline** | `GAP-G3-05` |

---

## 2. Executable Artifacts & Cryptographic Hashes

| Artifact Path | Target Architecture | Size (Bytes) | SHA-256 Digest |
| :--- | :--- | :--- | :--- |
| `build/esp/EFI/BOOT/BOOTX64.EFI` | `x86_64-unknown-uefi` | 136,192 | `f1b82cbe6a291dd1a6e19f0ef8cb33f168f1f1fe650295cbaf910ef57b965aff` |
| `userspace/user_init` | `x86_64-unknown-none` | 5,464 | `407fa15b8cd090a11072a413b7d1c551718afd44b80e05eef7e444903169d393` |
| `kernel/target/x86_64-unknown-uefi/debug/inos_kernel.efi` | `x86_64-unknown-uefi` | 136,192 | `f1b82cbe6a291dd1a6e19f0ef8cb33f168f1f1fe650295cbaf910ef57b965aff` |

---

## 3. Detailed Audit Findings

### 3.1 OCaml Runtime in User-Space
- **Current State:** The freestanding binary executed in Ring 3 is `userspace/user_init/src/main.rs`. It executed with CPL=3, invoked `syscall` (`SYS_GET_VERSION` returning `0x0100`), attempted unauthorized access to `0xFFFF_8000_0000_0000`, triggered `#PF (Vector 14, Error Code 4: User=1, Write=0, Present=0)`, and was safely terminated by the kernel.
- **Limitation:** OCaml compiler modules (`compiler/ir`, `compiler/operation`, `models/adapter`) currently compile under host Linux/WSL via Dune. They do not yet compile to a freestanding ELF binary executing directly in Ring 3 on the custom kernel.
- **Integration Gap:** `GAP-G3-01`: Cross-compile minimal OCaml runtime / C runtime shim or wasm micro-runtime into Ring 3 user process to execute compiled OCaml bytecode natively.

### 3.2 Hardware-Exercised Virtual Devices (Display & Input)
- **Current State:** `scripts/launch_qemu.sh` executes QEMU with `-display none -serial stdio`. Input events (`KeyCode::KeyA`, mouse packets) were pushed directly to `Ps2InputDriver::queue`. Framebuffer rendering (`FB_WIDTH=128, FB_HEIGHT=96`) modified a kernel buffer (`G3_FRAMEBUFFER`).
- **Limitation:** Host user cannot see the desktop window or click on UI elements because QEMU's graphical display device is not enabled and the framebuffer is not blitted to UEFI GOP or VirtIO-GPU MMIO.
- **Integration Gap:** `GAP-G3-02`: Query UEFI GOP (`EFI_GRAPHICS_OUTPUT_PROTOCOL`) during boot, map physical LFB, blit compositor surface to physical screen, and configure QEMU with `-display gtk` or `-display sdl` with VirtIO-keyboard/mouse.

### 3.3 Disk Persistence Across Shutdown / Reboot
- **Current State:** The filesystem driver (`InosFileSystem`) and Write-Ahead Log (`AtomicJournal`) were exercised on `MemoryBlockDevice`. Crash recovery was validated by simulating memory state before and after commit.
- **Limitation:** The block device resides in volatile kernel `.bss`. When QEMU exits, all blocks are lost.
- **Integration Gap:** `GAP-G3-03`: Create a persistent 10 MB raw disk image `build/storage.img`, pass `-drive file=build/storage.img,if=virtio,format=raw` to QEMU, implement VirtIO-block PCI driver or IDE ATA PIO driver, and verify file survival across distinct QEMU runs.

### 3.4 Real Model Inference
- **Current State:** `ModelArbitrationEngine` computes token counts proportionally based on contract text length, checks for forbidden policy strings (`MALICIOUS`, `ESCAPE_CAP`), and returns `ArbitrationResult::Agreed`.
- **Limitation:** No quantized GGUF weights or local inference engine (e.g. `llama.cpp`) are loaded into QEMU guest memory.
- **Integration Gap:** `GAP-G3-04`: Connect host-side Ollama / llama-server via QEMU user-networking or loopback socket (`S004`) to query genuine local model inference.

### 3.5 Interactive Human Desktop
- **Current State:** Automated determinism ensures 100% reproducible boot testing without human delay.
- **Limitation:** A human operator cannot currently type into the running desktop without an interactive event loop reading from COM1 UART stdin or graphical window events.
- **Integration Gap:** `GAP-G3-05`: Add interactive shell/event loop reading scancodes from COM1 UART or GUI keyboard with nonblocking timeout.

---

## 4. Integration Gap Action Plan

These tasks are tracked alongside Gate G4 development without modifying the established backlog history:

1. **`GAP-G3-01`**: Port freestanding bytecode runner to user-space Ring 3.
2. **`GAP-G3-02`**: Connect UEFI GOP physical linear framebuffer and enable GUI display in QEMU.
3. **`GAP-G3-03`**: Real VirtIO block disk persistence across separate QEMU boot instances.
4. **`GAP-G3-04`**: Live local model socket bridge over QEMU user-net adapter.
5. **`GAP-G3-05`**: Interactive serial/keyboard input listener for human-driven file manipulation and undo.
