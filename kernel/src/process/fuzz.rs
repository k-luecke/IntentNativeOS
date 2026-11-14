// kernel/src/process/fuzz.rs - Syscall and IPC Parser Fuzzing Engine (K022)

use crate::arch::x86_64::syscall::{dispatch_syscall, SyscallFrame};
use crate::ipc::{BoundedIpcQueue, IpcMessage, IpcMessageType};

pub const FUZZ_MUTATION_COUNT: usize = 64;

pub fn run_fuzz_corpus() -> usize {
    let mut safe_mutations = 0;

    // 1. Syscall Gate Fuzz Corpus
    let test_nrs: [u64; 8] = [0, 9999, u64::MAX, 0x8000_0000_0000_0000, 1, 4, 6, 8];
    let test_args: [u64; 8] = [0, 1, 4096, 0xDEADBEEF, u64::MAX, 0xFFFF_8000_0000_0000, 0x1000, 3];

    for &nr in test_nrs.iter() {
        for &arg in test_args.iter() {
            let frame = SyscallFrame {
                nr,
                arg1: arg,
                arg2: arg.wrapping_add(1),
                arg3: arg.wrapping_sub(1),
                arg4: arg ^ 0xAAAA_5555_AAAA_5555,
                arg5: 0,
            };

            // Must never panic; either Ok or Err is handled safely
            let _ = dispatch_syscall(frame);
            safe_mutations += 1;
        }
    }

    // 2. IPC Message Queue Fuzz Corpus
    let mut queue = BoundedIpcQueue::new();
    let fuzz_payloads: [&[u8]; 8] = [
        b"",
        b"\x00\xFF\xAA\x55",
        b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        b"\x7F\x45\x4C\x46\x00\x00\x00\x00",
        b"malformed\x00intent",
        b"\xFF\xFF\xFF\xFF",
        b"0123456789abcdef",
        b"\xDE\xAD\xBE\xEF\xCA\xFE\xBA\xBE",
    ];

    for (idx, payload) in fuzz_payloads.iter().enumerate() {
        let msg = IpcMessage::new(
            (idx as u64).wrapping_mul(0x1337),
            idx as u64,
            (idx as u64).wrapping_add(10),
            IpcMessageType::DirectIntent,
            payload,
        );

        let _ = queue.enqueue(msg);
        let _ = queue.cancel_message((idx as u64).wrapping_mul(0x1337));
        let _ = queue.dequeue();
    }

    safe_mutations
}
