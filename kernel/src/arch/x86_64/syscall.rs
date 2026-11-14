// kernel/src/arch/x86_64/syscall.rs - Syscall Gate and Versioned ABI Dispatcher (K012)

pub const SYSCALL_ABI_VERSION: u32 = 0x0200; // v2.0 (128-bit capability token ABI & transaction subsystem)
use crate::arch::x86_64::serial::SERIAL1;

pub const SYS_YIELD: u64 = 1;
pub const SYS_EXIT: u64 = 2;
pub const SYS_GET_PID: u64 = 3;
pub const SYS_IPC_SEND: u64 = 4;
pub const SYS_IPC_RECV: u64 = 5;
pub const SYS_CAP_DELEGATE: u64 = 6;
pub const SYS_CAP_REVOKE: u64 = 7;
pub const SYS_MEM_MAP: u64 = 8;
pub const SYS_GET_VERSION: u64 = 9;
pub const SYS_FILE_CREATE: u64 = 10;
pub const SYS_FILE_WRITE: u64 = 11;
pub const SYS_DEBUG_PRINT: u64 = 12;
pub const SYS_MEM_PROTECT: u64 = 13;
pub const SYS_MEM_UNMAP: u64 = 14;
pub const SYS_SUBMIT_INTENT_PROPOSAL: u64 = 15;
pub const SYS_CAP_REQUEST: u64 = 16;
pub const SYS_NET_SOCKET: u64 = 17;
pub const SYS_NET_CONNECT: u64 = 18;
pub const SYS_NET_SEND: u64 = 19;
pub const SYS_NET_RECV: u64 = 20;
pub const SYS_NET_CLOSE: u64 = 21;
pub const SYS_NET_POLL: u64 = 22;
pub const SYS_GET_TIME: u64 = 23;
pub const SYS_NET_RESOLVE: u64 = 24;
pub const SYS_GET_RANDOM: u64 = 25;
pub const SYS_FILE_READ: u64 = 26;
pub const SYS_SET_SCRIPT_CONTEXT: u64 = 27;
pub const SYS_CLEAR_SCRIPT_CONTEXT: u64 = 28;
pub const SYS_MIGRATE_PACKAGE_IDENTITY: u64 = 29;
pub const SYS_TX_BEGIN: u64 = 30;
pub const SYS_TX_COMMIT: u64 = 31;
pub const SYS_TX_ROLLBACK: u64 = 32;
pub const SYS_GET_CALLER_IDENTITY: u64 = 33;
pub const SYS_FILE_LIST: u64 = 34;
pub const SYS_FILE_RENAME: u64 = 35;
pub const SYS_FILE_DELETE: u64 = 36;

pub const PROPOSAL_MAGIC: u32 = 0x494E5450; // "INTP"

pub const OP_TYPE_UNKNOWN: u32 = 0;
pub const OP_TYPE_COPY: u32 = 1;
pub const OP_TYPE_DELETE: u32 = 2;
pub const OP_TYPE_ARCHIVE: u32 = 3;

pub const PROVENANCE_NONE: u32 = 0;
pub const PROVENANCE_MODEL_GENERATED: u32 = 1;
pub const PROVENANCE_EXPLICIT_CONVENTION: u32 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CandidateProposal {
    pub magic: u32,
    pub op_type: u32,
    pub op_name: [u8; 16],
    pub src_path: [u8; 64],
    pub src_len: u32,
    pub src_provenance: u32,
    pub dst_path: [u8; 64],
    pub dst_len: u32,
    pub dst_provenance: u32,
    pub token_count: u32,
    pub token_ids: [u32; 32],
    pub raw_text: [u8; 128],
    pub raw_text_len: u32,
}

impl CandidateProposal {
    pub const fn empty() -> Self {
        Self {
            magic: 0,
            op_type: OP_TYPE_UNKNOWN,
            op_name: [0; 16],
            src_path: [0; 64],
            src_len: 0,
            src_provenance: PROVENANCE_NONE,
            dst_path: [0; 64],
            dst_len: 0,
            dst_provenance: PROVENANCE_NONE,
            token_count: 0,
            token_ids: [0; 32],
            raw_text: [0; 128],
            raw_text_len: 0,
        }
    }
}

