// userspace/apps/web_render/src/main.rs - Isolated Native Document Renderer & HTML Parser (Milestones WEB-02A & WEB-02B)
// Runs at hardware CPL=3 under dedicated PML4 with zero ambient capabilities.
// Communicates strictly via versioned inos.web.ipc.v1 channels with trusted compositor validation.
// Implements streaming HTML tokenization, WHATWG error recovery, inert script parsing, and deterministic limits.

#![no_std]
#![no_main]

extern crate alloc;

mod limits;
mod dom;
mod decoder;
mod tokenizer;
mod tree_builder;

use alloc::vec::Vec;
use inos_app_sdk::{debug_print, exit, get_cpl, get_pid, BoundedHeapAllocator};
use inos_app_sdk::syscall::{raw_syscall, SyscallError, SYS_CAP_REQUEST, SYS_NET_SOCKET, SYS_FILE_CREATE, SYS_IPC_SEND, SYS_IPC_RECV};

use limits::{LimitKind, ParseError, WorkBudgetTracker, MAX_DOM_NODES, MAX_INPUT_BYTES, MAX_WORK_OPERATIONS};
use dom::{DomArena, NodeData};
use decoder::Utf8StreamDecoder;
use tokenizer::HtmlTokenizer;
use tree_builder::HtmlTreeBuilder;

#[global_allocator]
static ALLOCATOR: BoundedHeapAllocator<{ 512 * 1024 }> = BoundedHeapAllocator::new();

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    debug_print("[WEB_RENDER] Panic occurred in Ring 3 renderer!\n");
    exit(101);
}

#[no_mangle]
pub extern "C" fn __stack_chk_fail() -> ! {
    debug_print("[WEB_RENDER] ERROR: Stack canary failure detected!\n");
    exit(134);
}

pub const WEB_IPC_MAGIC: u32 = 0x57454231; // "WEB1"
pub const WEB_IPC_VERSION: u32 = 1;
pub const WEB_IPC_MAX_PAYLOAD: usize = 1024;

pub const IPC_CHAN_DOC_FEED: u32 = 1;
pub const IPC_CHAN_RENDER_CMD: u32 = 2;

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

pub struct HtmlParser {
    pub decoder: Utf8StreamDecoder,
    pub tokenizer: HtmlTokenizer,
    pub builder: HtmlTreeBuilder,
    pub arena: DomArena,
    pub budget: WorkBudgetTracker,
}

impl HtmlParser {
    pub fn new() -> Result<Self, ParseError> {
        Ok(Self {
            decoder: Utf8StreamDecoder::new(MAX_INPUT_BYTES),
            tokenizer: HtmlTokenizer::new(),
            builder: HtmlTreeBuilder::new(),
            arena: DomArena::new(MAX_DOM_NODES)?,
            budget: WorkBudgetTracker::new(MAX_WORK_OPERATIONS),
        })
    }

