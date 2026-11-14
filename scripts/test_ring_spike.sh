#!/usr/bin/env bash
set -euo pipefail

cat << 'EOF' > /tmp/ring_probe/src/main.rs
#![no_std]
#![no_main]

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicUsize, Ordering};
use rustls::unbuffered::ConnectionState;

struct BumpAlloc {
    arena: [u8; 512 * 1024],
    offset: AtomicUsize,
}

unsafe impl Sync for BumpAlloc {}

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let mut cur = self.offset.load(Ordering::Relaxed);
        loop {
            let aligned = (cur + align - 1) & !(align - 1);
            let next = aligned + size;
            if next > self.arena.len() {
                return core::ptr::null_mut();
            }
            match self.offset.compare_exchange_weak(cur, next, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => {
                    let ptr = (&raw const self.arena) as *mut u8;
                    return ptr.add(aligned);
                }
                Err(actual) => cur = actual,
            }
        }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOCATOR: BumpAlloc = BumpAlloc {
    arena: [0u8; 512 * 1024],
    offset: AtomicUsize::new(0),
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    for b in buf.iter_mut() {
        *b = 0x42;
    }
    Ok(())
}

getrandom::register_custom_getrandom!(custom_getrandom);

#[derive(Debug)]
struct InosTimeProvider;
impl rustls::time_provider::TimeProvider for InosTimeProvider {
    fn current_time(&self) -> Option<rustls::pki_types::UnixTime> {
        Some(rustls::pki_types::UnixTime::since_unix_epoch(core::time::Duration::from_secs(1728000000)))
    }
}

#[no_mangle]
pub extern "C" fn __stack_chk_fail() -> ! {
    loop {}
}

enum StepAction {
    Transmit(usize),
    NeedMoreData,
    Closed,
}

fn step_tls(
    conn: &mut rustls::client::UnbufferedClientConnection,
    incoming: &mut [u8],
    outgoing: &mut [u8],
    app_data_to_send: Option<&[u8]>,
    response_body: &mut Vec<u8>,
) -> Result<(usize, StepAction), rustls::Error> {
    let status = conn.process_tls_records(incoming);
    let discard = status.discard;

    let action = match status.state {
        Ok(ConnectionState::EncodeTlsData(mut enc)) => {
            let written = enc.encode(outgoing)
                .map_err(|_| rustls::Error::General(alloc::string::String::from("Encode error")))?;
            StepAction::Transmit(written)
        }
        Ok(ConnectionState::TransmitTlsData(tx)) => {
            tx.done();
            StepAction::Transmit(0)
        }
        Ok(ConnectionState::BlockedHandshake) => {
            StepAction::NeedMoreData
        }
        Ok(ConnectionState::WriteTraffic(mut wt)) => {
            if let Some(data) = app_data_to_send {
                let written = wt.encrypt(data, outgoing)
                    .map_err(|_| rustls::Error::General(alloc::string::String::from("Encrypt error")))?;
                StepAction::Transmit(written)
            } else {
                StepAction::NeedMoreData
            }
        }
        Ok(ConnectionState::ReadTraffic(mut rt)) => {
            while let Some(res) = rt.next_record() {
                let record = res?;
                response_body.extend_from_slice(record.payload);
            }
            StepAction::NeedMoreData
        }
        Ok(ConnectionState::PeerClosed) | Ok(ConnectionState::Closed) => {
            StepAction::Closed
        }
        Err(e) => {
            return Err(e);
        }
        _ => {
            StepAction::Closed
        }
    };

    Ok((discard, action))
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let provider = rustls::crypto::ring::default_provider();
    let root_store = rustls::RootCertStore::empty();
    let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
        Arc::new(root_store),
        Arc::new(provider.clone()),
    )
    .build()
    .unwrap();

    let config = rustls::ClientConfig::builder_with_details(
        Arc::new(provider),
        Arc::new(InosTimeProvider),
    )
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .dangerous()
    .with_custom_certificate_verifier(verifier)
    .with_no_client_auth();

    let server_name = rustls_pki_types::ServerName::try_from("host.test").unwrap().to_owned();
    let mut conn = rustls::client::UnbufferedClientConnection::new(Arc::new(config), server_name).unwrap();

    let mut incoming_buf = [0u8; 4096];
    let incoming_len = 0;
    let mut outgoing_buf = [0u8; 4096];
    let mut response_body = Vec::new();

    let _ = step_tls(
        &mut conn,
        &mut incoming_buf[..incoming_len],
        &mut outgoing_buf,
        Some(b"GET /hello HTTP/1.1\r\n\r\n"),
        &mut response_body,
    );

    loop {}
}
EOF

export PATH="$HOME/.cargo/bin:${PATH}"
cd /tmp/ring_probe
RUSTFLAGS="-C target-feature=+sse,+sse2" cargo build --release --target x86_64-unknown-none
echo "COMPLETE STEP_TLS ENGINE COMPILED CLEANLY!"