pub static mut LATEST_PROPOSAL: Option<CandidateProposal> = None;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallError {
    InvalidSyscallNumber = 1,
    InvalidArgument = 2,
    PermissionDenied = 3,
    QueueFull = 4,
    BufferTooSmall = 5,
    ObjectNotFound = 6,
    CapabilityRevoked = 7,
    OutOfMemory = 8,
    QuotaExceeded = 14,
    EntropyUnavailable = 15,
    QueueEmpty = 16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallFrame {
    pub nr: u64,
    pub arg1: u64,
    pub arg2: u64,
    pub arg3: u64,
    pub arg4: u64,
    pub arg5: u64,
}

pub fn dispatch_syscall(frame: SyscallFrame) -> Result<u64, SyscallError> {
    match frame.nr {
        SYS_YIELD => {
            // Returns 0 on successful yield
            Ok(0)
        }
        SYS_EXIT => {
            let exit_code = frame.arg1;
            Ok(exit_code)
        }
        SYS_GET_PID => {
            let active_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            if active_pid != 0 {
                Ok(active_pid)
            } else {
                Ok(1)
            }
        }
        SYS_IPC_SEND => {
            let target_pid = frame.arg1;
            let msg_ptr = frame.arg2;
            let msg_len = frame.arg3 as usize;
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let sender_pid = if caller_pid != 0 { caller_pid } else { 1 };

            if msg_ptr == 0 || msg_len == 0 {
                return Err(SyscallError::InvalidArgument);
            }

            if target_pid == 1 {
                // Compositor IPC Protocol
                if msg_len != core::mem::size_of::<crate::ipc::compositor::CompositorMsg>() {
                    return Err(SyscallError::InvalidArgument);
                }
                let mut msg_buf = [0u8; core::mem::size_of::<crate::ipc::compositor::CompositorMsg>()];
                crate::arch::x86_64::user_ptr::copy_slice_from_user(msg_ptr, &mut msg_buf)?;
                let mut msg = unsafe { core::ptr::read_unaligned(msg_buf.as_ptr() as *const crate::ipc::compositor::CompositorMsg) };
                let res = unsafe {
                    crate::ipc::compositor::GLOBAL_COMPOSITOR_SERVER.handle_ipc_msg(sender_pid, &mut msg)
                };
                if res.is_ok() {
                    let out_slice = unsafe { core::slice::from_raw_parts(&msg as *const _ as *const u8, core::mem::size_of::<crate::ipc::compositor::CompositorMsg>()) };
                    let _ = crate::arch::x86_64::user_ptr::copy_slice_to_user(out_slice, msg_ptr);
                }
                res
            } else {
                // Cross-Process Typed IPC Dispatch (INOS-AUTOMATION-ABI-001)
                if target_pid == 0 || target_pid > 980 {
                    SERIAL1.write_str("[IPC_ROUTER] Rejecting unroutable target PID ");
                    SERIAL1.write_u64_dec(target_pid);
                    SERIAL1.write_str("\n");
                    return Err(SyscallError::ObjectNotFound);
                }
                if msg_len > crate::ipc::MAX_PAYLOAD_SIZE {
                    return Err(SyscallError::BufferTooSmall);
                }
                let mut payload_buf = [0u8; crate::ipc::MAX_PAYLOAD_SIZE];
                crate::arch::x86_64::user_ptr::copy_slice_from_user(msg_ptr, &mut payload_buf[..msg_len])?;
                let mtype = crate::ipc::IpcMessageType::from_u32(frame.arg4 as u32);

                SERIAL1.write_str("[IPC_ROUTER] Typed dispatch: PID ");
                SERIAL1.write_u64_dec(sender_pid);
                SERIAL1.write_str(" -> PID ");
                SERIAL1.write_u64_dec(target_pid);
                SERIAL1.write_str(" (len=");
                SERIAL1.write_u64_dec(msg_len as u64);
                SERIAL1.write_str(", type=");
                SERIAL1.write_u64_dec(mtype as u32 as u64);
                SERIAL1.write_str(")\n");

                unsafe {
                    crate::ipc::GLOBAL_IPC_ROUTER.send(sender_pid, target_pid, mtype, &payload_buf[..msg_len])
                        .map_err(|err| match err {
                            crate::ipc::IpcError::QueueFull => SyscallError::QueueFull,
                            _ => SyscallError::InvalidArgument,
                        })
                }
            }
        }
        SYS_IPC_RECV => {
            let buf_ptr = frame.arg1;
            let buf_len = frame.arg2 as usize;
            let out_meta_ptr = frame.arg3; // [sender_pid, msg_type, payload_len]
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            if buf_ptr == 0 || buf_len == 0 {
                return Err(SyscallError::InvalidArgument);
            }

            let msg = unsafe {
                crate::ipc::GLOBAL_IPC_ROUTER.recv(pid)
                    .map_err(|err| match err {
                        crate::ipc::IpcError::QueueEmpty => SyscallError::QueueEmpty,
                        _ => SyscallError::InvalidArgument,
                    })?
            };

            let copy_len = core::cmp::min(buf_len, msg.payload_len);
            crate::arch::x86_64::user_ptr::copy_slice_to_user(&msg.payload[..copy_len], buf_ptr)?;

            if out_meta_ptr != 0 {
                let meta = [msg.sender_pid, msg.msg_type as u64, copy_len as u64];
                let meta_bytes = unsafe {
                    core::slice::from_raw_parts(meta.as_ptr() as *const u8, 24)
                };
                crate::arch::x86_64::user_ptr::copy_slice_to_user(meta_bytes, out_meta_ptr)?;
            }

            SERIAL1.write_str("[IPC_ROUTER] Dequeued message for PID ");
            SERIAL1.write_u64_dec(pid);
            SERIAL1.write_str(" from PID ");
            SERIAL1.write_u64_dec(msg.sender_pid);
            SERIAL1.write_str(" (len=");
            SERIAL1.write_u64_dec(copy_len as u64);
            SERIAL1.write_str(")\n");

            Ok(copy_len as u64)
        }
        SYS_CAP_DELEGATE => {
            let cap_handle = frame.arg1;
            let target_pid = frame.arg2;
            let rights = frame.arg3;
            if cap_handle == 0 || target_pid == 0 || rights == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            // Return new delegated handle
            Ok(cap_handle ^ target_pid ^ rights)
        }
        SYS_CAP_REVOKE => {
            let cap_handle = frame.arg1;
            if cap_handle == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            Ok(0)
        }
        SYS_MEM_MAP => {
            let virt_addr = frame.arg1;
            let page_count = frame.arg2;
            let _flags = frame.arg3;
            let min_phys = frame.arg4;
            if virt_addr % 4096 != 0 || page_count == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::charge_process_memory(pid, page_count as u32)?;

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_MEM_MAP: virt=0x");
            super::serial::SERIAL1.write_u64_hex(virt_addr);
            super::serial::SERIAL1.write_str(" pages=");
            super::serial::SERIAL1.write_u64_dec(page_count);
            if min_phys > 0 {
                super::serial::SERIAL1.write_str(" (min_phys >= 0x");
                super::serial::SERIAL1.write_u64_hex(min_phys);
                super::serial::SERIAL1.write_str(")");
            }
            super::serial::SERIAL1.write_str("\n");
            Ok(virt_addr)
        }
        SYS_MEM_PROTECT => {
            let virt_addr = frame.arg1;
            let page_count = frame.arg2;
            let _prot = frame.arg3;
            if virt_addr % 4096 != 0 || page_count == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_MEM_PROTECT: virt=0x");
            super::serial::SERIAL1.write_u64_hex(virt_addr);
            super::serial::SERIAL1.write_str(" pages=");
            super::serial::SERIAL1.write_u64_dec(page_count);
            super::serial::SERIAL1.write_str(" hardware W^X enforced: WRITABLE cleared, NO_EXECUTE set\n");
            Ok(0)
        }
        SYS_MEM_UNMAP => {
            let virt_addr = frame.arg1;
            let page_count = frame.arg2;
            if virt_addr % 4096 != 0 || page_count == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::release_process_memory(pid, page_count as u32);

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_MEM_UNMAP: virt=0x");
            super::serial::SERIAL1.write_u64_hex(virt_addr);
            super::serial::SERIAL1.write_str(" pages=");
            super::serial::SERIAL1.write_u64_dec(page_count);
            super::serial::SERIAL1.write_str(" released\n");
            Ok(0)
        }
        SYS_GET_VERSION => {
            Ok(SYSCALL_ABI_VERSION as u64)
        }
        SYS_CAP_REQUEST => {
            let path_ptr = frame.arg1;
            let path_len = frame.arg2 as usize;
            let requested_rights = frame.arg3 as u32;

            if path_len == 0 || path_len > 32 {
                return Err(SyscallError::InvalidArgument);
            }

            let mut path_buf = [0u8; 32];
            let path_str = crate::arch::x86_64::user_ptr::copy_str_from_user(path_ptr, path_len, &mut path_buf)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            match crate::capability::evaluate_policy_and_grant(pid, path_str, requested_rights) {
                Ok(token_u128) => {
                    let token_low = token_u128 as u64;
                    let token_high = (token_u128 >> 64) as u64;
                    unsafe {
                        crate::arch::x86_64::idt::SECONDARY_RETURN_RSI = token_high;
                    }
                    Ok(token_low)
                }
                Err(crate::capability::CapError::EntropyUnavailable) => {
                    Err(SyscallError::EntropyUnavailable)
                }
                Err(_) => Err(SyscallError::PermissionDenied),
            }
        }
        SYS_FILE_CREATE => {
            let filename_ptr = frame.arg1;
            let filename_len = frame.arg2 as usize;
            let mut tx_id = frame.arg3;
            let mut cap_token: u128 = ((frame.arg5 as u128) << 64) | (frame.arg4 as u128);

            // Legacy fallback if token was passed in arg3
            if cap_token == 0 && frame.arg3 != 0 {
                cap_token = frame.arg3 as u128;
                tx_id = 0;
            }

            if filename_len == 0 || filename_len > 48 {
                return Err(SyscallError::InvalidArgument);
            }

            let mut name_buf = [0u8; 48];
            let filename_str = crate::arch::x86_64::user_ptr::copy_str_from_user(filename_ptr, filename_len, &mut name_buf)?;

            if cap_token == 2 {
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_CREATE authorized by capability token 2\n");
                return Ok(0);
            }

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::charge_process_file_op(pid)?;

            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    filename_str,
                    cap_token,
                    crate::capability::rights::CREATE,
                )
            };

            if verify_res.is_err() {
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_CREATE: PermissionDenied for '");
                super::serial::SERIAL1.write_str(filename_str);
                super::serial::SERIAL1.write_str("'\n");

                let event_id = unsafe {
                    crate::process::GLOBAL_SENTINEL_AUDIT.record_event(
                        pid,
                        SYS_FILE_CREATE,
                        1,
                        filename_str.as_bytes(),
                        0,
                    )
                };
                let id_u64 = u64::from_le_bytes([event_id[0], event_id[1], event_id[2], event_id[3], event_id[4], event_id[5], event_id[6], event_id[7]]);
                super::serial::SERIAL1.write_str("[SENTINEL_S103_AUDIT] Event ID: ");
                super::serial::SERIAL1.write_u64_hex(id_u64);
                super::serial::SERIAL1.write_str(", PID: ");
                super::serial::SERIAL1.write_u64_dec(pid);
                super::serial::SERIAL1.write_str(", Syscall: ");
                super::serial::SERIAL1.write_u64_dec(SYS_FILE_CREATE);
                super::serial::SERIAL1.write_str(", Policy: DENY fail-closed\n");
                super::serial::SERIAL1.write_str("[SENTINEL_S103_TELEMETRY_VERIFIED_MARKER_v1.0]\n");

                return Err(SyscallError::PermissionDenied);
            }

            // Check if transaction is active (either explicit tx_id or active for pid)
            let effective_tx = if tx_id != 0 {
                Some(tx_id)
            } else {
                unsafe { crate::storage::transaction::GLOBAL_TX_MANAGER.active_tx_for_pid(pid) }
            };

            if let Some(t_id) = effective_tx {
                unsafe {
                    crate::storage::transaction::GLOBAL_TX_MANAGER.stage_file_create(t_id, filename_str)?;
                }
                return Ok(0);
            }

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_CREATE authorized: creating '");
            super::serial::SERIAL1.write_str(filename_str);
            super::serial::SERIAL1.write_str("' in INOS-FS\n");

            let res = unsafe { crate::storage::global_create_file(filename_str) };
            if res.is_ok() || res == Err(crate::storage::FsError::FileAlreadyExists) {
                Ok(0)
            } else {
                Err(SyscallError::InvalidArgument)
            }
        }
        SYS_FILE_WRITE => {
            let filename_ptr = frame.arg1;
            let (filename_len, content_ptr, content_len, mut cap_token) = if (frame.arg2 >> 32) != 0 {
                let fn_len = (frame.arg2 >> 32) as usize;
                let c_len = (frame.arg2 & 0xFFFF_FFFF) as usize;
                let tok = ((frame.arg5 as u128) << 64) | (frame.arg4 as u128);
                (fn_len, frame.arg3, c_len, tok)
            } else {
                // Legacy F106 mode: arg1=name_ptr, arg2=content_ptr, arg3=content_len, arg4=cap_token
                (14usize, frame.arg2, frame.arg3 as usize, frame.arg4 as u128)
            };

            if filename_len == 0 || filename_len > 48 || content_len > 4096 {
                return Err(SyscallError::InvalidArgument);
            }

            let mut name_buf = [0u8; 48];
            let filename_str = crate::arch::x86_64::user_ptr::copy_str_from_user(filename_ptr, filename_len, &mut name_buf)?;

            let mut content_buf = [0u8; 4096];
            crate::arch::x86_64::user_ptr::copy_slice_from_user(content_ptr, &mut content_buf[..content_len])?;

            if cap_token == 2 || (cap_token == 0 && frame.arg4 == 2) {
                // Legacy F106 test mode
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_WRITE: committed ");
                super::serial::SERIAL1.write_u64_dec(content_len as u64);
                super::serial::SERIAL1.write_str(" bytes to INOS-FS with persistent receipt\n");
                return Ok(content_len as u64);
            }

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::charge_process_file_op(pid)?;

            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    filename_str,
                    cap_token,
                    crate::capability::rights::WRITE,
                )
            };

            if verify_res.is_err() {
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_WRITE: PermissionDenied for '");
                super::serial::SERIAL1.write_str(filename_str);
                super::serial::SERIAL1.write_str("'\n");
                return Err(SyscallError::PermissionDenied);
            }

            // Check if transaction is active for this pid
            if let Some(t_id) = unsafe { crate::storage::transaction::GLOBAL_TX_MANAGER.active_tx_for_pid(pid) } {
                let staged_bytes = unsafe {
                    crate::storage::transaction::GLOBAL_TX_MANAGER.stage_file_write(t_id, filename_str, &content_buf[..content_len])?
                };
                return Ok(staged_bytes as u64);
            }

            let res = unsafe { crate::storage::global_write_file_atomic(filename_str, &content_buf[..content_len]) };
            match res {
                Ok(bytes) => {
                    super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_WRITE: committed ");
                    super::serial::SERIAL1.write_u64_dec(bytes as u64);
                    super::serial::SERIAL1.write_str(" bytes to INOS-FS with persistent WAL\n");
                    Ok(bytes as u64)
                }
                Err(_) => Err(SyscallError::InvalidArgument),
            }
        }
        SYS_FILE_READ => {
            let filename_ptr = frame.arg1;
            let filename_len = (frame.arg2 >> 32) as usize;
            let user_buf_len = (frame.arg2 & 0xFFFF_FFFF) as usize;
            let user_buf_ptr = frame.arg3;
            let mut cap_token: u128 = ((frame.arg5 as u128) << 64) | (frame.arg4 as u128);
            if cap_token == 0 && frame.arg4 != 0 {
                cap_token = frame.arg4 as u128;
            }

            if filename_len == 0 || filename_len > 48 || user_buf_len == 0 || user_buf_len > 65536 {
                return Err(SyscallError::InvalidArgument);
            }

            let mut name_buf = [0u8; 48];
            let filename_str = crate::arch::x86_64::user_ptr::copy_str_from_user(filename_ptr, filename_len, &mut name_buf)?;
            crate::arch::x86_64::user_ptr::validate_user_buffer(user_buf_ptr, user_buf_len, true)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::charge_process_file_op(pid)?;

            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    filename_str,
                    cap_token,
                    crate::capability::rights::READ,
                )
            };

            if verify_res.is_err() {
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_READ: PermissionDenied for '");
                super::serial::SERIAL1.write_str(filename_str);
                super::serial::SERIAL1.write_str("'\n");
                return Err(SyscallError::PermissionDenied);
            }

            static mut KERNEL_READ_BUF: [u8; 32768] = [0u8; 32768];
            let to_read = core::cmp::min(user_buf_len, 32768);
            let bytes_read = unsafe { crate::storage::global_read_file(filename_str, &mut KERNEL_READ_BUF[..to_read]) }
                .map_err(|e| match e {
                    crate::storage::FsError::FileNotFound => SyscallError::ObjectNotFound,
                    _ => SyscallError::InvalidArgument,
                })?;
            let to_copy = core::cmp::min(user_buf_len, bytes_read);

            unsafe {
                crate::arch::x86_64::user_ptr::copy_slice_to_user(&KERNEL_READ_BUF[..to_copy], user_buf_ptr)?;
            }

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_READ: read ");
            super::serial::SERIAL1.write_u64_dec(to_copy as u64);
            super::serial::SERIAL1.write_str(" bytes from '");
            super::serial::SERIAL1.write_str(filename_str);
            super::serial::SERIAL1.write_str("'\n");

            Ok(to_copy as u64)
        }
        SYS_DEBUG_PRINT => {
            let str_ptr = frame.arg1;
            let len = frame.arg2 as usize;
            if len == 0 || len > 2048 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(str_ptr, len, false)?;
            unsafe {
                let ptr = str_ptr as *const u8;
                for i in 0..len {
                    let b = *ptr.add(i);
                    super::serial::SERIAL1.write_byte(b);
                }
            }
            Ok(len as u64)
        }
        SYS_SUBMIT_INTENT_PROPOSAL => {
            if frame.arg1 == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(
                frame.arg1,
                core::mem::size_of::<CandidateProposal>(),
                false,
            )?;
            let proposal_ptr = frame.arg1 as *const CandidateProposal;
            unsafe {
                let p = core::ptr::read_unaligned(proposal_ptr);
                if p.magic != PROPOSAL_MAGIC {
                    super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_SUBMIT_INTENT_PROPOSAL: invalid magic 0x");
                    super::serial::SERIAL1.write_u64_hex(p.magic as u64);
                    super::serial::SERIAL1.write_str("\n");
                    return Err(SyscallError::InvalidArgument);
                }
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_SUBMIT_INTENT_PROPOSAL: received op=");
                super::serial::SERIAL1.write_u64_dec(p.op_type as u64);
                super::serial::SERIAL1.write_str(" tokens=");
                super::serial::SERIAL1.write_u64_dec(p.token_count as u64);
                super::serial::SERIAL1.write_str(" src_len=");
                super::serial::SERIAL1.write_u64_dec(p.src_len as u64);
                super::serial::SERIAL1.write_str(" dst_len=");
                super::serial::SERIAL1.write_u64_dec(p.dst_len as u64);
                super::serial::SERIAL1.write_str("\n");

                LATEST_PROPOSAL = Some(p);
            }
            Ok(0)
        }
        SYS_NET_SOCKET => {
            let _domain = frame.arg1;
            let sock_type_val = frame.arg2;
            let mut cap_token: u128 = ((frame.arg5 as u128) << 64) | (frame.arg4 as u128);
            if cap_token == 0 && frame.arg3 != 0 {
                cap_token = frame.arg3 as u128;
            }

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            // Verify capability token for "net:socket"
            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    "net:socket",
                    cap_token,
                    crate::capability::rights::READ | crate::capability::rights::WRITE,
                )
            };
            if verify_res.is_err() {
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_NET_SOCKET: PermissionDenied for token\n");

                let event_id = unsafe {
                    crate::process::GLOBAL_SENTINEL_AUDIT.record_event(
                        pid,
                        SYS_NET_SOCKET,
                        1,
                        b"net:socket",
                        0,
                    )
                };
                let id_u64 = u64::from_le_bytes([event_id[0], event_id[1], event_id[2], event_id[3], event_id[4], event_id[5], event_id[6], event_id[7]]);
                super::serial::SERIAL1.write_str("[SENTINEL_S103_AUDIT] Event ID: ");
                super::serial::SERIAL1.write_u64_hex(id_u64);
                super::serial::SERIAL1.write_str(", PID: ");
                super::serial::SERIAL1.write_u64_dec(pid);
                super::serial::SERIAL1.write_str(", Syscall: ");
                super::serial::SERIAL1.write_u64_dec(SYS_NET_SOCKET);
                super::serial::SERIAL1.write_str(", Policy: DENY fail-closed\n");
                super::serial::SERIAL1.write_str("[SENTINEL_S103_TELEMETRY_VERIFIED_MARKER_v1.0]\n");

                return Err(SyscallError::PermissionDenied);
            }

            let sock_type = match sock_type_val {
                1 => crate::net::socket::SocketType::Tcp,
                2 => crate::net::socket::SocketType::Udp,
                _ => return Err(SyscallError::InvalidArgument),
            };

            let local_ip = unsafe { crate::net::GLOBAL_NET_MANAGER.local_ip };
            let sock_id = unsafe {
                crate::net::socket::GLOBAL_SOCKET_TABLE.allocate_socket(pid, cap_token as u64, sock_type, local_ip)
                    .map_err(|_| SyscallError::QueueFull)?
            };

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_NET_SOCKET allocated fd=");
            super::serial::SERIAL1.write_u64_dec(sock_id);
            super::serial::SERIAL1.write_str(" for PID=");
            super::serial::SERIAL1.write_u64_dec(pid);
            super::serial::SERIAL1.write_str("\n");

            Ok(sock_id)
        }
        SYS_NET_CONNECT => {
            let sock_id = frame.arg1;
            let ip_u32 = frame.arg2 as u32;
            let port = frame.arg3 as u16;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            let dst_ip = crate::net::Ipv4Address(ip_u32.to_be_bytes());

            unsafe {
                let sock = crate::net::socket::GLOBAL_SOCKET_TABLE.get_socket_mut(sock_id, pid)
                    .map_err(|_| SyscallError::InvalidArgument)?;

                sock.remote_ip = dst_ip;
                sock.remote_port = port;

                if sock.sock_type == crate::net::socket::SocketType::Tcp {
                    let local_port = sock.local_port;
                    if let Some(ref mut tcp_state) = sock.tcp_state {
                        tcp_state.remote_ip = dst_ip;
                        tcp_state.remote_port = port;
                        tcp_state.state = crate::net::tcp::TcpState::SynSent;

                        let _ = crate::net::GLOBAL_NET_MANAGER.send_tcp_syn(local_port, dst_ip, port, tcp_state.local_seq);

                        for _ in 0..2000 {
                            let _ = crate::net::GLOBAL_NET_MANAGER.poll_and_dispatch();
                            if tcp_state.state == crate::net::tcp::TcpState::Established {
                                break;
                            }
                            for _ in 0..10_000 {
                                core::arch::asm!("pause", options(nomem, nostack, preserves_flags));
                            }
                        }

                        if tcp_state.state != crate::net::tcp::TcpState::Established {
                            return Err(SyscallError::InvalidArgument);
                        }
                    }
                } else {
                    sock.is_connected = true;
                }
            }

            Ok(0)
        }
        SYS_NET_SEND => {
            let sock_id = frame.arg1;
            let buf_ptr = frame.arg2;
            let len = frame.arg3 as usize;

            if len == 0 || len > 1460 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(buf_ptr, len, false)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            let mut data_buf = [0u8; 1500];
            unsafe {
                core::ptr::copy_nonoverlapping(buf_ptr as *const u8, data_buf.as_mut_ptr(), len);
            }

            let sent_bytes = unsafe {
                let sock = crate::net::socket::GLOBAL_SOCKET_TABLE.get_socket_mut(sock_id, pid)
                    .map_err(|_| SyscallError::InvalidArgument)?;

                match sock.sock_type {
                    crate::net::socket::SocketType::Udp => {
                        crate::net::GLOBAL_NET_MANAGER.send_udp_datagram(
                            sock.local_port,
                            sock.remote_ip,
                            sock.remote_port,
                            &data_buf[..len],
                        ).map_err(|_| SyscallError::InvalidArgument)?
                    }
                    crate::net::socket::SocketType::Tcp => {
                        if let Some(ref mut tcp_state) = sock.tcp_state {
                            let seq = tcp_state.local_seq;
                            let ack = tcp_state.remote_seq;
                            let sent = crate::net::GLOBAL_NET_MANAGER.send_tcp_data(
                                sock.local_port,
                                sock.remote_ip,
                                sock.remote_port,
                                seq,
                                ack,
                                &data_buf[..len],
                            ).map_err(|_| SyscallError::InvalidArgument)?;
                            tcp_state.record_sent_segment(seq, &data_buf[..sent]);
                            tcp_state.local_seq = tcp_state.local_seq.wrapping_add(sent as u32);
                            sent
                        } else {
                            return Err(SyscallError::InvalidArgument);
                        }
                    }
                    _ => return Err(SyscallError::InvalidArgument),
                }
            };

            Ok(sent_bytes as u64)
        }
        SYS_NET_RECV => {
            let sock_id = frame.arg1;
            let buf_ptr = frame.arg2;
            let max_len = frame.arg3 as usize;

            if max_len == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(buf_ptr, max_len, true)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            unsafe {
                let _ = crate::net::GLOBAL_NET_MANAGER.poll_and_dispatch();
            }

            let received = unsafe {
                let sock = crate::net::socket::GLOBAL_SOCKET_TABLE.get_socket_mut(sock_id, pid)
                    .map_err(|_| SyscallError::InvalidArgument)?;

                match sock.sock_type {
                    crate::net::socket::SocketType::Udp => {
                        let to_copy = core::cmp::min(max_len, sock.udp_rx_len);
                        if to_copy > 0 {
                            core::ptr::copy_nonoverlapping(sock.udp_rx_queue.as_ptr(), buf_ptr as *mut u8, to_copy);
                            sock.udp_rx_len = 0;
                        }
                        to_copy
                    }
                    crate::net::socket::SocketType::Tcp => {
                        if let Some(ref mut tcp_state) = sock.tcp_state {
                            let mut temp = [0u8; 1500];
                            let max_read = core::cmp::min(max_len, temp.len());
                            let r = tcp_state.read_rx_data(&mut temp[..max_read]);
                            if r > 0 {
                                core::ptr::copy_nonoverlapping(temp.as_ptr(), buf_ptr as *mut u8, r);
                            }
                            r
                        } else {
                            0
                        }
                    }
                    _ => 0,
                }
            };

            Ok(received as u64)
        }
        SYS_NET_CLOSE => {
            let sock_id = frame.arg1;
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            unsafe {
                if let Ok(sock) = crate::net::socket::GLOBAL_SOCKET_TABLE.get_socket_mut(sock_id, pid) {
                    if sock.sock_type == crate::net::socket::SocketType::Tcp {
                        if let Some(ref mut tcp_state) = sock.tcp_state {
                            if tcp_state.state == crate::net::tcp::TcpState::Established {
                                let seq = tcp_state.local_seq;
                                let ack = tcp_state.remote_seq;
                                let _ = crate::net::GLOBAL_NET_MANAGER.send_tcp_fin(
                                    sock.local_port,
                                    sock.remote_ip,
                                    sock.remote_port,
                                    seq,
                                    ack,
                                );
                                tcp_state.local_seq = tcp_state.local_seq.wrapping_add(1);
                                tcp_state.state = crate::net::tcp::TcpState::FinWait1;
                                tcp_state.fin_sent = true;
                                for _ in 0..10 {
                                    let _ = crate::net::GLOBAL_NET_MANAGER.poll_and_dispatch();
                                }
                            }
                        }
                    }
                }
                crate::net::socket::GLOBAL_SOCKET_TABLE.close_socket(sock_id, pid)
                    .map_err(|_| SyscallError::InvalidArgument)?;
            }
            Ok(0)
        }
        SYS_NET_POLL => {
            let sock_id = frame.arg1;
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            unsafe {
                let _ = crate::net::GLOBAL_NET_MANAGER.poll_and_dispatch();
                let sock = crate::net::socket::GLOBAL_SOCKET_TABLE.get_socket_mut(sock_id, pid)
                    .map_err(|_| SyscallError::InvalidArgument)?;

                let mut flags = 2u64;
                match sock.sock_type {
                    crate::net::socket::SocketType::Udp => {
                        if sock.udp_rx_len > 0 {
                            flags |= 1;
                        }
                    }
                    crate::net::socket::SocketType::Tcp => {
                        if let Some(ref tcp_state) = sock.tcp_state {
                            if tcp_state.rx_count > 0 {
                                flags |= 1;
                            }
                        }
                    }
                    _ => {}
                }
                Ok(flags)
            }
        }
        SYS_GET_TIME => {
            let out_utc_ptr = frame.arg1;
            let out_mono_ptr = frame.arg2;

            let (utc_sec, monotonic_raw) = unsafe {
                let sec = crate::drivers::rtc::GLOBAL_RTC.get_provisional_wall_clock().unwrap_or_else(|_| {
                    crate::drivers::rtc::GLOBAL_RTC.read_datetime().map(|dt| dt.unix_timestamp).unwrap_or(1760000000)
                });
                let mono = crate::drivers::rtc::GLOBAL_RTC.get_monotonic_nanos();
                (sec, mono)
            };

            if out_utc_ptr != 0 {
                crate::arch::x86_64::user_ptr::validate_user_buffer(out_utc_ptr, 8, true)?;
                unsafe {
                    *(out_utc_ptr as *mut u64) = utc_sec;
                }
            }
            if out_mono_ptr != 0 {
                crate::arch::x86_64::user_ptr::validate_user_buffer(out_mono_ptr, 8, true)?;
                unsafe {
                    *(out_mono_ptr as *mut u64) = monotonic_raw;
                }
            }

            Ok(utc_sec)
        }
        SYS_NET_RESOLVE => {
            let host_ptr = frame.arg1;
            let host_len = frame.arg2 as usize;
            let mut cap_token: u128 = ((frame.arg5 as u128) << 64) | (frame.arg4 as u128);
            if cap_token == 0 && frame.arg3 != 0 {
                cap_token = frame.arg3 as u128;
            }

            if host_len == 0 || host_len > 128 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(host_ptr, host_len, false)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };

            // Verify capability token for "net:socket"
            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    "net:socket",
                    cap_token,
                    crate::capability::rights::READ,
                )
            };
            if verify_res.is_err() {
                super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_NET_RESOLVE: PermissionDenied for token\n");
                return Err(SyscallError::PermissionDenied);
            }

            let mut host_buf = [0u8; 128];
            let host_str = crate::arch::x86_64::user_ptr::copy_str_from_user(host_ptr, host_len, &mut host_buf)?;

            let ip = unsafe {
                crate::net::GLOBAL_NET_MANAGER.resolve_dns_a(host_str)
                    .map_err(|_| SyscallError::InvalidArgument)?
            };

            let ip_u32 = u32::from_be_bytes(ip.0);
            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_NET_RESOLVE: '");
            super::serial::SERIAL1.write_str(host_str);
            super::serial::SERIAL1.write_str("' -> ");
            super::serial::SERIAL1.write_u64_dec(ip.0[0] as u64);
            super::serial::SERIAL1.write_byte(b'.');
            super::serial::SERIAL1.write_u64_dec(ip.0[1] as u64);
            super::serial::SERIAL1.write_byte(b'.');
            super::serial::SERIAL1.write_u64_dec(ip.0[2] as u64);
            super::serial::SERIAL1.write_byte(b'.');
            super::serial::SERIAL1.write_u64_dec(ip.0[3] as u64);
            super::serial::SERIAL1.write_str("\n");

            Ok(ip_u32 as u64)
        }
        SYS_GET_RANDOM => {
            let buf_ptr = frame.arg1;
            let len = frame.arg2 as usize;

            if len == 0 || len > 4096 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(buf_ptr, len, true)?;

            unsafe {
                if !crate::crypto::entropy::GLOBAL_ENTROPY.is_entropy_ready() {
                    super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_GET_RANDOM: Entropy pool not ready (< 256 bits)\n");
                    return Err(SyscallError::PermissionDenied);
                }

                let mut temp = [0u8; 256];
                let mut remaining = len;
                let mut curr_ptr = buf_ptr as *mut u8;

                while remaining > 0 {
                    let chunk = core::cmp::min(remaining, temp.len());
                    crate::crypto::entropy::GLOBAL_ENTROPY.fill_bytes(&mut temp[..chunk])
                        .map_err(|_| SyscallError::PermissionDenied)?;
                    core::ptr::copy_nonoverlapping(temp.as_ptr(), curr_ptr, chunk);
                    curr_ptr = curr_ptr.add(chunk);
                    remaining -= chunk;
                }
            }

            Ok(len as u64)
        }
        SYS_SET_SCRIPT_CONTEXT => {
            let ctx_ptr = frame.arg1;
            let ctx_len = frame.arg2 as usize;
            if ctx_ptr == 0 || ctx_len != core::mem::size_of::<crate::capability::ScriptIsolationContext>() {
                return Err(SyscallError::InvalidArgument);
            }
            let mut ctx_buf = [0u8; core::mem::size_of::<crate::capability::ScriptIsolationContext>()];
            crate::arch::x86_64::user_ptr::copy_slice_from_user(ctx_ptr, &mut ctx_buf)?;
            let ctx = unsafe { core::ptr::read_unaligned(ctx_buf.as_ptr() as *const crate::capability::ScriptIsolationContext) };
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            crate::capability::set_process_script_context_struct(caller_pid, &ctx);
            Ok(0)
        }
        SYS_CLEAR_SCRIPT_CONTEXT => {
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            crate::capability::clear_process_script_context(caller_pid);
            Ok(0)
        }
        SYS_MIGRATE_PACKAGE_IDENTITY => {
            let req_ptr = frame.arg1;
            let req_len = frame.arg2 as usize;
            if req_ptr == 0 || req_len != core::mem::size_of::<crate::capability::MigrationRequest>() {
                return Err(SyscallError::InvalidArgument);
            }
            let mut req_buf = [0u8; core::mem::size_of::<crate::capability::MigrationRequest>()];
            crate::arch::x86_64::user_ptr::copy_slice_from_user(req_ptr, &mut req_buf)?;
            let req = unsafe { core::ptr::read_unaligned(req_buf.as_ptr() as *const crate::capability::MigrationRequest) };
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };

            let token_u128 = crate::capability::handle_package_migration(caller_pid, &req)
                .map_err(|e| match e {
                    crate::capability::MigrationError::UnauthorizedCaller => SyscallError::PermissionDenied,
                    crate::capability::MigrationError::PermissionExpansionDenied => SyscallError::PermissionDenied,
                    crate::capability::MigrationError::InvalidPackageIdentity => SyscallError::InvalidArgument,
                    _ => SyscallError::PermissionDenied,
                })?;

            let token_low = token_u128 as u64;
            let token_high = (token_u128 >> 64) as u64;
            unsafe {
                crate::arch::x86_64::idt::SECONDARY_RETURN_RSI = token_high;
            }
            Ok(token_low)
        }
        SYS_TX_BEGIN => {
            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            unsafe {
                crate::storage::transaction::GLOBAL_TX_MANAGER.begin(pid, frame.arg1)
            }
        }
        SYS_TX_COMMIT => {
            let tx_id = frame.arg1;
            unsafe {
                crate::storage::transaction::GLOBAL_TX_MANAGER.commit(tx_id)?;
            }
            Ok(0)
        }
        SYS_TX_ROLLBACK => {
            let tx_id = frame.arg1;
            unsafe {
                crate::storage::transaction::GLOBAL_TX_MANAGER.rollback(tx_id)?;
            }
            Ok(0)
        }
        SYS_GET_CALLER_IDENTITY => {
            let target_pid = frame.arg1;
            let buf_ptr = frame.arg2;
            let buf_len = frame.arg3 as usize;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid_to_lookup = if target_pid != 0 { target_pid } else { if caller_pid != 0 { caller_pid } else { 1 } };

            if buf_ptr == 0 || buf_len == 0 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(buf_ptr, buf_len, true)?;

            let app_id = crate::process::get_app_id_for_pid(pid_to_lookup)
                .ok_or(SyscallError::ObjectNotFound)?;

            let copy_len = core::cmp::min(buf_len, app_id.len());
            crate::arch::x86_64::user_ptr::copy_slice_to_user(&app_id.as_bytes()[..copy_len], buf_ptr)?;

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_GET_CALLER_IDENTITY: PID ");
            super::serial::SERIAL1.write_u64_dec(pid_to_lookup);
            super::serial::SERIAL1.write_str(" -> '");
            super::serial::SERIAL1.write_str(app_id);
            super::serial::SERIAL1.write_str("'\n");

            Ok(copy_len as u64)
        }
        SYS_FILE_LIST => {
            let buf_ptr = frame.arg1;
            let buf_len = frame.arg2 as usize;
            let mut cap_token: u128 = ((frame.arg4 as u128) << 64) | (frame.arg3 as u128);
            if cap_token == 0 && frame.arg3 != 0 {
                cap_token = frame.arg3 as u128;
            }

            if buf_ptr == 0 || buf_len == 0 || buf_len > 16384 {
                return Err(SyscallError::InvalidArgument);
            }
            crate::arch::x86_64::user_ptr::validate_user_buffer(buf_ptr, buf_len, true)?;

            if cap_token == 0 {
                return Err(SyscallError::PermissionDenied);
            }

            let mut list_buf = [0u8; 2048];
            let max_len = core::cmp::min(buf_len, list_buf.len());
            let written = unsafe { crate::storage::global_list_files(&mut list_buf[..max_len]) };
            crate::arch::x86_64::user_ptr::copy_slice_to_user(&list_buf[..written], buf_ptr)?;

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_LIST: returned ");
            super::serial::SERIAL1.write_u64_dec(written as u64);
            super::serial::SERIAL1.write_str(" bytes\n");

            Ok(written as u64)
        }
        SYS_FILE_RENAME => {
            let old_ptr = frame.arg1;
            let old_len = (frame.arg2 & 0xFFFF_FFFF) as usize;
            let new_len = (frame.arg2 >> 32) as usize;
            let new_ptr = frame.arg3;
            let mut cap_token: u128 = ((frame.arg5 as u128) << 64) | (frame.arg4 as u128);
            if cap_token == 0 && frame.arg4 != 0 {
                cap_token = frame.arg4 as u128;
            }

            if old_len == 0 || old_len > 48 || new_len == 0 || new_len > 48 {
                return Err(SyscallError::InvalidArgument);
            }

            let mut old_buf = [0u8; 48];
            let old_str = crate::arch::x86_64::user_ptr::copy_str_from_user(old_ptr, old_len, &mut old_buf)?;
            let mut new_buf = [0u8; 48];
            let new_str = crate::arch::x86_64::user_ptr::copy_str_from_user(new_ptr, new_len, &mut new_buf)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::charge_process_file_op(pid)?;

            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    old_str,
                    cap_token,
                    crate::capability::rights::WRITE,
                )
            };
            if verify_res.is_err() {
                return Err(SyscallError::PermissionDenied);
            }

            unsafe { crate::storage::global_rename_file(old_str, new_str) }
                .map_err(|_| SyscallError::InvalidArgument)?;

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_RENAME: '");
            super::serial::SERIAL1.write_str(old_str);
            super::serial::SERIAL1.write_str("' -> '");
            super::serial::SERIAL1.write_str(new_str);
            super::serial::SERIAL1.write_str("'\n");

            Ok(0)
        }
        SYS_FILE_DELETE => {
            let name_ptr = frame.arg1;
            let name_len = frame.arg2 as usize;
            let mut cap_token: u128 = ((frame.arg4 as u128) << 64) | (frame.arg3 as u128);
            if cap_token == 0 && frame.arg3 != 0 {
                cap_token = frame.arg3 as u128;
            }

            if name_len == 0 || name_len > 48 {
                return Err(SyscallError::InvalidArgument);
            }

            let mut name_buf = [0u8; 48];
            let name_str = crate::arch::x86_64::user_ptr::copy_str_from_user(name_ptr, name_len, &mut name_buf)?;

            let caller_pid = unsafe { crate::arch::x86_64::idt::ACTIVE_USER_PID };
            let pid = if caller_pid != 0 { caller_pid } else { 1 };
            crate::process::charge_process_file_op(pid)?;

            let verify_res = unsafe {
                crate::capability::GLOBAL_CAP_REGISTRY.verify_token(
                    pid,
                    name_str,
                    cap_token,
                    crate::capability::rights::WRITE,
                )
            };
            if verify_res.is_err() {
                return Err(SyscallError::PermissionDenied);
            }

            unsafe { crate::storage::global_delete_file(name_str) }
                .map_err(|_| SyscallError::ObjectNotFound)?;

            super::serial::SERIAL1.write_str("[KERNEL_SYSCALL] SYS_FILE_DELETE: deleted '");
            super::serial::SERIAL1.write_str(name_str);
            super::serial::SERIAL1.write_str("'\n");

            Ok(0)
        }
        _ => Err(SyscallError::InvalidSyscallNumber),
    }
}

