#!/usr/bin/env bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:${PATH}"

cat << 'EOF' > /tmp/test_pki/src/main.rs
extern crate alloc;

use rustls_pki_types::CertificateDer;
use webpki::anchor_from_trusted_cert;

fn main() {
    let der_bytes = include_bytes!("certs/ca.der");
    let cert_der = CertificateDer::from(&der_bytes[..]);
    let early_res = anchor_from_trusted_cert(&cert_der);
    let size = core::mem::size_of_val(&early_res);
    let bytes = unsafe { core::slice::from_raw_parts(&early_res as *const _ as *const u8, size) };
    println!("test_pki early_res size: {}", size);
    println!("test_pki early_res bytes: {:02x?}", &bytes[..core::cmp::min(48, size)]);
    println!("test_pki early_res is_ok: {}", early_res.is_ok());
}
EOF

cd /tmp/test_pki
cargo run --release