    pub fn parse_chunk(&mut self, chunk: &[u8]) -> Result<(), ParseError> {
        let mut chars = Vec::new();
        self.decoder.feed_chunk(chunk, &mut chars, &mut self.budget)?;

        let mut tokens = Vec::new();
        self.tokenizer.feed_chars(&chars, &mut tokens, &mut self.budget)?;

        for token in tokens {
            self.builder.process_token(token, &mut self.arena, &mut self.budget)?;
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), ParseError> {
        let mut chars = Vec::new();
        self.decoder.finish(&mut chars, &mut self.budget)?;

        let mut tokens = Vec::new();
        if !chars.is_empty() {
            self.tokenizer.feed_chars(&chars, &mut tokens, &mut self.budget)?;
        }
        self.tokenizer.finish(&mut tokens, &mut self.budget)?;

        for token in tokens {
            self.builder.process_token(token, &mut self.arena, &mut self.budget)?;
        }
        Ok(())
    }
}

pub fn parse_document(html: &[u8]) -> Result<DomArena, ParseError> {
    let mut parser = HtmlParser::new()?;
    parser.parse_chunk(html)?;
    parser.finish()?;
    Ok(parser.arena)
}

pub fn parse_document_chunked(html: &[u8], chunk_size: usize) -> Result<DomArena, ParseError> {
    let mut parser = HtmlParser::new()?;
    for chunk in html.chunks(chunk_size) {
        parser.parse_chunk(chunk)?;
    }
    parser.finish()?;
    Ok(parser.arena)
}

core::arch::global_asm!(
    ".section .text._start, \"ax\"",
    ".global _start",
    "_start:",
    "and rsp, -16",
    "call real_start",
    "ud2"
);

#[no_mangle]
pub extern "C" fn real_start() -> ! {
    debug_print("[WEB_RENDER] Native Ring 3 Document Renderer started (Milestones WEB-02A & WEB-02B).\n");

    // =========================================================================
    // 1. Hardware CPL=3 Privilege Verification
    // =========================================================================
    let cpl = get_cpl();
    if cpl == 3 {
        debug_print("[WEB_RENDER] Hardware CPL=3 isolation confirmed.\n");
    } else {
        debug_print("[WEB_RENDER] ERROR: Not running at CPL=3!\n");
        exit(1);
    }

    let pid = get_pid();
    if pid == 310 {
        debug_print("[WEB_RENDER] Registered Process ID 310 confirmed (dedicated sandbox).\n");
    } else {
        debug_print("[WEB_RENDER] ERROR: Unexpected PID!\n");
        exit(2);
    }

    // =========================================================================
    // 2. Ambient Authority Rejection Gate (Zero Ambient Capabilities)
    // =========================================================================
    debug_print("[WEB_RENDER] Negative Test 1: Requesting network socket capability...\n");
    let net_path = b"net:socket\0";
    let cap_res = unsafe {
        raw_syscall(SYS_CAP_REQUEST, net_path.as_ptr() as u64, 10, 1, 0, 0)
    };
    match cap_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Network socket capability request denied fail-closed (PermissionDenied).\n");
            debug_print("[WEB_02A_AMBIENT_NET_DENIED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Renderer granted network capability!\n");
            exit(3);
        }
    }

    debug_print("[WEB_RENDER] Negative Test 2: Attempting direct socket creation...\n");
    let sock_res = unsafe {
        raw_syscall(SYS_NET_SOCKET, 99999, 0, 0, 0, 0)
    };
    match sock_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Direct socket syscall denied fail-closed.\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Socket created by renderer!\n");
            exit(4);
        }
    }

    debug_print("[WEB_RENDER] Negative Test 3: Requesting filesystem write capability...\n");
    let fs_path = b"test_output.txt\0";
    let fs_cap_res = unsafe {
        raw_syscall(SYS_CAP_REQUEST, fs_path.as_ptr() as u64, 15, 2, 0, 0)
    };
    match fs_cap_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Filesystem write capability denied fail-closed (PermissionDenied).\n");
            debug_print("[WEB_02A_AMBIENT_FS_DENIED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Renderer granted filesystem capability!\n");
            exit(5);
        }
    }

    debug_print("[WEB_RENDER] Negative Test 4: Attempting direct file creation syscall...\n");
    let file_create_res = unsafe {
        raw_syscall(SYS_FILE_CREATE, fs_path.as_ptr() as u64, 15, 0, 0, 0)
    };
    match file_create_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Direct file create denied fail-closed.\n");
            debug_print("[WEB_02A_AMBIENT_HW_DENIED_MARKER_v1.0]\n");
            debug_print("[WEB_02A_AMBIENT_AUTHORITY_DENIED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: File created by renderer!\n");
            exit(6);
        }
    }

    // =========================================================================
    // 3. Authorized Bounded IPC Reception (Document Feed from Broker)
    // =========================================================================
    debug_print("[WEB_RENDER] Receiving document stream via IPC_CHAN_DOC_FEED...\n");
    let mut wire = WebIpcWire::empty();
    let mut cmd: WebRenderCmd;

    let recv_res = unsafe {
        raw_syscall(
            SYS_IPC_RECV,
            IPC_CHAN_DOC_FEED as u64,
            &mut wire as *mut WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };

    match recv_res {
        Ok(bytes_received) => {
            if wire.magic == WEB_IPC_MAGIC && wire.version == WEB_IPC_VERSION && wire.msg_type == MSG_DOC_FEED {
                debug_print("[WEB_RENDER] Received valid inos.web.ipc.v1 document chunk: len=");
                let _ = bytes_received;
                debug_print(" bytes from Broker (PID 301).\n");
                debug_print("[WEB_02A_IPC_DOC_RECV_VERIFIED_MARKER_v1.0]\n");
            } else {
                debug_print("[WEB_RENDER] ERROR: Corrupted IPC message headers!\n");
                exit(7);
            }
        }
        Err(e) => {
            debug_print("[WEB_RENDER] ERROR: Failed to receive IPC document feed!\n");
            let _ = e;
            exit(8);
        }
    }

    // =========================================================================
    // 4. Authorized Compositor Display Command Submission
    // =========================================================================
    debug_print("[WEB_RENDER] Submitting validated display commands for content surface 2...\n");

    // Command 1: DrawRect background
    cmd = WebRenderCmd::empty();
    cmd.op = WebRenderOp::DrawRect as u32;
    cmd.surface_id = 2; // Content surface
    cmd.x = 20;
    cmd.y = 20;
    cmd.width = 400;
    cmd.height = 100;
    cmd.color = 0x00F0_F4F8;

    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.channel_id = IPC_CHAN_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.target_pid = 0; // Compositor
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &cmd as *const WebRenderCmd as *const u8,
            wire.payload.as_mut_ptr(),
            core::mem::size_of::<WebRenderCmd>(),
        );
    }

    let send_res1 = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    if send_res1.is_err() {
        debug_print("[WEB_RENDER] ERROR: DrawRect submission failed!\n");
        exit(9);
    }

    // Command 2: DrawText content
    cmd = WebRenderCmd::empty();
    cmd.op = WebRenderOp::DrawText as u32;
    cmd.surface_id = 2; // Content surface
    cmd.x = 40;
    cmd.y = 40;
    let sample_text = b"INOS Native Document Engine";
    cmd.text_len = sample_text.len() as u32;
    cmd.text_bytes[..sample_text.len()].copy_from_slice(sample_text);

    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.channel_id = IPC_CHAN_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.target_pid = 0;
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &cmd as *const WebRenderCmd as *const u8,
            wire.payload.as_mut_ptr(),
            core::mem::size_of::<WebRenderCmd>(),
        );
    }

    let send_res2 = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    if send_res2.is_err() {
        debug_print("[WEB_RENDER] ERROR: DrawText submission failed!\n");
        exit(10);
    }

    // Command 3: Present
    cmd = WebRenderCmd::empty();
    cmd.op = WebRenderOp::Present as u32;
    cmd.surface_id = 2;

    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.channel_id = IPC_CHAN_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.target_pid = 0;
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &cmd as *const WebRenderCmd as *const u8,
            wire.payload.as_mut_ptr(),
            core::mem::size_of::<WebRenderCmd>(),
        );
    }

    let send_res3 = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    if send_res3.is_err() {
        debug_print("[WEB_RENDER] ERROR: Present submission failed!\n");
        exit(11);
    }

    debug_print("[WEB_RENDER] PASS: Display commands validated and accepted by Compositor.\n");
    debug_print("[WEB_02A_IPC_DISPLAY_SUBMIT_VERIFIED_MARKER_v1.0]\n");

    // =========================================================================
    // 5. Negative Security Matrix (IPC & Compositor Boundaries)
    // =========================================================================

    // Negative Test 5.1: Forged Sender Identity
    debug_print("[WEB_RENDER] Negative Test 5.1: Forging sender identity (impersonating PID 300 UI)...\n");
    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.sender_pid = 300; // FORGED! Caller is 310
    wire.sender_generation = 1;
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;

    let forged_res = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    match forged_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Forged sender identity rejected (PermissionDenied).\n");
            debug_print("[WEB_02A_FORGED_IPC_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Forged sender accepted!\n");
            exit(12);
        }
    }

    // Negative Test 5.2: Stale Endpoint Handle / Generation Mismatch
    debug_print("[WEB_RENDER] Negative Test 5.2: Submitting stale handle with invalid generation...\n");
    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 9999; // STALE! Current generation is 1
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;

    let stale_res = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    match stale_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Stale handle rejected fail-closed (PermissionDenied).\n");
            debug_print("[WEB_02A_STALE_HANDLE_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Stale generation handle accepted!\n");
            exit(13);
        }
    }

    // Negative Test 5.3: Oversized Message Payload
    debug_print("[WEB_RENDER] Negative Test 5.3: Submitting oversized payload (2048 bytes)...\n");
    wire = WebIpcWire::empty();
    wire.msg_type = MSG_STATUS;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.payload_len = 2048; // Exceeds 1024!

    let oversized_res = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    match oversized_res {
        Err(SyscallError::InvalidArgument) => {
            debug_print("[WEB_RENDER] PASS: Oversized payload rejected fail-closed (InvalidArgument).\n");
            debug_print("[WEB_02A_OVERSIZED_IPC_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Oversized payload accepted!\n");
            exit(14);
        }
    }

    // Negative Test 5.4: Unauthorized Draw on Browser Chrome (Surface 1)
    debug_print("[WEB_RENDER] Negative Test 5.4: Attempting draw on Browser UI Chrome (Surface 1)...\n");
    cmd = WebRenderCmd::empty();
    cmd.op = WebRenderOp::DrawRect as u32;
    cmd.surface_id = 1; // PROTECTED CHROME SURFACE!
    cmd.x = 0;
    cmd.y = 0;
    cmd.width = 200;
    cmd.height = 40;

    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &cmd as *const WebRenderCmd as *const u8,
            wire.payload.as_mut_ptr(),
            core::mem::size_of::<WebRenderCmd>(),
        );
    }

    let chrome_res = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    match chrome_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Attempted draw on Browser Chrome rejected (PermissionDenied).\n");
            debug_print("[WEB_02A_CHROME_PROTECTION_VERIFIED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Drawing on Browser Chrome allowed!\n");
            exit(15);
        }
    }

    // Negative Test 5.5: Out-of-Bounds and Integer Overflow
    debug_print("[WEB_RENDER] Negative Test 5.5: Attempting out-of-bounds coordinate draw...\n");
    cmd = WebRenderCmd::empty();
    cmd.op = WebRenderOp::DrawRect as u32;
    cmd.surface_id = 2;
    cmd.x = 2000; // Out of 1000 width!
    cmd.y = 10;
    cmd.width = 100;
    cmd.height = 50;

    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &cmd as *const WebRenderCmd as *const u8,
            wire.payload.as_mut_ptr(),
            core::mem::size_of::<WebRenderCmd>(),
        );
    }

    let oob_res = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    match oob_res {
        Err(SyscallError::InvalidArgument) => {
            debug_print("[WEB_RENDER] PASS: Out-of-bounds draw rejected (InvalidArgument).\n");
            debug_print("[WEB_02A_COMPOSITOR_BOUNDS_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Out-of-bounds draw accepted!\n");
            exit(16);
        }
    }

    // Negative Test 5.6: Invalid UTF-8 String in DrawText
    debug_print("[WEB_RENDER] Negative Test 5.6: Attempting invalid UTF-8 sequence in DrawText...\n");
    cmd = WebRenderCmd::empty();
    cmd.op = WebRenderOp::DrawText as u32;
    cmd.surface_id = 2;
    cmd.x = 10;
    cmd.y = 10;
    cmd.text_len = 3;
    cmd.text_bytes[0] = 0xFF; // Invalid UTF-8!
    cmd.text_bytes[1] = 0xFE;
    cmd.text_bytes[2] = 0xFD;

    wire = WebIpcWire::empty();
    wire.msg_type = MSG_RENDER_CMD;
    wire.sender_pid = 310;
    wire.sender_generation = 1;
    wire.payload_len = core::mem::size_of::<WebRenderCmd>() as u32;
    unsafe {
        core::ptr::copy_nonoverlapping(
            &cmd as *const WebRenderCmd as *const u8,
            wire.payload.as_mut_ptr(),
            core::mem::size_of::<WebRenderCmd>(),
        );
    }

    let utf8_res = unsafe {
        raw_syscall(
            SYS_IPC_SEND,
            0,
            &wire as *const WebIpcWire as u64,
            core::mem::size_of::<WebIpcWire>() as u64,
            0,
            0,
        )
    };
    match utf8_res {
        Err(SyscallError::InvalidArgument) => {
            debug_print("[WEB_RENDER] PASS: Invalid UTF-8 sequence rejected fail-closed (InvalidArgument).\n");
            debug_print("[WEB_02A_UTF8_VALIDATION_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] CRITICAL SECURITY FAILURE: Invalid UTF-8 accepted!\n");
            exit(17);
        }
    }

    debug_print("[WEB_RENDER] Milestone WEB-02A ALL ACCEPTANCE GATES PASSED (100% VERIFIED).\n");
    debug_print("[WEB_02A_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0]\n");

    // =========================================================================
    // 6. Milestone WEB-02B HTML Parser Acceptance Gates (Ring 3 Native Parsing)
    // =========================================================================
    debug_print("\n[WEB_RENDER] ======================================================\n");
    debug_print("[WEB_RENDER] Starting Milestone WEB-02B HTML Parser Verification\n");
    debug_print("[WEB_RENDER] ======================================================\n");

    // Gate 1: Representative HTML Document Parsing into Verified DOM
    debug_print("[WEB_RENDER] Gate 1: Parsing representative document into verified DOM...\n");
    let rep_doc = b"<!DOCTYPE html><html><head><title>INOS Browser</title></head><body><h1>INOS Native Browser</h1><p>Streaming HTML tokenizer &amp; bounded DOM parser verified.</p></body></html>";
    match parse_document(rep_doc) {
        Ok(arena) => {
            let html_node = arena.find_element_by_tag("html");
            let head_node = arena.find_element_by_tag("head");
            let title_node = arena.find_element_by_tag("title");
            let body_node = arena.find_element_by_tag("body");
            let h1_node = arena.find_element_by_tag("h1");
            let p_node = arena.find_element_by_tag("p");

            if html_node.is_some() && head_node.is_some() && title_node.is_some()
                && body_node.is_some() && h1_node.is_some() && p_node.is_some()
            {
                debug_print("[WEB_RENDER] Gate 1 PASS: Representative document parsed with full DOM tree.\n");
                debug_print("[WEB_02B_REPRESENTATIVE_DOM_VERIFIED_MARKER_v1.0]\n");
            } else {
                debug_print("[WEB_RENDER] Gate 1 FAIL: Missing expected elements in DOM!\n");
                exit(18);
            }
        }
        Err(_) => {
            debug_print("[WEB_RENDER] Gate 1 FAIL: Document parsing returned error!\n");
            exit(19);
        }
    }

    // Gate 2: Chunk-Boundary Invariance Verification
    debug_print("[WEB_RENDER] Gate 2: Verifying chunk-boundary invariance across chunk splits...\n");
    let invariance_doc = b"<!DOCTYPE html><html><head><title>Chunk &amp; Invariance</title></head><body><!-- test comment --><p>Hello <b>world</b> &quot;test&quot; &copy; 2026!</p></body></html>";
    let arena_full = parse_document(invariance_doc).expect("Full buffer parse failed");
    let arena_1byte = parse_document_chunked(invariance_doc, 1).expect("1-byte chunk parse failed");
    let arena_3byte = parse_document_chunked(invariance_doc, 3).expect("3-byte chunk parse failed");
    let arena_7byte = parse_document_chunked(invariance_doc, 7).expect("7-byte chunk parse failed");

    if arena_full == arena_1byte && arena_full == arena_3byte && arena_full == arena_7byte {
        debug_print("[WEB_RENDER] Gate 2 PASS: 1-byte, 3-byte, and 7-byte chunks yield identical DOMs.\n");
        debug_print("[WEB_02B_CHUNK_INVARIANCE_VERIFIED_MARKER_v1.0]\n");
    } else {
        debug_print("[WEB_RENDER] Gate 2 FAIL: Chunk boundary variation produced divergent DOM trees!\n");
        exit(20);
    }

    // Gate 3: Deep Nesting Limit Stress Test
    debug_print("[WEB_RENDER] Gate 3: Testing deep element nesting recursion limit (TreeDepth <= 64)...\n");
    let mut deep_doc = Vec::new();
    for _ in 0..100 {
        deep_doc.extend_from_slice(b"<div>");
    }
    match parse_document(&deep_doc) {
        Err(ParseError::LimitExceeded(LimitKind::TreeDepth)) => {
            debug_print("[WEB_RENDER] Gate 3 PASS: Deep nesting deterministically rejected with TreeDepth limit.\n");
            debug_print("[WEB_02B_MAX_DEPTH_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] Gate 3 FAIL: Deep nesting was not rejected with TreeDepth limit!\n");
            exit(21);
        }
    }

    // Gate 4: Oversized Attribute & Document Limits
    debug_print("[WEB_RENDER] Gate 4: Testing oversized attribute value and input byte limits...\n");
    let mut huge_attr_doc = Vec::new();
    huge_attr_doc.extend_from_slice(b"<div class=\"");
    for _ in 0..2048 {
        huge_attr_doc.push(b'a');
    }
    huge_attr_doc.extend_from_slice(b"\"></div>");

    match parse_document(&huge_attr_doc) {
        Err(ParseError::LimitExceeded(LimitKind::AttrValueLength)) => {
            debug_print("[WEB_RENDER] Gate 4 PASS: Oversized attribute rejected with AttrValueLength limit.\n");
            debug_print("[WEB_02B_OVERSIZED_ATTR_REJECTED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] Gate 4 FAIL: Oversized attribute was not rejected with AttrValueLength!\n");
            exit(22);
        }
    }

    // Gate 5: Malformed Stream & Broken Entity Recovery
    debug_print("[WEB_RENDER] Gate 5: Testing malformed HTML stream and entity recovery...\n");
    let mut malformed_stream = Vec::new();
    malformed_stream.extend_from_slice(b"<p>Broken &unknown; entity and unclosed <b>tag");
    // Trailing incomplete 3-byte UTF-8 sequence (first 2 bytes of 0xE2 0x82 0xAC)
    malformed_stream.push(0xE2);
    malformed_stream.push(0x82);

    match parse_document(&malformed_stream) {
        Ok(arena) => {
            let p_id = arena.find_element_by_tag("p").expect("p should exist");
            let p_node = arena.get(p_id).unwrap();
            let p_child = arena.get(p_node.first_child.unwrap()).unwrap();
            let mut entity_preserved = false;
            if let NodeData::Text(ref s) = p_child.data {
                if s.contains("&unknown;") {
                    entity_preserved = true;
                }
            }

            let b_id = arena.find_element_by_tag("b").expect("b should exist");
            let b_node = arena.get(b_id).unwrap();
            let b_child = arena.get(b_node.first_child.unwrap()).unwrap();
            let mut ufffd_present = false;
            if let NodeData::Text(ref s) = b_child.data {
                if s.contains('\u{FFFD}') {
                    ufffd_present = true;
                }
            }

            if entity_preserved && ufffd_present {
                debug_print("[WEB_RENDER] Gate 5 PASS: Malformed entity preserved literally, incomplete UTF-8 replaced with U+FFFD.\n");
                debug_print("[WEB_02B_MALFORMED_RECOVERY_VERIFIED_MARKER_v1.0]\n");
            } else {
                debug_print("[WEB_RENDER] Gate 5 FAIL: Malformed entity or incomplete UTF-8 not recovered properly!\n");
                exit(23);
            }
        }
        Err(_) => {
            debug_print("[WEB_RENDER] Gate 5 FAIL: Malformed document failed to recover!\n");
            exit(24);
        }
    }

    // Gate 6: Inert Script and Style Handling
    debug_print("[WEB_RENDER] Gate 6: Testing inert script & style execution with passive event attributes...\n");
    let script_html = b"<html><head><script>alert('xss'); while(true){}</script><style>body { color: red; }</style></head><body><div onclick=\"runBad()\">Click</div></body></html>";
    match parse_document(script_html) {
        Ok(arena) => {
            let script_id = arena.find_element_by_tag("script").expect("script element should exist");
            let script_node = arena.get(script_id).unwrap();
            let script_child = arena.get(script_node.first_child.expect("script text child")).unwrap();
            let mut script_inert = false;
            if let NodeData::Text(ref s) = script_child.data {
                if s == "alert('xss'); while(true){}" {
                    script_inert = true;
                }
            }

            let div_id = arena.find_element_by_tag("div").expect("div element should exist");
            let div_node = arena.get(div_id).unwrap();
            let mut event_passive = false;
            if let NodeData::Element(ref el) = div_node.data {
                if el.get_attribute("onclick") == Some("runBad()") {
                    event_passive = true;
                }
            }

            if script_inert && event_passive {
                debug_print("[WEB_RENDER] Gate 6 PASS: Script tags stored as inert text; event handlers stored passively.\n");
                debug_print("[WEB_02B_INERT_SCRIPT_VERIFIED_MARKER_v1.0]\n");
            } else {
                debug_print("[WEB_RENDER] Gate 6 FAIL: Script or event handler not correctly stored!\n");
                exit(25);
            }
        }
        Err(_) => {
            debug_print("[WEB_RENDER] Gate 6 FAIL: Script document parse returned error!\n");
            exit(26);
        }
    }

    // Gate 7: Zero Ambient Authority Maintained After Parsing
    debug_print("[WEB_RENDER] Gate 7: Verifying zero ambient authority maintained post-parsing...\n");
    let post_sock_res = unsafe {
        raw_syscall(SYS_NET_SOCKET, 88888, 0, 0, 0, 0)
    };
    match post_sock_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Post-parsing socket creation denied fail-closed.\n");
        }
        _ => {
            debug_print("[WEB_RENDER] Gate 7 FAIL: Post-parsing socket created!\n");
            exit(27);
        }
    }

    let post_file_res = unsafe {
        raw_syscall(SYS_FILE_CREATE, fs_path.as_ptr() as u64, 15, 0, 0, 0)
    };
    match post_file_res {
        Err(SyscallError::PermissionDenied) => {
            debug_print("[WEB_RENDER] PASS: Post-parsing file creation denied fail-closed.\n");
            debug_print("[WEB_02B_ZERO_AMBIENT_MAINTAINED_MARKER_v1.0]\n");
        }
        _ => {
            debug_print("[WEB_RENDER] Gate 7 FAIL: Post-parsing file created!\n");
            exit(28);
        }
    }

    // =========================================================================
    // 8. Interactive Compositor Browser & DOM Inspector (Live Desktop Mode)
    // =========================================================================
    if let Ok(mut shell) = inos_app_sdk::compositor::AppShell::new(720, 480, "INOS Web Browser - Isolated Ring 3 Renderer") {
        let mut active_page: u8 = 0; // 0=Welcome, 1=Security, 2=Conformance, 3=IntScript Docs
        let mut inspector_mode: bool = false;
        let mut selected_link_idx: usize = 0;
        let mut scroll_y: i32 = 0;
        let mut dynamic_html_buf: [u8; 1024] = [0u8; 1024];
        let mut dynamic_html_len: usize = 0;
        let mut status_msg = "Zero-Ambient Sandbox (PID 310) | [1-3]:Pages [4]:DOM Inspector [Tab/Enter]:Links";

        let mut active_arena = parse_document(builtin_page_html(0).1)
            .unwrap_or_else(|_| DomArena::new(MAX_DOM_NODES).unwrap());
        let init_title = extract_document_title(&active_arena);
        shell.stage_ribbon_offer("OFFER: lora_web", init_title);

        shell.run_event_loop(|sh, ev| {
            let mut page_changed = false;

            // Poll incoming WebIpcWire / AutomationIpcWire document or inspection commands
            let mut ipc_doc = WebIpcWire::empty();
            let doc_recv = unsafe {
                raw_syscall(
                    SYS_IPC_RECV,
                    IPC_CHAN_DOC_FEED as u64,
                    &mut ipc_doc as *mut WebIpcWire as u64,
                    core::mem::size_of::<WebIpcWire>() as u64,
                    0,
                    0,
                )
            };
            if let Ok(n) = doc_recv {
                if n > 0 && ipc_doc.magic == WEB_IPC_MAGIC && ipc_doc.msg_type == MSG_DOC_FEED {
                    let plen = (ipc_doc.payload_len as usize).min(WEB_IPC_MAX_PAYLOAD);
                    if plen > 0 {
                        dynamic_html_buf[..plen].copy_from_slice(&ipc_doc.payload[..plen]);
                        dynamic_html_len = plen;
                        active_page = 4;
                        page_changed = true;
                        status_msg = "Loaded dynamic HTML5 stream from IPC_CHAN_DOC_FEED.";
                    }
                }
            }

            if let Ok((_sender, _msg_type, auto_wire)) = inos_app_sdk::recv_automation_ipc() {
                if auto_wire.op_code == inos_app_sdk::compositor::OP_STAGE_RIBBON
                    || auto_wire.op_code == inos_app_sdk::compositor::OP_BIND_RIBBON
                {
                    let _ = sh.redraw_ribbon_and_status_only(status_msg);
                    return true;
                } else if auto_wire.op_code == inos_app_sdk::OP_SET_CELL
                    || auto_wire.op_code == inos_app_sdk::OP_EVALUATE
                    || auto_wire.op_code == inos_app_sdk::compositor::OP_INSPECT
                {
                    let f_len = auto_wire.formula.iter().position(|&b| b == 0).unwrap_or(32);
                    if f_len > 0 && auto_wire.formula[0] == b'<' {
                        dynamic_html_buf[..f_len].copy_from_slice(&auto_wire.formula[..f_len]);
                        dynamic_html_len = f_len;
                        active_page = 4;
                        page_changed = true;
                        status_msg = "Loaded live HTML fragment via AutomationIpcWire.";
                    } else if auto_wire.op_code == inos_app_sdk::compositor::OP_INSPECT {
                        inspector_mode = true;
                        status_msg = "Switched to Live DOM Arena Inspector via OP_INSPECT.";
                    }
                }
            }

            let (_total_links_prev, current_href_opt) =
                scan_arena_links(&active_arena, selected_link_idx);

            if let Some(event) = ev {
                match event {
                    inos_app_sdk::compositor::InputEvent::KeyPress { scancode, ascii } => {
                        if ascii == b'1' {
                            active_page = 0;
                            inspector_mode = false;
                            page_changed = true;
                            status_msg = "Loaded inos://welcome/index.html (DOM Layout Active)";
                        } else if ascii == b'2' {
                            active_page = 1;
                            inspector_mode = false;
                            page_changed = true;
                            status_msg = "Loaded inos://security/sandbox-policy.html";
                        } else if ascii == b'3' {
                            active_page = 2;
                            inspector_mode = false;
                            page_changed = true;
                            status_msg = "Loaded inos://intscript/docs.html";
                        } else if ascii == b'4' {
                            inspector_mode = !inspector_mode;
                            scroll_y = 0;
                            status_msg = if inspector_mode {
                                "Live DOM Tree Inspector Active: walking arena NodeIds & attributes"
                            } else {
                                "Returned to Hypertext Layout Viewport"
                            };
                        } else if scancode == inos_app_sdk::compositor::KEY_TAB || ascii == b'n' || ascii == b'N' {
                            let (link_cnt, _) = scan_arena_links(&active_arena, 0);
                            if link_cnt > 0 {
                                selected_link_idx = (selected_link_idx + 1) % link_cnt;
                                status_msg = "Focused next <a href> hyperlink (Press Enter to follow)";
                            }
                        } else if scancode == inos_app_sdk::compositor::KEY_ENTER || ascii == b'\r' || ascii == b'\n' {
                            if let Some(href) = current_href_opt {
                                if let Some(target_idx) = resolve_url_to_page_idx(href) {
                                    active_page = target_idx;
                                    inspector_mode = false;
                                    page_changed = true;
                                    status_msg = "Followed <a href> hyperlink to target inos:// document";
                                }
                            }
                        } else if scancode == inos_app_sdk::compositor::KEY_DOWN {
                            scroll_y = (scroll_y + 18).min(180);
                        } else if scancode == inos_app_sdk::compositor::KEY_UP {
                            scroll_y = (scroll_y - 18).max(0);
                        }
                    }
                    inos_app_sdk::compositor::InputEvent::MouseEvent { x, y, buttons } => {
                        if (buttons & 1) != 0 && y >= 7 && y <= 31 {
                            if x >= 8 && x < 92 {
                                active_page = 0;
                                inspector_mode = false;
                                page_changed = true;
                            } else if x >= 96 && x < 196 {
                                active_page = 1;
                                inspector_mode = false;
                                page_changed = true;
                            } else if x >= 200 && x < 296 {
                                active_page = 2;
                                inspector_mode = false;
                                page_changed = true;
                            } else if x >= 300 && x < 406 {
                                inspector_mode = !inspector_mode;
                                scroll_y = 0;
                            }
                        }
                    }
                }
            }

            if page_changed {
                selected_link_idx = 0;
                scroll_y = 0;
                let src_bytes: &[u8] = if active_page == 4 && dynamic_html_len > 0 {
                    &dynamic_html_buf[..dynamic_html_len]
                } else {
                    builtin_page_html(active_page).1
                };
                if let Ok(parsed_arena) = parse_document(src_bytes) {
                    active_arena = parsed_arena;
                    let page_title = extract_document_title(&active_arena);
                    sh.stage_ribbon_offer("OFFER: lora_web", page_title);
                }
            }

            let url_str = if active_page == 4 {
                "inos://ipc-stream/live.html"
            } else {
                builtin_page_html(active_page).0
            };

            let _ = sh.clear(0x001E293B);
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 0, 0, 720, 38, 0x000F172A);
            let _ = sh.draw_button(8, 7, 84, 24, "[1] Home", !inspector_mode && active_page == 0);
            let _ = sh.draw_button(96, 7, 100, 24, "[2] Sandbox", !inspector_mode && active_page == 1);
            let _ = sh.draw_button(200, 7, 96, 24, "[3] Docs", !inspector_mode && active_page == 2);
            let _ = sh.draw_button(300, 7, 106, 24, "[4] DOM Tree", inspector_mode);
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 412, 7, 296, 24, 0x001E293B);
            let _ = inos_app_sdk::compositor::draw_text(sh.surface_id, 418, 12, 0x0038BDF8, url_str);

            // Viewport area
            let _ = inos_app_sdk::compositor::fill_rect(sh.surface_id, 12, 46, 696, 382, 0x00F8FAFC);
            if inspector_mode {
                render_dom_inspector_to_surface(sh.surface_id, &active_arena, scroll_y);
            } else {
                let (_link_count, _focused_href) = render_dom_arena_to_surface(
                    sh.surface_id,
                    &active_arena,
                    selected_link_idx,
                    scroll_y,
                );
            }

            let _ = sh.draw_status_bar(status_msg);
            true
        });
    }

    debug_print("\n[WEB_RENDER] Milestone WEB-02B ALL ACCEPTANCE GATES PASSED (100% VERIFIED).\n");
    debug_print("[WEB_02B_ALL_ACCEPTANCE_GATES_PASSED_MARKER_v1.0]\n");

    exit(0);
}

