// userspace/sdk/src/lib.rs - Standard Application SDK for Intent-Native OS

#![no_std]

pub mod syscall;
pub mod compositor;
pub mod net;
pub mod allocator;

pub use syscall::{
    rights, SyscallError, AutomationIpcWire, AUTOMATION_ABI_MAGIC,
    CandidateProposal, IntSpoolHeader, PROPOSAL_MAGIC,
    OP_TYPE_UNKNOWN, OP_TYPE_COPY, OP_TYPE_DELETE, OP_TYPE_ARCHIVE,
    PROVENANCE_NONE, PROVENANCE_MODEL_GENERATED, PROVENANCE_EXPLICIT_CONVENTION,
    OP_DISCOVER, OP_OPEN, OP_GET_CELL, OP_SET_CELL, OP_EVALUATE, OP_SAVE,
    STATUS_OK, STATUS_NOT_FOUND, STATUS_TYPE_MISMATCH, STATUS_FORMULA_ERROR, STATUS_PERMISSION_DENIED,
};
pub use allocator::BoundedHeapAllocator;

/// Cooperatively yields execution quantum to sibling process
pub fn yield_now() {
    unsafe {
        let _ = syscall::raw_syscall(syscall::SYS_YIELD, 0, 0, 0, 0, 0);
    }
}

/// Checks current CPU privilege level (CPL) via Code Segment selector
#[inline(always)]
pub fn get_cpl() -> u16 {
    let cs: u16;
    unsafe {
        core::arch::asm!("mov {:x}, cs", out(reg) cs, options(nomem, nostack));
    }
    cs & 3
}

/// Print debug string to kernel serial console
pub fn debug_print(msg: &str) {
    unsafe {
        let _ = syscall::raw_syscall(
            syscall::SYS_DEBUG_PRINT,
            msg.as_ptr() as u64,
            msg.len() as u64,
            0,
            0,
            0,
        );
    }
}

/// Get current process ID
pub fn get_pid() -> u64 {
    unsafe {
        syscall::raw_syscall(syscall::SYS_GET_PID, 0, 0, 0, 0, 0).unwrap_or(0)
    }
}

static mut CAP_TOKEN_TABLE: [(u64, u64); 32] = [(0, 0); 32];
static mut CAP_TOKEN_CURSOR: usize = 0;

/// Records a (low, high) 128-bit capability token pair in the process-local lookup table
pub fn record_cap_token(low: u64, high: u64) {
    unsafe {
        for i in 0..32 {
            if CAP_TOKEN_TABLE[i].0 == low && (CAP_TOKEN_TABLE[i].0 != 0 || CAP_TOKEN_TABLE[i].1 != 0) {
                CAP_TOKEN_TABLE[i].1 = high;
                return;
            }
        }
        let idx = CAP_TOKEN_CURSOR % 32;
        CAP_TOKEN_TABLE[idx] = (low, high);
        CAP_TOKEN_CURSOR = CAP_TOKEN_CURSOR.wrapping_add(1);
    }
}

/// Resolves a 64-bit capability token handle (low word) to its full 128-bit token
pub fn resolve_full_token(low: u64) -> u128 {
    if low <= 0xFFFF {
        return low as u128;
    }
    unsafe {
        for i in 0..32 {
            let (k, high) = CAP_TOKEN_TABLE[i];
            if k == low && (k != 0 || high != 0) {
                return ((high as u128) << 64) | (low as u128);
            }
        }
    }
    low as u128
}

/// Queries the immutable kernel-authenticated installed package identity for a given PID
pub fn get_caller_identity(target_pid: u64, buf: &mut [u8]) -> Result<usize, SyscallError> {
    unsafe {
        let res = syscall::raw_syscall(
            syscall::SYS_GET_CALLER_IDENTITY,
            target_pid,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
            0,
        )?;
        Ok(res as usize)
    }
}

/// Request a scoped capability token from trusted OS policy for a target path
pub fn request_capability(path: &str, requested_rights: u32) -> Result<u64, SyscallError> {
    let token_128 = request_capability_128(path, requested_rights)?;
    let low = token_128 as u64;
    let high = (token_128 >> 64) as u64;
    record_cap_token(low, high);
    Ok(low)
}

