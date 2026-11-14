// userspace/sdk/src/syscall.rs - Syscall numbers, error codes, and int 0x80 ABI wrappers

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
    Unknown = 255,
}

impl SyscallError {
    pub fn from_u64(val: u64) -> Self {
        match val {
            1 => SyscallError::InvalidSyscallNumber,
            2 => SyscallError::InvalidArgument,
            3 => SyscallError::PermissionDenied,
            4 => SyscallError::QueueFull,
            5 => SyscallError::BufferTooSmall,
            6 => SyscallError::ObjectNotFound,
            7 => SyscallError::CapabilityRevoked,
            8 => SyscallError::OutOfMemory,
            14 => SyscallError::QuotaExceeded,
            15 => SyscallError::EntropyUnavailable,
            16 => SyscallError::QueueEmpty,
            _ => SyscallError::Unknown,
        }
    }
}

pub mod rights {
    pub const READ: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const EXECUTE: u32 = 1 << 2;
    pub const DELEGATE: u32 = 1 << 3;
    pub const REVOKE: u32 = 1 << 4;
    pub const CREATE: u32 = 1 << 5;
    pub const IPC: u32 = 1 << 6;
    pub const SCHEDULE: u32 = 1 << 7;
    pub const MIGRATE_ADMIN: u32 = 1 << 8;
    pub const CLIPBOARD_READ: u32 = READ;
    pub const CLIPBOARD_WRITE: u32 = WRITE;
}

#[inline(always)]
pub unsafe fn raw_syscall(
    nr: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> Result<u64, SyscallError> {
    let ret: u64;
    let err: u64;
    core::arch::asm!(
        "int 0x80",
        inout("rax") nr => ret,
        in("rdi") arg1,
        in("rsi") arg2,
        inout("rdx") arg3 => err,
        in("r10") arg4,
        in("r8") arg5,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    if err != 0 {
        Err(SyscallError::from_u64(err))
    } else {
        Ok(ret)
    }
}

#[inline(always)]
pub unsafe fn raw_syscall_128(
    nr: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> Result<(u64, u64), SyscallError> {
    let ret_low: u64;
    let ret_high: u64;
    let err: u64;
    core::arch::asm!(
        "int 0x80",
        inout("rax") nr => ret_low,
        in("rdi") arg1,
        inout("rsi") arg2 => ret_high,
        inout("rdx") arg3 => err,
        in("r10") arg4,
        in("r8") arg5,
        out("rcx") _,
        out("r11") _,
        options(nostack)
    );
    if err != 0 {
        Err(SyscallError::from_u64(err))
    } else {
        Ok((ret_low, ret_high))
    }
}

pub const AUTOMATION_ABI_MAGIC: u16 = 0xAA01;

pub const OP_DISCOVER: u16 = 1;
pub const OP_OPEN: u16 = 2;
pub const OP_GET_CELL: u16 = 3;
pub const OP_SET_CELL: u16 = 4;
pub const OP_EVALUATE: u16 = 5;
pub const OP_SAVE: u16 = 6;

pub const STATUS_OK: u16 = 0;
pub const STATUS_NOT_FOUND: u16 = 1;
pub const STATUS_TYPE_MISMATCH: u16 = 2;
pub const STATUS_FORMULA_ERROR: u16 = 3;
pub const STATUS_PERMISSION_DENIED: u16 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomationIpcWire {
    pub magic: u16,          // 0xAA01
    pub version: u16,        // 1
    pub request_id: u32,
    pub op_code: u16,
    pub status_code: u16,
    pub _reserved: u32,
    pub cell: [u8; 8],
    pub value_num: i64,
    pub formula: [u8; 32],
    pub target_file: [u8; 32],
    pub error_msg: [u8; 32],
}

impl AutomationIpcWire {
    pub const fn empty() -> Self {
        Self {
            magic: AUTOMATION_ABI_MAGIC,
            version: 1,
            request_id: 0,
            op_code: 0,
            status_code: STATUS_OK,
            _reserved: 0,
            cell: [0u8; 8],
            value_num: 0,
            formula: [0u8; 32],
            target_file: [0u8; 32],
            error_msg: [0u8; 32],
        }
    }

    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        }
    }

    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < core::mem::size_of::<Self>() {
            return None;
        }
        let wire = unsafe { core::ptr::read_unaligned(buf.as_ptr() as *const Self) };
        if wire.magic != AUTOMATION_ABI_MAGIC || wire.version != 1 {
            return None;
        }
        Some(wire)
    }
}

