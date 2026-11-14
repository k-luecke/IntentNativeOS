# kernel/ (Trust Boundary T0)

Minimal protected kernel, UEFI bootloader, physical memory allocator, page table manager, interrupt controllers, and bare-metal VirtIO drivers.

- **Language:** Rust (with minimal assembly)
- **Target:** x86-64 UEFI (`x86_64-unknown-none` / `x86_64-unknown-uefi`)
- **Authority:** T0. Highest privilege. No models or complex parsers execute here.