/// Request a 128-bit scoped capability token from trusted OS policy
pub fn request_capability_128(path: &str, requested_rights: u32) -> Result<u128, SyscallError> {
    unsafe {
        let (low, high) = syscall::raw_syscall_128(
            syscall::SYS_CAP_REQUEST,
            path.as_ptr() as u64,
            path.len() as u64,
            requested_rights as u64,
            0,
            0,
        )?;
        record_cap_token(low, high);
        Ok(((high as u128) << 64) | (low as u128))
    }
}

/// Create a new file in INOS-FS using an authorized capability token
pub fn file_create(path: &str, cap_token: u64) -> Result<(), SyscallError> {
    let token_128 = resolve_full_token(cap_token);
    file_create_128(path, token_128, 0)
}

/// Create a new file in INOS-FS using an authorized 128-bit capability token and optional transaction
pub fn file_create_128(path: &str, cap_token: u128, tx_id: u64) -> Result<(), SyscallError> {
    let token_low = cap_token as u64;
    let token_high = (cap_token >> 64) as u64;
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_FILE_CREATE,
            path.as_ptr() as u64,
            path.len() as u64,
            tx_id,
            token_low,
            token_high,
        )?;
        Ok(())
    }
}

/// Write data to an existing file in INOS-FS using an authorized capability token
pub fn file_write(path: &str, content: &[u8], cap_token: u64) -> Result<usize, SyscallError> {
    let token_128 = resolve_full_token(cap_token);
    file_write_128(path, content, token_128)
}

/// Write data to a file in INOS-FS using an authorized 128-bit capability token
pub fn file_write_128(path: &str, content: &[u8], cap_token: u128) -> Result<usize, SyscallError> {
    let packed_lens = ((path.len() as u64) << 32) | ((content.len() as u64) & 0xFFFF_FFFF);
    let token_low = cap_token as u64;
    let token_high = (cap_token >> 64) as u64;
    unsafe {
        let res = syscall::raw_syscall(
            syscall::SYS_FILE_WRITE,
            path.as_ptr() as u64,
            packed_lens,
            content.as_ptr() as u64,
            token_low,
            token_high,
        )?;
        Ok(res as usize)
    }
}

/// Read data from an existing file in INOS-FS using an authorized capability token
pub fn file_read(path: &str, buf: &mut [u8], cap_token: u64) -> Result<usize, SyscallError> {
    let token_128 = resolve_full_token(cap_token);
    file_read_128(path, buf, token_128)
}

/// Read data from an existing file in INOS-FS using an authorized 128-bit capability token
pub fn file_read_128(path: &str, buf: &mut [u8], cap_token: u128) -> Result<usize, SyscallError> {
    let packed_lens = ((path.len() as u64) << 32) | ((buf.len() as u64) & 0xFFFF_FFFF);
    let token_low = cap_token as u64;
    let token_high = (cap_token >> 64) as u64;
    unsafe {
        let res = syscall::raw_syscall(
            syscall::SYS_FILE_READ,
            path.as_ptr() as u64,
            packed_lens,
            buf.as_mut_ptr() as u64,
            token_low,
            token_high,
        )?;
        Ok(res as usize)
    }
}

/// List files in INOS-FS into `buf` formatted as `name:size\n` lines
pub fn file_list(buf: &mut [u8], cap_token: u64) -> Result<usize, SyscallError> {
    let token_128 = resolve_full_token(cap_token);
    let token_low = token_128 as u64;
    let token_high = (token_128 >> 64) as u64;
    unsafe {
        let res = syscall::raw_syscall(
            syscall::SYS_FILE_LIST,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            token_low,
            token_high,
            0,
        )?;
        Ok(res as usize)
    }
}