fn builtin_page_html(page_idx: u8) -> (&'static str, &'static [u8]) {
    match page_idx {
        0 => (
            "inos://welcome/index.html",
            b"<!DOCTYPE html><html><head><title>INOS Native Browser - Ring 3</title></head><body>\
              <h1>INOS Native Browser (Ring 3 Isolated Renderer)</h1>\
              <hr>\
              <p>Streaming HTML5 tokenizer and bounded DOM Arena layout engine active.</p>\
              <ul>\
                <li>Hardware page table (CPL=3) with zero ambient FS or Network authority.</li>\
                <li>Deterministic limits: MAX_DOM_NODES=1024, TreeDepth=64, AttrValue=1024.</li>\
                <li>Chunk-boundary invariance verified across 1-byte, 3-byte, and 7-byte splits.</li>\
              </ul>\
              <h2>Workspace Hyperlinks (Tab + Enter to Navigate)</h2>\
              <a href=\"inos://security/sandbox-policy.html\">Open Renderer Capability Isolation Policy</a>\
              <a href=\"inos://intscript/docs.html\">Open IntScript &amp; DOM Conformance Reference</a>\
              <code>inos.web.ipc.v1 | PID=310 | Zero-Ambient-Authority</code>\
              </body></html>",
        ),
        1 => (
            "inos://security/sandbox-policy.html",
            b"<!DOCTYPE html><html><head><title>Renderer Capability Isolation Matrix</title></head><body>\
              <h1>Renderer Capability Isolation Matrix</h1>\
              <hr>\
              <p>Every untrusted document is parsed inside an isolated Ring 3 address space.</p>\
              <table>\
                <tr><td>Boundary Check</td><td>Enforcement</td><td>Status</td></tr>\
                <tr><td>SYS_NET_SOCKET</td><td>Fail-Closed</td><td>PASS (Denied)</td></tr>\
                <tr><td>SYS_FILE_CREATE</td><td>Fail-Closed</td><td>PASS (Denied)</td></tr>\
                <tr><td>Surface ID 1 Chrome</td><td>PID Ownership</td><td>PASS (Protected)</td></tr>\
              </table>\
              <ul>\
                <li>Inline &lt;script&gt; and event attributes stored as passive inert text.</li>\
              </ul>\
              <a href=\"inos://welcome/index.html\">Return to INOS Browser Home</a>\
              <a href=\"inos://intscript/docs.html\">View IntScript &amp; Conformance Docs</a>\
              </body></html>",
        ),
        2 => (
            "inos://intscript/docs.html",
            b"<!DOCTYPE html><html><head><title>IntScript & DOM Conformance Docs</title></head><body>\
              <h1>IntScript &amp; DOM Conformance Specification</h1>\
              <hr>\
              <p>WHATWG entity decoding, surrogate blocking, and deterministic work budgets.</p>\
              <ul>\
                <li>Named entities: &amp;amp; &amp;lt; &amp;gt; &amp;quot; &amp;apos; &amp;nbsp; &amp;copy;</li>\
                <li>Malformed UTF-8 &amp; UTF-16 surrogates replaced with U+FFFD replacement char.</li>\
                <li>Cross-App Continuity: stages verified citation offers to Chrome Ribbon.</li>\
              </ul>\
              <code>inos.ipc.call('org.inos.web', 'inspect', 'inos://welcome/index.html')</code>\
              <a href=\"inos://welcome/index.html\">Back to Welcome Page</a>\
              <a href=\"inos://security/sandbox-policy.html\">Inspect Security Sandbox Matrix</a>\
              </body></html>",
        ),
        _ => (
            "inos://dom-inspector/conformance.html",
            b"<!DOCTYPE html><html><head><title>DOM Conformance Suite</title></head><body>\
              <h1>DOM Arena Conformance</h1><hr><p>All 7 WEB-02A/02B gates verified.</p>\
              <a href=\"inos://welcome/index.html\">Home</a></body></html>",
        ),
    }
}

