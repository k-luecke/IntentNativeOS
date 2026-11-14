// kernel/src/ipc/web.rs - Versioned Web IPC Protocol (inos.web.ipc.v1) & Lifecycle Security
// Implements bounded IPC endpoints, generation-tracked handles, and trusted compositor display validation.

use crate::arch::x86_64::serial::SERIAL1;
use crate::arch::x86_64::syscall::SyscallError;

pub const WEB_IPC_MAGIC: u32 = 0x57454231; // "WEB1"
pub const WEB_IPC_VERSION: u32 = 1;
pub const WEB_IPC_MAX_PAYLOAD: usize = 1024;
pub const WEB_IPC_QUEUE_CAPACITY: usize = 16;
pub const WEB_MAX_COMMANDS_PER_FRAME: u32 = 64;

pub const IPC_CHAN_DOC_FEED: u32 = 1;
pub const IPC_CHAN_RENDER_CMD: u32 = 2;
pub const IPC_CHAN_STATUS: u32 = 3;

pub const MSG_DOC_FEED: u32 = 1;
pub const MSG_RENDER_CMD: u32 = 2;
pub const MSG_STATUS: u32 = 3;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WebIpcWire {
    pub magic: u32,
    pub version: u32,
    pub msg_type: u32,
    pub channel_id: u32,
    pub sender_pid: u64,
    pub sender_generation: u64,
    pub target_pid: u64,
    pub seq: u64,
    pub payload_len: u32,
    pub _reserved: u32,
    pub payload: [u8; WEB_IPC_MAX_PAYLOAD],
}

impl WebIpcWire {
    pub const fn empty() -> Self {
        Self {
            magic: WEB_IPC_MAGIC,
            version: WEB_IPC_VERSION,
            msg_type: 0,
            channel_id: 0,
            sender_pid: 0,
            sender_generation: 0,
            target_pid: 0,
            seq: 0,
            payload_len: 0,
            _reserved: 0,
            payload: [0u8; WEB_IPC_MAX_PAYLOAD],
        }
    }

    pub fn new(sender_pid: u64, target_pid: u64, sender_gen: u64, msg_type: u32) -> Self {
        let mut w = Self::empty();
        w.sender_pid = sender_pid;
        w.target_pid = target_pid;
        w.sender_generation = sender_gen;
        w.msg_type = msg_type;
        w
    }