/// Atomically rename a file in INOS-FS using an authorized capability token
pub fn file_rename(old_path: &str, new_path: &str, cap_token: u64) -> Result<(), SyscallError> {
    let token_128 = resolve_full_token(cap_token);
    let token_low = token_128 as u64;
    let token_high = (token_128 >> 64) as u64;
    let packed_lens = ((new_path.len() as u64) << 32) | ((old_path.len() as u64) & 0xFFFF_FFFF);
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_FILE_RENAME,
            old_path.as_ptr() as u64,
            packed_lens,
            new_path.as_ptr() as u64,
            token_low,
            token_high,
        )?;
        Ok(())
    }
}

/// Delete a file in INOS-FS using an authorized capability token
pub fn file_delete(path: &str, cap_token: u64) -> Result<(), SyscallError> {
    let token_128 = resolve_full_token(cap_token);
    let token_low = token_128 as u64;
    let token_high = (token_128 >> 64) as u64;
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_FILE_DELETE,
            path.as_ptr() as u64,
            path.len() as u64,
            token_low,
            token_high,
            0,
        )?;
        Ok(())
    }
}

/// Begin a new multi-file transaction in INOS-FS
pub fn tx_begin(flags: u64) -> Result<u64, SyscallError> {
    unsafe {
        syscall::raw_syscall(syscall::SYS_TX_BEGIN, flags, 0, 0, 0, 0)
    }
}

/// Commit an active transaction atomically to disk
pub fn tx_commit(tx_id: u64) -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(syscall::SYS_TX_COMMIT, tx_id, 0, 0, 0, 0)?;
        Ok(())
    }
}

/// Rollback an active transaction, discarding all staged mutations
pub fn tx_rollback(tx_id: u64) -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(syscall::SYS_TX_ROLLBACK, tx_id, 0, 0, 0, 0)?;
        Ok(())
    }
}

/// Returns provisional UTC epoch seconds and raw monotonic time ticks
pub fn get_time() -> (u64, u64) {
    let mut utc_secs: u64 = 0;
    let mut mono_ticks: u64 = 0;
    unsafe {
        let _ = syscall::raw_syscall(
            syscall::SYS_GET_TIME,
            &mut utc_secs as *mut u64 as u64,
            &mut mono_ticks as *mut u64 as u64,
            0,
            0,
            0,
        );
    }
    (utc_secs, mono_ticks)
}

/// Fills destination buffer with cryptographically secure random bytes
pub fn get_random(buf: &mut [u8]) -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_GET_RANDOM,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
            0,
            0,
        )?;
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptIsolationContext {
    pub script_id: [u8; 32],
    pub path_count: u32,
    pub permissions: u32,
    pub allowed_paths: [[u8; 64]; 8],
}

impl ScriptIsolationContext {
    pub const fn empty() -> Self {
        Self {
            script_id: [0u8; 32],
            path_count: 0,
            permissions: 0,
            allowed_paths: [[0u8; 64]; 8],
        }
    }
}

/// Activate script-level capability isolation context
pub fn set_script_context(ctx: &ScriptIsolationContext) -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_SET_SCRIPT_CONTEXT,
            ctx as *const ScriptIsolationContext as u64,
            core::mem::size_of::<ScriptIsolationContext>() as u64,
            0,
            0,
            0,
        )?;
        Ok(())
    }
}

/// Clear script-level capability isolation context
pub fn clear_script_context() -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_CLEAR_SCRIPT_CONTEXT,
            0,
            0,
            0,
            0,
            0,
        )?;
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationRequest {
    pub src_pkg: [u8; 32],
    pub dst_pkg: [u8; 32],
    pub requested_rights: u32,
    pub migration_cap_token: u64,
}

impl MigrationRequest {
    pub fn new(src: &str, dst: &str, rights: u32, cap_token: u64) -> Self {
        let mut req = Self {
            src_pkg: [0u8; 32],
            dst_pkg: [0u8; 32],
            requested_rights: rights,
            migration_cap_token: cap_token,
        };
        let s_bytes = src.as_bytes();
        let s_len = core::cmp::min(s_bytes.len(), 32);
        req.src_pkg[..s_len].copy_from_slice(&s_bytes[..s_len]);

        let d_bytes = dst.as_bytes();
        let d_len = core::cmp::min(d_bytes.len(), 32);
        req.dst_pkg[..d_len].copy_from_slice(&d_bytes[..d_len]);
        req
    }
}