fn resolve_url_to_page_idx(href: &str) -> Option<u8> {
    if href.contains("welcome") {
        Some(0)
    } else if href.contains("security") {
        Some(1)
    } else if href.contains("intscript") || href.contains("docs") {
        Some(2)
    } else if href.contains("dom-inspector") || href.contains("conformance") {
        Some(3)
    } else {
        None
    }
}

fn collect_node_text<'a>(arena: &'a DomArena, node_id: dom::NodeId, out: &mut [u8]) -> usize {
    let mut len = 0usize;
    let mut cur = arena.get(node_id).and_then(|n| n.first_child);
    while let Some(cid) = cur {
        if let Some(cnode) = arena.get(cid) {
            match cnode.data {
                NodeData::Text(ref t) => {
                    for &b in t.as_bytes() {
                        if b == b'\n' || b == b'\r' {
                            continue;
                        }
                        if len < out.len() {
                            out[len] = b;
                            len += 1;
                        }
                    }
                }
                NodeData::Element(_) => {
                    let sub_len = collect_node_text(arena, cid, &mut out[len..]);
                    len += sub_len;
                }
                _ => {}
            }
            cur = cnode.next_sibling;
        } else {
            break;
        }
    }
    len
}

pub fn extract_document_title(arena: &DomArena) -> &str {
    if let Some(tid) = arena.find_element_by_tag("title") {
        if let Some(tnode) = arena.get(tid) {
            if let Some(cid) = tnode.first_child {
                if let Some(cnode) = arena.get(cid) {
                    if let NodeData::Text(ref s) = cnode.data {
                        return s.as_str();
                    }
                }
            }
        }
    }
    "Verified DOM Citation -> notes_output.txt"
}