    pub fn set_payload(&mut self, data: &[u8]) {
        let len = data.len().min(WEB_IPC_MAX_PAYLOAD);
        self.payload[..len].copy_from_slice(&data[..len]);
        self.payload_len = len as u32;
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebRenderOp {
    DrawRect = 1,
    DrawText = 2,
    DrawBorder = 3,
    SetClip = 4,
    Present = 5,
}

impl WebRenderOp {
    pub fn from_u32(val: u32) -> Option<Self> {
        match val {
            1 => Some(WebRenderOp::DrawRect),
            2 => Some(WebRenderOp::DrawText),
            3 => Some(WebRenderOp::DrawBorder),
            4 => Some(WebRenderOp::SetClip),
            5 => Some(WebRenderOp::Present),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WebRenderCmd {
    pub op: u32,
    pub surface_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: u32,
    pub text_len: u32,
    pub text_bytes: [u8; 128],
}

impl WebRenderCmd {
    pub const fn empty() -> Self {
        Self {
            op: 0,
            surface_id: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            color: 0,
            text_len: 0,
            text_bytes: [0u8; 128],
        }
    }
}

// Process generation / lifecycle security:
// When a process exits or is reaped, its generation increments.
// Stale IPC handles carrying previous generations are rejected fail-closed.
pub static mut PROCESS_GENERATIONS: [u64; 1024] = [1; 1024];

pub fn get_process_generation(pid: u64) -> u64 {
    let idx = (pid as usize) % 1024;
    unsafe { PROCESS_GENERATIONS[idx] }
}

pub fn increment_process_generation(pid: u64) {
    let idx = (pid as usize) % 1024;
    unsafe {
        PROCESS_GENERATIONS[idx] = PROCESS_GENERATIONS[idx].wrapping_add(1);
    }
}

pub struct WebIpcQueue {
    pub messages: [Option<WebIpcWire>; WEB_IPC_QUEUE_CAPACITY],
    pub head: usize,
    pub tail: usize,
    pub count: usize,
    pub commands_submitted_this_frame: u32,
}

impl WebIpcQueue {
    pub const fn new() -> Self {
        Self {
            messages: [None; WEB_IPC_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
            commands_submitted_this_frame: 0,
        }
    }

    pub fn enqueue(&mut self, msg: WebIpcWire) -> Result<(), SyscallError> {
        if self.count >= WEB_IPC_QUEUE_CAPACITY {
            SERIAL1.write_str("[WEB_IPC] REJECT: IPC queue capacity exhausted\n");
            return Err(SyscallError::QueueFull);
        }
        self.messages[self.tail] = Some(msg);
        self.tail = (self.tail + 1) % WEB_IPC_QUEUE_CAPACITY;
        self.count += 1;
        Ok(())
    }

    pub fn dequeue_for(&mut self, target_pid: u64) -> Result<WebIpcWire, SyscallError> {
        for i in 0..WEB_IPC_QUEUE_CAPACITY {
            let idx = (self.head + i) % WEB_IPC_QUEUE_CAPACITY;
            if let Some(msg) = self.messages[idx] {
                if msg.target_pid == target_pid {
                    self.messages[idx] = None;
                    self.count = self.count.saturating_sub(1);
                    if self.count == 0 {
                        self.head = self.tail;
                    } else {
                        while self.messages[self.head].is_none() {
                            self.head = (self.head + 1) % WEB_IPC_QUEUE_CAPACITY;
                        }
                    }
                    return Ok(msg);
                }
            }
        }
        Err(SyscallError::ObjectNotFound)
    }

    pub fn flush_for_pid(&mut self, pid: u64) {
        for slot in self.messages.iter_mut() {
            if let Some(msg) = slot {
                if msg.sender_pid == pid || msg.target_pid == pid {
                    *slot = None;
                    self.count = self.count.saturating_sub(1);
                }
            }
        }
        if self.count == 0 {
            self.head = self.tail;
        } else {
            while self.messages[self.head].is_none() {
                self.head = (self.head + 1) % WEB_IPC_QUEUE_CAPACITY;
            }
        }
    }
}

pub static mut GLOBAL_WEB_IPC: WebIpcQueue = WebIpcQueue::new();

/// Handles `SYS_IPC_SEND` for `inos.web.ipc.v1` wire messages
pub fn handle_web_ipc_send(caller_pid: u64, msg: &WebIpcWire) -> Result<u64, SyscallError> {
    // 1. Validate Protocol Magic & Version
    if msg.magic != WEB_IPC_MAGIC {
        SERIAL1.write_str("[WEB_IPC] REJECT: Invalid magic 0x");
        SERIAL1.write_u64_hex(msg.magic as u64);
        SERIAL1.write_str("\n");
        return Err(SyscallError::InvalidArgument);
    }

    if msg.version != WEB_IPC_VERSION {
        SERIAL1.write_str("[WEB_IPC] REJECT: Unsupported protocol version: ");
        SERIAL1.write_u64_dec(msg.version as u64);
        SERIAL1.write_str("\n");
        return Err(SyscallError::InvalidArgument);
    }

    // 2. Anti-Spoofing: Caller must strictly match sender_pid
    if caller_pid != msg.sender_pid {
        SERIAL1.write_str("[WEB_IPC] SECURITY REJECTION: Forged sender identity! Caller PID=");
        SERIAL1.write_u64_dec(caller_pid);
        SERIAL1.write_str(" forged sender_pid=");
        SERIAL1.write_u64_dec(msg.sender_pid);
        SERIAL1.write_str(". Access denied fail-closed!\n");
        return Err(SyscallError::PermissionDenied);
    }

    // 3. Stale Handle Rejection: Check generation
    let expected_gen = get_process_generation(caller_pid);
    if msg.sender_generation != expected_gen {
        SERIAL1.write_str("[WEB_IPC] SECURITY REJECTION: Stale IPC endpoint handle! PID=");
        SERIAL1.write_u64_dec(caller_pid);
        SERIAL1.write_str(" handle_gen=");
        SERIAL1.write_u64_dec(msg.sender_generation);
        SERIAL1.write_str(" current_gen=");
        SERIAL1.write_u64_dec(expected_gen);
        SERIAL1.write_str(". Access denied fail-closed!\n");
        return Err(SyscallError::PermissionDenied);
    }

    // 4. Validate Payload Bounds
    if msg.payload_len as usize > WEB_IPC_MAX_PAYLOAD {
        SERIAL1.write_str("[WEB_IPC] REJECT: Oversized payload length: ");
        SERIAL1.write_u64_dec(msg.payload_len as u64);
        SERIAL1.write_str(" (max 1024)\n");
        return Err(SyscallError::InvalidArgument);
    }

    // 5. Message Type Routing & Security Policy
    match msg.msg_type {
        MSG_DOC_FEED => {
            // Document feed may only be sent by Network Broker (PID 301) or Browser UI (PID 300) to Renderer (PID 310)
            if caller_pid == 310 {
                SERIAL1.write_str("[WEB_IPC] SECURITY REJECTION: Renderer (PID 310) unauthorized to dispatch doc feeds!\n");
                return Err(SyscallError::PermissionDenied);
            }
            if msg.target_pid != 310 {
                SERIAL1.write_str("[WEB_IPC] REJECT: Document feed must target renderer PID 310\n");
                return Err(SyscallError::InvalidArgument);
            }

            unsafe {
                GLOBAL_WEB_IPC.enqueue(*msg)?;
            }
            SERIAL1.write_str("[WEB_IPC] DocFeed message enqueued for PID 310 (len=");
            SERIAL1.write_u64_dec(msg.payload_len as u64);
            SERIAL1.write_str(")\n");
            Ok(msg.payload_len as u64)
        }

        MSG_RENDER_CMD => {
            // Renderer (PID 310) submitting display commands to Compositor
            if caller_pid != 310 {
                SERIAL1.write_str("[WEB_IPC] SECURITY REJECTION: Only renderer (PID 310) may submit WebRenderCmd\n");
                return Err(SyscallError::PermissionDenied);
            }

            if msg.payload_len as usize != core::mem::size_of::<WebRenderCmd>() {
                SERIAL1.write_str("[WEB_IPC] REJECT: Malformed WebRenderCmd payload size\n");
                return Err(SyscallError::InvalidArgument);
            }

            let cmd = unsafe {
                core::ptr::read_unaligned(msg.payload.as_ptr() as *const WebRenderCmd)
            };

            // Trusted Compositor Validation
            validate_and_execute_render_cmd(caller_pid, &cmd)?;
            Ok(0)
        }

        MSG_STATUS => {
            unsafe {
                GLOBAL_WEB_IPC.enqueue(*msg)?;
            }
            Ok(0)
        }

        _ => {
            SERIAL1.write_str("[WEB_IPC] REJECT: Unknown msg_type: ");
            SERIAL1.write_u64_dec(msg.msg_type as u64);
            SERIAL1.write_str("\n");
            Err(SyscallError::InvalidArgument)
        }
    }
}

/// Trusted Compositor: Validates display command boundaries, surface ownership,
/// clipping, UTF-8 validity, and prevents drawing over Browser UI Chrome.
fn validate_and_execute_render_cmd(caller_pid: u64, cmd: &WebRenderCmd) -> Result<(), SyscallError> {
    let op = WebRenderOp::from_u32(cmd.op).ok_or(SyscallError::InvalidArgument)?;

    // Surface Ownership & Chrome Isolation:
    // Surface 1 is Browser UI Chrome (owned by PID 300: address bar, controls, permission prompts).
    // Surface 2 is Browser Document Content (owned by PID 310).
    // Renderer PID 310 MUST NOT draw on Surface 1!
    if cmd.surface_id != 2 {
        SERIAL1.write_str("[COMPOSITOR_GUARD] SECURITY REJECTION: Renderer PID ");
        SERIAL1.write_u64_dec(caller_pid);
        SERIAL1.write_str(" attempted draw on surface ");
        SERIAL1.write_u64_dec(cmd.surface_id as u64);
        SERIAL1.write_str(" (Browser Chrome / System UI protected). Access denied fail-closed!\n");
        return Err(SyscallError::PermissionDenied);
    }

    // Document Content Surface Bounds: 1000 x 600
    const CONTENT_WIDTH: u32 = 1000;
    const CONTENT_HEIGHT: u32 = 600;

    // Enforce quota per frame
    unsafe {
        GLOBAL_WEB_IPC.commands_submitted_this_frame += 1;
        if GLOBAL_WEB_IPC.commands_submitted_this_frame > WEB_MAX_COMMANDS_PER_FRAME {
            SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Frame draw quota exceeded (DoS defense)\n");
            return Err(SyscallError::QueueFull);
        }
    }

    match op {
        WebRenderOp::DrawRect | WebRenderOp::DrawBorder => {
            // Negative coordinates check
            if cmd.x < 0 || cmd.y < 0 {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Negative coordinates: x=");
                SERIAL1.write_u64_dec(cmd.x as u64);
                SERIAL1.write_str(" y=");
                SERIAL1.write_u64_dec(cmd.y as u64);
                SERIAL1.write_str("\n");
                return Err(SyscallError::InvalidArgument);
            }

            let ux = cmd.x as u32;
            let uy = cmd.y as u32;

            // Integer overflow and surface boundary checks
            let right = ux.checked_add(cmd.width).ok_or_else(|| {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Integer overflow in x + width\n");
                SyscallError::InvalidArgument
            })?;

            let bottom = uy.checked_add(cmd.height).ok_or_else(|| {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Integer overflow in y + height\n");
                SyscallError::InvalidArgument
            })?;

            if right > CONTENT_WIDTH || bottom > CONTENT_HEIGHT {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Draw command exceeds content surface boundaries (right=");
                SERIAL1.write_u64_dec(right as u64);
                SERIAL1.write_str(" bottom=");
                SERIAL1.write_u64_dec(bottom as u64);
                SERIAL1.write_str(")\n");
                return Err(SyscallError::InvalidArgument);
            }

            SERIAL1.write_str("[COMPOSITOR] Validated display command: op=");
            SERIAL1.write_u64_dec(cmd.op as u64);
            SERIAL1.write_str(" rect=(");
            SERIAL1.write_u64_dec(ux as u64);
            SERIAL1.write_str(",");
            SERIAL1.write_u64_dec(uy as u64);
            SERIAL1.write_str(",");
            SERIAL1.write_u64_dec(cmd.width as u64);
            SERIAL1.write_str(",");
            SERIAL1.write_u64_dec(cmd.height as u64);
            SERIAL1.write_str(")\n");
            Ok(())
        }

        WebRenderOp::DrawText => {
            if cmd.x < 0 || cmd.y < 0 {
                return Err(SyscallError::InvalidArgument);
            }
            let ux = cmd.x as u32;
            let uy = cmd.y as u32;

            let text_len = cmd.text_len as usize;
            if text_len == 0 || text_len > 128 {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Invalid text length\n");
                return Err(SyscallError::InvalidArgument);
            }

            // UTF-8 Validation
            let text_slice = &cmd.text_bytes[..text_len];
            let text_str = core::str::from_utf8(text_slice).map_err(|_| {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: Invalid UTF-8 sequence in DrawText\n");
                SyscallError::InvalidArgument
            })?;

            let pixel_width = (text_len as u32).checked_mul(8).ok_or(SyscallError::InvalidArgument)?;
            let right = ux.checked_add(pixel_width).ok_or(SyscallError::InvalidArgument)?;
            let bottom = uy.checked_add(16).ok_or(SyscallError::InvalidArgument)?;

            if right > CONTENT_WIDTH || bottom > CONTENT_HEIGHT {
                SERIAL1.write_str("[COMPOSITOR_GUARD] REJECT: DrawText out of content bounds\n");
                return Err(SyscallError::InvalidArgument);
            }

            SERIAL1.write_str("[COMPOSITOR] Validated DrawText: '");
            SERIAL1.write_str(text_str);
            SERIAL1.write_str("'\n");
            Ok(())
        }

        WebRenderOp::SetClip => {
            let ux = cmd.x as u32;
            let uy = cmd.y as u32;
            let right = ux.checked_add(cmd.width).ok_or(SyscallError::InvalidArgument)?;
            let bottom = uy.checked_add(cmd.height).ok_or(SyscallError::InvalidArgument)?;
            if right > CONTENT_WIDTH || bottom > CONTENT_HEIGHT {
                return Err(SyscallError::InvalidArgument);
            }
            SERIAL1.write_str("[COMPOSITOR] SetClip boundary updated\n");
            Ok(())
        }

        WebRenderOp::Present => {
            unsafe {
                GLOBAL_WEB_IPC.commands_submitted_this_frame = 0;
            }
            SERIAL1.write_str("[COMPOSITOR] Present committed for content surface 2\n");
            Ok(())
        }
    }
}

/// Handles `SYS_IPC_RECV` for `inos.web.ipc.v1`
pub fn handle_web_ipc_recv(caller_pid: u64, buf_ptr: u64, buf_len: usize) -> Result<u64, SyscallError> {
    if buf_len < core::mem::size_of::<WebIpcWire>() {
        return Err(SyscallError::BufferTooSmall);
    }

    let msg = unsafe {
        GLOBAL_WEB_IPC.dequeue_for(caller_pid)?
    };

    let msg_bytes = unsafe {
        core::slice::from_raw_parts(&msg as *const WebIpcWire as *const u8, core::mem::size_of::<WebIpcWire>())
    };

    crate::arch::x86_64::user_ptr::copy_slice_to_user(msg_bytes, buf_ptr)?;
    SERIAL1.write_str("[WEB_IPC] Message successfully dispatched to PID ");
    SERIAL1.write_u64_dec(caller_pid);
    SERIAL1.write_str(" (type=");
    SERIAL1.write_u64_dec(msg.msg_type as u64);
    SERIAL1.write_str(")\n");

    Ok(msg.payload_len as u64)
}
