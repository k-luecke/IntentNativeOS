# Third-Party Dependencies & Build Reproducibility Manifest

This manifest documents all vendored third-party code in INOS, their upstream origins, pinned versions, package checksums, and intentional patches required for bare-metal `x86_64-unknown-none` operation.

---

## 1. Vendored Packages

### A. `ring`
* **Version**: `0.17.14`
* **Upstream**: `https://github.com/briansmith/ring` (crates.io package `ring 0.17.14`)
* **Target Architecture**: `x86_64-unknown-none`
* **Purpose**: Cryptographic primitives for TLS 1.3 (Curve25519 / X25519, ChaCha20-Poly1305, AES-GCM, SHA-256/384, ECDSA, RSA PKCS#1 / PSS).
* **Applied Patches**:
  * **File**: `third_party/ring/build.rs` line 131
  * **Diff**:
    ```diff
    --- a/third_party/ring/build.rs
    +++ b/third_party/ring/build.rs
    @@ -128,6 +128,7 @@ fn cpp_flags(compiler: &cc::Tool) -> &'static [&'static str] {
         if !compiler.is_like_msvc() {
             static NON_MSVC_FLAGS: &[&str] = &[
                 "-fvisibility=hidden",
    +            "-fno-stack-protector",
                 "-std=c1x",
    ```
  * **Rationale**: On Linux x86-64, GCC enables `-fstack-protector-strong` by default when compiling C sources (`crypto/curve25519/curve25519.c`), generating canary accesses at `%fs:0x28`. On bare-metal Ring 3 before the OS initializes thread-local storage / `FS_BASE`, dereferencing `%fs:0x28` triggers Vector 14 (#PF, CR2 = 0x28). Adding `-fno-stack-protector` allows the C primitives to compile without unmapped segment dereferences.
  * **Roadmap**: INOS kernel task manager will initialize `MSR_FS_BASE` per process with a hardware-seeded canary value at `FS:[0x28]`, enabling full stack canary validation across all user-space libraries.

### B. `rustls-webpki`
* **Version**: `0.103.15`
* **Upstream**: `https://github.com/rustls/webpki` (crates.io package `rustls-webpki 0.103.15`)
* **Target Architecture**: `x86_64-unknown-none` (features: `ring`, `alloc`, `no_std`)
* **Purpose**: X.509 certificate path validation, TrustAnchor root store validation, expiration validation, and strict Subject Alternative Name (SAN) name constraint enforcement.
* **Applied Patches**: Zero code modifications (standard `no_std + alloc` configuration).

---

## 2. Compiler & Flag Specifications

All userspace applications linking these cryptographic libraries (`net_app`) must be compiled with the following deterministic `RUSTFLAGS` in `scripts/build_kernel.sh`:
```bash
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -C target-feature=+sse,+sse2,-soft-float"
```
* `-C target-feature=+sse,+sse2`: Enables hardware 128-bit vector instructions required for cryptographic algorithms and System V AMD64 ABI compliance.
* `-C target-feature=-soft-float`: Explicitly disables the default LLVM soft-float memcpy lowering on `x86_64-unknown-none`, preventing strided 2-byte vector store bugs.
* `-C relocation-model=static`: Prevents generation of position-independent GOT/PLT indirections incompatible with the static Ring 3 ELF loader.