fn scan_arena_links<'a>(arena: &'a DomArena, selected_link_idx: usize) -> (usize, Option<&'a str>) {
    let mut link_count = 0usize;
    let mut selected_href: Option<&'a str> = None;
    for idx in 0..arena.node_count() {
        if let Some(node) = arena.get(dom::NodeId(idx as u32)) {
            if let NodeData::Element(ref el) = node.data {
                if el.tag_name.eq_ignore_ascii_case("a") {
                    if let Some(href) = el.get_attribute("href") {
                        if link_count == selected_link_idx {
                            selected_href = Some(href);
                        }
                        link_count += 1;
                    }
                }
            }
        }
    }
    (link_count, selected_href)
}

/// Traverses the live `DomArena` tree and renders block/inline layout boxes for
/// `<h1>`, `<h2>`, `<p>`, `<ul>`/`<li>`, `<a href="...">`, `<code>`, `<hr>`, and `<table>`/`<tr>`/`<td>`.
pub fn render_dom_arena_to_surface<'a>(
    surface_id: u32,
    arena: &'a DomArena,
    selected_link_idx: usize,
    scroll_y: i32,
) -> (usize, Option<&'a str>) {
    let mut cursor_y: i32 = 58 - scroll_y;
    let mut link_count: usize = 0;
    let mut selected_href: Option<&'a str> = None;

    let start_id = arena
        .find_element_by_tag("body")
        .unwrap_or(arena.document_id);

    render_dom_subtree(
        surface_id,
        arena,
        start_id,
        &mut cursor_y,
        &mut link_count,
        selected_link_idx,
        &mut selected_href,
    );

    (link_count, selected_href)
}