pub const PROPOSAL_MAGIC: u32 = 0x494E5450; // "INTP"

pub const OP_TYPE_UNKNOWN: u32 = 0;
pub const OP_TYPE_COPY: u32 = 1;
pub const OP_TYPE_DELETE: u32 = 2;
pub const OP_TYPE_ARCHIVE: u32 = 3;

pub const PROVENANCE_NONE: u32 = 0;
pub const PROVENANCE_MODEL_GENERATED: u32 = 1;
pub const PROVENANCE_EXPLICIT_CONVENTION: u32 = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            magic: PROPOSAL_MAGIC,
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

    pub fn new_intent(op_type: u32, op_str: &str, src: &str, dst: &str, prompt_text: &str) -> Self {
        let mut p = Self::empty();
        p.op_type = op_type;
        p.src_provenance = PROVENANCE_MODEL_GENERATED;
        p.dst_provenance = PROVENANCE_MODEL_GENERATED;

        let op_b = op_str.as_bytes();
        let op_l = core::cmp::min(op_b.len(), 16);
        p.op_name[..op_l].copy_from_slice(&op_b[..op_l]);

        let src_b = src.as_bytes();
        let src_l = core::cmp::min(src_b.len(), 64);
        p.src_path[..src_l].copy_from_slice(&src_b[..src_l]);
        p.src_len = src_l as u32;

        let dst_b = dst.as_bytes();
        let dst_l = core::cmp::min(dst_b.len(), 64);
        p.dst_path[..dst_l].copy_from_slice(&dst_b[..dst_l]);
        p.dst_len = dst_l as u32;

        let txt_b = prompt_text.as_bytes();
        let txt_l = core::cmp::min(txt_b.len(), 128);
        p.raw_text[..txt_l].copy_from_slice(&txt_b[..txt_l]);
        p.raw_text_len = txt_l as u32;
        p
    }
}

/// Decoded 512-byte `INT_` On-Board Intelligence Spool Header (LBA 24576 ABI)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntSpoolHeader {
    pub version: u16,
    pub status: u16,
    pub filename: [u8; 32],
    pub filename_len: usize,
    pub payload_len: u32,
    pub sectors_needed: u16,
    pub provenance_hash: [u8; 32],
    pub prompt_hash: [u8; 32],
    pub is_ambiguous: bool,
}

impl IntSpoolHeader {
    pub fn parse_from_sector(sector: &[u8]) -> Option<Self> {
        if sector.len() < 113 {
            return None;
        }
        if &sector[0..4] != b"INT_" {
            return None;
        }
        let version = u16::from_le_bytes([sector[4], sector[5]]);
        let status = u16::from_le_bytes([sector[6], sector[7]]);
        let fn_len = core::cmp::min(32, u16::from_le_bytes([sector[8], sector[9]]) as usize);
        let mut filename = [0u8; 32];
        filename[..fn_len].copy_from_slice(&sector[10..(10 + fn_len)]);
        let payload_len = u32::from_le_bytes([sector[42], sector[43], sector[44], sector[45]]);
        let sectors_needed = u16::from_le_bytes([sector[46], sector[47]]);
        let mut provenance_hash = [0u8; 32];
        provenance_hash.copy_from_slice(&sector[48..80]);
        let mut prompt_hash = [0u8; 32];
        prompt_hash.copy_from_slice(&sector[80..112]);
        let is_ambiguous = sector[112] != 0;
        Some(Self {
            version,
            status,
            filename,
            filename_len: fn_len,
            payload_len,
            sectors_needed,
            provenance_hash,
            prompt_hash,
            is_ambiguous,
        })
    }
}