/// Request authenticated package identity migration via trusted migration service
pub fn migrate_package_identity(req: &MigrationRequest) -> Result<u64, SyscallError> {
    unsafe {
        let (low, high) = syscall::raw_syscall_128(
            syscall::SYS_MIGRATE_PACKAGE_IDENTITY,
            req as *const MigrationRequest as u64,
            core::mem::size_of::<MigrationRequest>() as u64,
            0,
            0,
            0,
        )?;
        record_cap_token(low, high);
        Ok(low)
    }
}

/// Dispatches raw or typed IPC message to target PID
pub fn ipc_send_msg(target_pid: u64, data: &[u8]) -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_IPC_SEND,
            target_pid,
            data.as_ptr() as u64,
            data.len() as u64,
            0,
            0,
        )?;
        Ok(())
    }
}

/// Sends a typed IPC message to another process
pub fn ipc_send(target_pid: u64, msg_type: u32, payload: &[u8]) -> Result<u64, SyscallError> {
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_IPC_SEND,
            target_pid,
            payload.as_ptr() as u64,
            payload.len() as u64,
            msg_type as u64,
            0,
        )
    }
}

/// Receives raw or typed IPC message from target channel/PID
pub fn ipc_recv_msg(channel_id: u64, buf: &mut [u8]) -> Result<usize, SyscallError> {
    unsafe {
        let res = syscall::raw_syscall(
            syscall::SYS_IPC_RECV,
            channel_id,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
            0,
        )?;
        Ok(res as usize)
    }
}

/// Receives a typed IPC message sent to this process
pub fn ipc_recv(buf: &mut [u8]) -> Result<(u64, u32, usize), SyscallError> {
    let mut meta = [0u64; 3]; // [sender_pid, msg_type, payload_len]
    unsafe {
        let res = syscall::raw_syscall(
            syscall::SYS_IPC_RECV,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            meta.as_mut_ptr() as u64,
            0,
            0,
        )?;
        Ok((meta[0], meta[1] as u32, res as usize))
    }
}

/// Submits a structured natural-language CandidateProposal to the kernel Semantic Control Plane
pub fn submit_intent_proposal(proposal: &CandidateProposal) -> Result<(), SyscallError> {
    unsafe {
        syscall::raw_syscall(
            syscall::SYS_SUBMIT_INTENT_PROPOSAL,
            proposal as *const CandidateProposal as u64,
            core::mem::size_of::<CandidateProposal>() as u64,
            0,
            0,
            0,
        )?;
        Ok(())
    }
}

/// Sends a 128-byte binary AutomationIpcWire frame to a target application PID
pub fn send_automation_ipc(
    target_pid: u64,
    msg_type: u32,
    wire: &AutomationIpcWire,
) -> Result<u64, SyscallError> {
    ipc_send(target_pid, msg_type, wire.as_bytes())
}

/// Receives and validates a 128-byte binary AutomationIpcWire frame from the IPC queue
pub fn recv_automation_ipc() -> Result<(u64, u32, AutomationIpcWire), SyscallError> {
    let mut buf = [0u8; 256];
    let (sender_pid, msg_type, len) = ipc_recv(&mut buf)?;
    match AutomationIpcWire::from_bytes(&buf[..len]) {
        Some(wire) => Ok((sender_pid, msg_type, wire)),
        None => Err(SyscallError::InvalidArgument),
    }
}

/// Decodes a 512-byte `INT_` On-Board Intelligence Spool Header (LBA 24576 ABI)
pub fn parse_int_spool_header(sector: &[u8]) -> Option<IntSpoolHeader> {
    IntSpoolHeader::parse_from_sector(sector)
}

/// Normal process exit
pub fn exit(code: u64) -> ! {
    loop {
        unsafe {
            let _ = syscall::raw_syscall(syscall::SYS_EXIT, code, 0, 0, 0, 0);
            core::arch::asm!("ud2");
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return (a as i32) - (b as i32);
        }
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        *dest.add(i) = *src.add(i);
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    for i in 0..n {
        *s.add(i) = c as u8;
    }
    s
}