fn render_dom_subtree<'a>(
    surface_id: u32,
    arena: &'a DomArena,
    parent_id: dom::NodeId,
    cursor_y: &mut i32,
    link_count: &mut usize,
    selected_link_idx: usize,
    selected_href: &mut Option<&'a str>,
) {
    let mut child_opt = arena.get(parent_id).and_then(|n| n.first_child);
    while let Some(node_id) = child_opt {
        let node = match arena.get(node_id) {
            Some(n) => n,
            None => break,
        };
        if let NodeData::Element(ref el) = node.data {
            let tag = el.tag_name.as_str();
            if tag.eq_ignore_ascii_case("head")
                || tag.eq_ignore_ascii_case("title")
                || tag.eq_ignore_ascii_case("script")
                || tag.eq_ignore_ascii_case("style")
            {
                child_opt = node.next_sibling;
                continue;
            }

            let mut text_buf = [0u8; 96];
            if tag.eq_ignore_ascii_case("h1") {
                let tlen = collect_node_text(arena, node_id, &mut text_buf);
                if *cursor_y >= 48 && *cursor_y <= 405 {
                    if let Ok(s) = core::str::from_utf8(&text_buf[..tlen]) {
                        let _ = inos_app_sdk::compositor::draw_text(
                            surface_id,
                            26,
                            *cursor_y as u32,
                            0x001E3A8A,
                            s,
                        );
                    }
                }
                *cursor_y += 24;
            } else if tag.eq_ignore_ascii_case("h2") {
                let tlen = collect_node_text(arena, node_id, &mut text_buf);
                if *cursor_y >= 48 && *cursor_y <= 405 {
                    if let Ok(s) = core::str::from_utf8(&text_buf[..tlen]) {
                        let _ = inos_app_sdk::compositor::draw_text(
                            surface_id,
                            26,
                            *cursor_y as u32,
                            0x000F172A,
                            s,
                        );
                    }
                }
                *cursor_y += 22;
            } else if tag.eq_ignore_ascii_case("hr") {
                if *cursor_y >= 48 && *cursor_y <= 415 {
                    let _ = inos_app_sdk::compositor::fill_rect(
                        surface_id,
                        26,
                        *cursor_y as u32,
                        660,
                        2,
                        0x00CBD5E1,
                    );
                }
                *cursor_y += 12;
            } else if tag.eq_ignore_ascii_case("p") {
                let tlen = collect_node_text(arena, node_id, &mut text_buf);
                if *cursor_y >= 48 && *cursor_y <= 405 {
                    if let Ok(s) = core::str::from_utf8(&text_buf[..tlen]) {
                        let _ = inos_app_sdk::compositor::draw_text(
                            surface_id,
                            26,
                            *cursor_y as u32,
                            0x00334155,
                            s,
                        );
                    }
                }
                *cursor_y += 22;
            } else if tag.eq_ignore_ascii_case("li") {
                let mut li_buf = [0u8; 96];
                li_buf[0] = b'*';
                li_buf[1] = b' ';
                let tlen = collect_node_text(arena, node_id, &mut li_buf[2..]);
                if *cursor_y >= 48 && *cursor_y <= 405 {
                    if let Ok(s) = core::str::from_utf8(&li_buf[..tlen + 2]) {
                        let _ = inos_app_sdk::compositor::draw_text(
                            surface_id,
                            36,
                            *cursor_y as u32,
                            0x0015803D,
                            s,
                        );
                    }
                }
                *cursor_y += 20;
            } else if tag.eq_ignore_ascii_case("a") {
                let href = el.get_attribute("href").unwrap_or("");
                let is_selected = *link_count == selected_link_idx;
                if is_selected {
                    *selected_href = Some(href);
                }
                *link_count += 1;

                let mut link_buf = [0u8; 96];
                let prefix = if is_selected { b"[LINK ->] " as &[u8] } else { b"  [link]  " as &[u8] };
                link_buf[..prefix.len()].copy_from_slice(prefix);
                let tlen = collect_node_text(arena, node_id, &mut link_buf[prefix.len()..]);
                let total_len = prefix.len() + tlen;

                if *cursor_y >= 48 && *cursor_y <= 405 {
                    let y_pos = *cursor_y as u32;
                    if is_selected {
                        let _ = inos_app_sdk::compositor::fill_rect(
                            surface_id,
                            26,
                            y_pos.saturating_sub(2),
                            650,
                            18,
                            0x00DBEAFE,
                        );
                        let _ = inos_app_sdk::compositor::fill_rect(
                            surface_id,
                            26,
                            y_pos + 15,
                            420,
                            2,
                            0x002563EB,
                        );
                    }
                    if let Ok(s) = core::str::from_utf8(&link_buf[..total_len]) {
                        let fg = if is_selected { 0x001D4ED8 } else { 0x002563EB };
                        let _ = inos_app_sdk::compositor::draw_text(surface_id, 30, y_pos, fg, s);
                    }
                }
                *cursor_y += 22;
            } else if tag.eq_ignore_ascii_case("code") {
                let tlen = collect_node_text(arena, node_id, &mut text_buf);
                if *cursor_y >= 48 && *cursor_y <= 405 {
                    let y_pos = *cursor_y as u32;
                    let _ = inos_app_sdk::compositor::fill_rect(
                        surface_id,
                        26,
                        y_pos.saturating_sub(2),
                        650,
                        18,
                        0x00E2E8F0,
                    );
                    if let Ok(s) = core::str::from_utf8(&text_buf[..tlen]) {
                        let _ = inos_app_sdk::compositor::draw_text(
                            surface_id,
                            32,
                            y_pos,
                            0x000F172A,
                            s,
                        );
                    }
                }
                *cursor_y += 22;
            } else if tag.eq_ignore_ascii_case("tr") {
                let mut col_idx = 0u32;
                let mut td_opt = node.first_child;
                while let Some(td_id) = td_opt {
                    if let Some(td_node) = arena.get(td_id) {
                        if let NodeData::Element(ref td_el) = td_node.data {
                            if td_el.tag_name.eq_ignore_ascii_case("td")
                                || td_el.tag_name.eq_ignore_ascii_case("th")
                            {
                                let mut cell_buf = [0u8; 48];
                                let clen = collect_node_text(arena, td_id, &mut cell_buf);
                                if *cursor_y >= 48 && *cursor_y <= 405 {
                                    let cx = 30 + col_idx * 210;
                                    let cy = *cursor_y as u32;
                                    let _ = inos_app_sdk::compositor::fill_rect(
                                        surface_id,
                                        cx.saturating_sub(4),
                                        cy.saturating_sub(2),
                                        204,
                                        18,
                                        0x00E2E8F0,
                                    );
                                    if let Ok(s) = core::str::from_utf8(&cell_buf[..clen]) {
                                        let _ = inos_app_sdk::compositor::draw_text(
                                            surface_id,
                                            cx,
                                            cy,
                                            0x000F172A,
                                            s,
                                        );
                                    }
                                }
                                col_idx += 1;
                            }
                        }
                        td_opt = td_node.next_sibling;
                    } else {
                        break;
                    }
                }
                *cursor_y += 22;
            } else {
                render_dom_subtree(
                    surface_id,
                    arena,
                    node_id,
                    cursor_y,
                    link_count,
                    selected_link_idx,
                    selected_href,
                );
            }
        }
        child_opt = node.next_sibling;
    }
}

