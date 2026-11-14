#!/usr/bin/env bash
# scripts/build_kernel.sh - Builds the INOS x86-64 Rust Kernel and packages EFI boot image

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
export PATH="${HOME}/.cargo/bin:${PATH}"

if [ "${SKIP_BUILD:-0}" = "1" ] && [ -f "${ROOT_DIR}/build/esp/EFI/BOOT/BOOTX64.EFI" ]; then
    echo "=== [INOS] Using freshly compiled BOOTX64.EFI (SKIP_BUILD=1) ==="
    exit 0
fi

echo "=== [INOS] Building User-Space Init ELF (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/user_init"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Notes App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/notes_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Files App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/files_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Calc App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/calc_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Archive App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/archive_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Grid App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/grid_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Spin App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/spin_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Net App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/net_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -C target-feature=+sse,+sse2 -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 IntScript App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/intscript_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Script App Wrapper (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/script_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Write App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/write_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Plan App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/plan_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 PDF App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/pdf_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Mail App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/mail_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Flow App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/flow_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Canvas App (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/canvas_app"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Sentinel Security Console (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/security_console"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release

echo "=== [INOS] Building Native Ring 3 Web Renderer (x86_64-unknown-none) ==="
cd "${ROOT_DIR}/userspace/apps/web_render"
RUSTFLAGS="-C link-arg=-Tlink.ld -C relocation-model=static -A warnings" cargo build --target x86_64-unknown-none --release


echo "=== [INOS] Building inapp_packager Tool ==="
cd "${ROOT_DIR}/tools/format_validator"
cargo build --release

cd "${ROOT_DIR}/tools/inapp_packager"
cargo build --release

mkdir -p "${ROOT_DIR}/build/packages"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/notes_app/target/x86_64-unknown-none/release/notes_app" --app-id org.inos.notes --name Notes --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/notes.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/files_app/target/x86_64-unknown-none/release/files_app" --app-id org.inos.files --name Files --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/files.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/calc_app/target/x86_64-unknown-none/release/calc_app" --app-id org.inos.calc --name Calc --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/calc.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/archive_app/target/x86_64-unknown-none/release/archive_app" --app-id org.inos.archive --name Archive --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/archive.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/grid_app/target/x86_64-unknown-none/release/grid_app" --app-id org.inos.grid --name Grid --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/grid.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/spin_app/target/x86_64-unknown-none/release/spin_app" --app-id org.inos.spin --name Spin --version 1 --caps 0 --out "${ROOT_DIR}/build/packages/spin.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/net_app/target/x86_64-unknown-none/release/net_app" --app-id org.inos.net --name Net --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/net.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/script_app/target/x86_64-unknown-none/release/script_app" --app-id org.inos.script --name Script --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/script.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/intscript_app/target/x86_64-unknown-none/release/intscript_app" --app-id org.inos.intscript --name IntScript --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/intscript.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/write_app/target/x86_64-unknown-none/release/write_app" --app-id org.inos.write --name Write --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/write.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/plan_app/target/x86_64-unknown-none/release/plan_app" --app-id org.inos.plan --name Plan --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/plan.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/pdf_app/target/x86_64-unknown-none/release/pdf_app" --app-id org.inos.pdf --name PDF --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/pdf.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/mail_app/target/x86_64-unknown-none/release/mail_app" --app-id org.inos.mail --name Mail --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/mail.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/flow_app/target/x86_64-unknown-none/release/flow_app" --app-id org.inos.flow --name Flow --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/flow.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/canvas_app/target/x86_64-unknown-none/release/canvas_app" --app-id org.inos.canvas --name Canvas --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/canvas.inapp"
"${ROOT_DIR}/tools/inapp_packager/target/release/inapp_packager" --elf "${ROOT_DIR}/userspace/apps/security_console/target/x86_64-unknown-none/release/security_console" --app-id org.inos.security_console --name "Security Console" --version 1 --caps 1 --out "${ROOT_DIR}/build/packages/security_console.inapp"

echo "=== [INOS] Building Native OCaml Ring 3 Intent Runtime ==="
bash "${ROOT_DIR}/userspace/user_intent_runtime/build.sh"

echo "=== [INOS] Building x86-64 Kernel (UEFI Target) ==="
cd "${ROOT_DIR}/kernel"
sync
RUSTFLAGS="-A warnings" cargo build --release --target x86_64-unknown-uefi -j 1

mkdir -p "${ROOT_DIR}/build/esp/EFI/BOOT"
cp "${ROOT_DIR}/kernel/target/x86_64-unknown-uefi/release/inos_kernel.efi" "${ROOT_DIR}/build/esp/EFI/BOOT/BOOTX64.EFI"

echo "=== [INOS] Kernel image packaged at build/esp/EFI/BOOT/BOOTX64.EFI ==="