/// Renders the live DOM Tree Inspector tab (`[4]`) by walking actual `DomArena` nodes,
/// tag names, attributes (`href`, `class`), and text content.
pub fn render_dom_inspector_to_surface(surface_id: u32, arena: &DomArena, scroll_y: i32) {
    let _ = inos_app_sdk::compositor::draw_text(
        surface_id,
        24,
        56,
        0x001E3A8A,
        "LIVE DOM ARENA INSPECTOR (Walking Real Parsed NodeId Graph)",
    );
    let _ = inos_app_sdk::compositor::fill_rect(surface_id, 24, 74, 664, 2, 0x00CBD5E1);

    let mut row_y: i32 = 84 - scroll_y;
    render_dom_inspector_node(surface_id, arena, arena.document_id, 0, &mut row_y);
}

fn append_bytes(dst: &mut [u8], cursor: &mut usize, src: &[u8]) {
    for &b in src {
        if *cursor < dst.len() {
            dst[*cursor] = b;
            *cursor += 1;
        }
    }
}

fn append_u32_dec(dst: &mut [u8], cursor: &mut usize, mut val: u32) {
    if val == 0 {
        append_bytes(dst, cursor, b"0");
        return;
    }
    let mut tmp = [0u8; 10];
    let mut len = 0;
    while val > 0 {
        tmp[len] = b'0' + (val % 10) as u8;
        val /= 10;
        len += 1;
    }
    for i in (0..len).rev() {
        append_bytes(dst, cursor, &[tmp[i]]);
    }
}

fn render_dom_inspector_node(
    surface_id: u32,
    arena: &DomArena,
    node_id: dom::NodeId,
    depth: u32,
    row_y: &mut i32,
) {
    let node = match arena.get(node_id) {
        Some(n) => n,
        None => return,
    };

    let mut line = [0u8; 96];
    let mut cur = 0usize;
    append_bytes(&mut line, &mut cur, b"#");
    append_u32_dec(&mut line, &mut cur, node.id.0);
    append_bytes(&mut line, &mut cur, b" ");

    let mut color: u32 = 0x000F172A;
    let mut should_render = true;

    match node.data {
        NodeData::Document => {
            append_bytes(&mut line, &mut cur, b"DocumentRoot");
            color = 0x000F172A;
        }
        NodeData::Doctype { ref name } => {
            append_bytes(&mut line, &mut cur, b"<!DOCTYPE ");
            append_bytes(&mut line, &mut cur, name.as_bytes());
            append_bytes(&mut line, &mut cur, b">");
            color = 0x0064748B;
        }
        NodeData::Element(ref el) => {
            append_bytes(&mut line, &mut cur, b"<");
            append_bytes(&mut line, &mut cur, el.tag_name.as_bytes());
            if let Some(href) = el.get_attribute("href") {
                append_bytes(&mut line, &mut cur, b" href=\"");
                append_bytes(&mut line, &mut cur, href.as_bytes());
                append_bytes(&mut line, &mut cur, b"\"");
            }
            if let Some(cls) = el.get_attribute("class") {
                append_bytes(&mut line, &mut cur, b" class=\"");
                append_bytes(&mut line, &mut cur, cls.as_bytes());
                append_bytes(&mut line, &mut cur, b"\"");
            }
            append_bytes(&mut line, &mut cur, b">");
            color = 0x001E3A8A;
        }
        NodeData::Text(ref t) => {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                should_render = false;
            } else {
                append_bytes(&mut line, &mut cur, b"Text \"");
                let max_t = trimmed.len().min(48);
                append_bytes(&mut line, &mut cur, &trimmed.as_bytes()[..max_t]);
                append_bytes(&mut line, &mut cur, b"\"");
                color = 0x0015803D;
            }
        }
        NodeData::Comment(ref c) => {
            append_bytes(&mut line, &mut cur, b"<!-- ");
            append_bytes(&mut line, &mut cur, c.as_bytes());
            append_bytes(&mut line, &mut cur, b" -->");
            color = 0x0064748B;
        }
    }

    if should_render {
        if *row_y >= 80 && *row_y <= 410 {
            let x_indent = (24 + depth.min(8) * 18) as u32;
            if let Ok(s) = core::str::from_utf8(&line[..cur]) {
                let _ = inos_app_sdk::compositor::draw_text(
                    surface_id,
                    x_indent,
                    *row_y as u32,
                    color,
                    s,
                );
            }
        }
        *row_y += 18;
    }

    let mut child_opt = node.first_child;
    while let Some(cid) = child_opt {
        render_dom_inspector_node(surface_id, arena, cid, depth + 1, row_y);
        child_opt = arena.get(cid).and_then(|c| c.next_sibling);
    }
}

