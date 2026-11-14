// userspace/sdk/src/compositor.rs - Typed Compositor IPC Client API (APP-CAP-01)

use crate::syscall::{self, raw_syscall, SyscallError};

pub const COMPOSITOR_MAGIC: u32 = 0x434D5053;
pub const COMPOSITOR_VERSION: u32 = 1;
pub const RECEIVER_COMPOSITOR: u64 = 1;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorOp {
    RegisterSurface = 1,
    DrawText = 2,
    FillRect = 3,
    Present = 4,
    CloseSurface = 5,
    PollInputEvent = 6,
    SendInputEvent = 7,
    SetClipboard = 8,
    GetClipboard = 9,
    SetFocus = 10,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventType {
    None = 0,
    KeyDown = 1,
    KeyUp = 2,
    MouseDown = 3,
    MouseUp = 4,
    MouseMove = 5,
    Command = 6,
}

#[derive(Debug, Clone, Copy)]
pub struct RawInputEvent {
    pub event_type: InputEventType,
    pub arg1: u32, // char or mouse_x
    pub arg2: u32, // scancode or mouse_y
    pub arg3: u32, // modifiers or mouse_button
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    KeyPress { scancode: u32, ascii: u8 },
    MouseEvent { x: i32, y: i32, buttons: u32 },
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct CompositorMsg {
    pub magic: u32,
    pub version: u32,
    pub op: u32,
    pub surface_id: u32,
    pub arg1: u32,
    pub arg2: u32,
    pub arg3: u32,
    pub arg4: u32,
    pub payload: [u8; 64],
}

impl CompositorMsg {
    pub const fn new(op: CompositorOp, surface_id: u32) -> Self {
        Self {
            magic: COMPOSITOR_MAGIC,
            version: COMPOSITOR_VERSION,
            op: op as u32,
            surface_id,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            arg4: 0,
            payload: [0u8; 64],
        }
    }
}

pub fn create_surface(width: u32, height: u32, title: &str) -> Result<u32, SyscallError> {
    let mut msg = CompositorMsg::new(CompositorOp::RegisterSurface, 0);
    msg.arg1 = width;
    msg.arg2 = height;
    let copy_len = core::cmp::min(title.len(), 32);
    msg.payload[..copy_len].copy_from_slice(&title.as_bytes()[..copy_len]);

    let res = unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?
    };

    Ok(res as u32)
}

pub fn draw_text(surface_id: u32, x: u32, y: u32, color: u32, text: &str) -> Result<(), SyscallError> {
    let mut msg = CompositorMsg::new(CompositorOp::DrawText, surface_id);
    msg.arg1 = x;
    msg.arg2 = y;
    msg.arg3 = color;
    let copy_len = core::cmp::min(text.len(), 64);
    msg.arg4 = copy_len as u32;
    msg.payload[..copy_len].copy_from_slice(&text.as_bytes()[..copy_len]);

    unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?;
    }

    Ok(())
}

pub fn fill_rect(surface_id: u32, x: u32, y: u32, w: u32, h: u32, color: u32) -> Result<(), SyscallError> {
    let mut msg = CompositorMsg::new(CompositorOp::FillRect, surface_id);
    msg.arg1 = x;
    msg.arg2 = y;
    msg.arg3 = w;
    msg.arg4 = h;
    msg.payload[0..4].copy_from_slice(&color.to_le_bytes());

    unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?;
    }

    Ok(())
}

pub fn present_surface(surface_id: u32) -> Result<(), SyscallError> {
    let msg = CompositorMsg::new(CompositorOp::Present, surface_id);
    unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?;
    }
    Ok(())
}

pub fn close_surface(surface_id: u32) -> Result<(), SyscallError> {
    let msg = CompositorMsg::new(CompositorOp::CloseSurface, surface_id);
    unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?;
    }
    Ok(())
}

pub fn poll_input_event_ext(surface_id: u32) -> (Option<RawInputEvent>, bool) {
    let mut msg = CompositorMsg::new(CompositorOp::PollInputEvent, surface_id);
    let res = unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &mut msg as *mut CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )
    };
    let interactive = msg.payload[0] != 0;
    match res {
        Ok(count) if count > 0 => {
            let ev_type = match msg.arg1 {
                1 => InputEventType::KeyDown,
                2 => InputEventType::KeyUp,
                3 => InputEventType::MouseDown,
                4 => InputEventType::MouseUp,
                5 => InputEventType::MouseMove,
                6 => InputEventType::Command,
                _ => InputEventType::None,
            };
            (
                Some(RawInputEvent {
                    event_type: ev_type,
                    arg1: msg.arg2,
                    arg2: msg.arg3,
                    arg3: msg.arg4,
                }),
                interactive,
            )
        }
        _ => (None, interactive),
    }
}

pub fn poll_input_event(surface_id: u32) -> Option<RawInputEvent> {
    poll_input_event_ext(surface_id).0
}

pub fn set_clipboard(surface_id: u32, cap_token: u64, data: &[u8]) -> Result<usize, SyscallError> {
    let full_token = crate::resolve_full_token(cap_token);
    let token_low = full_token as u64;
    let token_high = (full_token >> 64) as u64;
    let mut msg = CompositorMsg::new(CompositorOp::SetClipboard, surface_id);
    let copy_len = core::cmp::min(data.len(), 64);
    msg.arg1 = copy_len as u32;
    msg.arg2 = (token_low & 0xFFFF_FFFF) as u32;
    msg.arg3 = (token_low >> 32) as u32;
    msg.arg4 = (token_high & 0xFFFF_FFFF) as u32;
    msg.payload[..copy_len].copy_from_slice(&data[..copy_len]);
    let res = unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?
    };
    Ok(res as usize)
}

pub fn get_clipboard(surface_id: u32, cap_token: u64, out_buf: &mut [u8]) -> Result<usize, SyscallError> {
    let full_token = crate::resolve_full_token(cap_token);
    let token_low = full_token as u64;
    let token_high = (full_token >> 64) as u64;
    let mut msg = CompositorMsg::new(CompositorOp::GetClipboard, surface_id);
    msg.arg2 = (token_low & 0xFFFF_FFFF) as u32;
    msg.arg3 = (token_low >> 32) as u32;
    msg.arg4 = (token_high & 0xFFFF_FFFF) as u32;
    let res = unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &mut msg as *mut CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?
    };
    let copy_len = core::cmp::min(out_buf.len(), res as usize);
    out_buf[..copy_len].copy_from_slice(&msg.payload[..copy_len]);
    Ok(copy_len)
}

pub fn set_focus(surface_id: u32) -> Result<(), SyscallError> {
    let msg = CompositorMsg::new(CompositorOp::SetFocus, surface_id);
    unsafe {
        raw_syscall(
            syscall::SYS_IPC_SEND,
            RECEIVER_COMPOSITOR,
            &msg as *const CompositorMsg as u64,
            core::mem::size_of::<CompositorMsg>() as u64,
            0,
            0,
        )?;
    }
    Ok(())
}

// Common PS/2 Set 1 Scancodes
pub const KEY_ESC: u32 = 0x01;
pub const KEY_BACKSPACE: u32 = 0x0E;
pub const KEY_TAB: u32 = 0x0F;
pub const KEY_ENTER: u32 = 0x1C;
pub const KEY_SPACE: u32 = 0x39;
pub const KEY_F1: u32 = 0x3B;
pub const KEY_F2: u32 = 0x3C;
pub const KEY_F3: u32 = 0x3D;
pub const KEY_F4: u32 = 0x3E;
pub const KEY_F5: u32 = 0x3F;
pub const KEY_UP: u32 = 0x48;
pub const KEY_LEFT: u32 = 0x4B;
pub const KEY_RIGHT: u32 = 0x4D;
pub const KEY_DOWN: u32 = 0x50;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityRibbonState {
    /// Default for unrelated or creative/sandboxed tasks (e.g., editing a family photo in Canvas
    /// or browsing untrusted DOM in Web Render): blocks unsolicited cross-app data pushes while
    /// keeping standard Clipboard Copy/Paste (`Ctrl+C` / `Ctrl+V`) active.
    LocalClipboardOnly = 0,
    /// Semantically related artifact detected (e.g., q3_budget.xlsx & document.pdf):
    /// stages a non-destructive offer in the chrome ribbon; never mutates canvas without user confirmation.
    StagedRibbonOffer = 1,
    /// User explicitly bound the two artifacts (Tab / Click on Ribbon):
    /// live auto-sync active with 1-click Unbind.
    BoundLiveSync = 2,
}

static mut SURFACE_RIBBON_STATES: [u8; 32] = [0; 32];

fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return false;
    }
    for i in 0..=(h.len() - n.len()) {
        let mut matched = true;
        for j in 0..n.len() {
            let hc = if h[i + j] >= b'A' && h[i + j] <= b'Z' {
                h[i + j] + 32
            } else {
                h[i + j]
            };
            let nc = if n[j] >= b'A' && n[j] <= b'Z' {
                n[j] + 32
            } else {
                n[j]
            };
            if hc != nc {
                matched = false;
                break;
            }
        }
        if matched {
            return true;
        }
    }
    false
}

/// Infers the default `ContinuityRibbonState` and specialist LoRA profile for a surface title.
/// Creative/photo surfaces (`Canvas`) and untrusted renderers (`Web Browser`) default to
/// `LocalClipboardOnly` so external grid/numeric values are never pushed into them unsolicited.
pub fn infer_surface_continuity_profile(
    title: &str,
    override_state: Option<ContinuityRibbonState>,
) -> (ContinuityRibbonState, &'static str, &'static str) {
    if contains_ascii_case_insensitive(title, "Canvas")
        || contains_ascii_case_insensitive(title, "Vector")
        || contains_ascii_case_insensitive(title, "Photo")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::LocalClipboardOnly);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_canvas",
                "Scoped Isolation: External Grid Push Blocked | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_canvas",
                "LINK AVAILABLE: vector_drawing.svg -> write_app | Tab/Click: Bind | Esc: Isolate",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_canvas",
                "BOUND: vector_drawing.svg <-> write_app (Vector Figure Sync) | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Grid")
        || contains_ascii_case_insensitive(title, "Spreadsheet")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_grid",
                "Local Spreadsheet Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_grid",
                "LINK AVAILABLE: grid_output.xlsx -> document.pdf (Photo Isolated) | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_grid",
                "BOUND: grid_output.xlsx <-> document.pdf & write_output.docx | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Write")
        || contains_ascii_case_insensitive(title, "Document")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_write",
                "Local Document Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_write",
                "LINK AVAILABLE: write_output.docx <-> grid_output.xlsx | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_write",
                "BOUND: write_output.docx <-> grid_output.xlsx & document.pdf | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "PDF")
        || contains_ascii_case_insensitive(title, "Reader")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_pdf",
                "Local PDF Reader Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_pdf",
                "LINK AVAILABLE: document.pdf <-> grid_output.xlsx & write_app | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_pdf",
                "BOUND: document.pdf <-> grid_output.xlsx (Live Table Digest) | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Plan")
        || contains_ascii_case_insensitive(title, "Calendar")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_plan",
                "Local Calendar Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_plan",
                "LINK AVAILABLE: calendar.ics <-> inbox.eml & tasks.txt | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_plan",
                "BOUND: calendar.ics <-> inbox.eml (Live Milestone Schedule) | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Mail")
        || contains_ascii_case_insensitive(title, "MIME")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_mail",
                "Local Mail Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_mail",
                "LINK AVAILABLE: attachment.csv -> grid_app & calendar.ics | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_mail",
                "BOUND: inbox.eml <-> grid_app (metrics.csv) & plan_app | Tab/Click: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Notes")
        || contains_ascii_case_insensitive(title, "Text Studio")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_notes",
                "Local Notes Mode (Cross-App Sync Paused) | F4/Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_notes",
                "LINK AVAILABLE: notes_output.txt <-> calc_app (REQ:1001) | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_notes",
                "BOUND: notes_output.txt <-> calc_app (Live Eval Receipt) | Tab/Click: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Calculator")
        || contains_ascii_case_insensitive(title, "Precision")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_calc",
                "Local Calculator Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_calc",
                "LINK AVAILABLE: calc_eval.txt <-> notes_app & grid_app | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_calc",
                "BOUND: calc_eval.txt <-> notes_app (Correlated IPC 200) | Tab/Click: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Archive") {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_zip",
                "Local Archive Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_zip",
                "LINK AVAILABLE: multi_test.zip <-> files_app (CRC32 Verified) | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_zip",
                "BOUND: multi_test.zip <-> files_app (Live Bundle Sync) | Tab/Click: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Files")
        || contains_ascii_case_insensitive(title, "Explorer")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_files",
                "Local Explorer Mode (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_files",
                "LINK AVAILABLE: INOS-FS Workspace <-> NNC Associative Index | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_files",
                "BOUND: INOS-FS Workspace <-> NNC Associative Merkle Index | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "IntScript")
        || contains_ascii_case_insensitive(title, "Script")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_ints",
                "Local Script Sandbox (Cross-App Sync Paused) | Ctrl+C/V Ready | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_ints",
                "LINK AVAILABLE: main.ints -> grid_output.xlsx & sales_report.svg | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_ints",
                "BOUND: main.ints <-> Atomic Transaction Spool (LBA 24576) | Tab: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Web")
        || contains_ascii_case_insensitive(title, "Browser")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::LocalClipboardOnly);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_web",
                "Zero-Ambient Sandbox: Untrusted DOM Isolated from Local Files | Tab: Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_web",
                "LINK AVAILABLE: Verified DOM Citation -> notes_output.txt | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_web",
                "BOUND: Read-Only Citation Extract -> notes_output.txt | Tab/Click: Unbind",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Sentinel")
        || contains_ascii_case_insensitive(title, "Security")
    {
        let st = override_state.unwrap_or(ContinuityRibbonState::BoundLiveSync);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_sec",
                "Isolated Security Console (External Telemetry Paused) | Tab: Stage Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_sec",
                "LINK AVAILABLE: Export Security Audit Receipt -> document.pdf | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_sec",
                "BOUND: Tonic Gate + Capability Audit + Cross-App Domain Guard | Tab: Mode",
            ),
        }
    } else if contains_ascii_case_insensitive(title, "Flow") {
        let st = override_state.unwrap_or(ContinuityRibbonState::BoundLiveSync);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: lora_flow",
                "Local Pipeline Editor (Live Bus Dispatch Paused) | Tab: Stage Offer",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: lora_flow",
                "LINK AVAILABLE: Stage Cross-App DAG (Grid -> Write -> PDF -> Mail) | Tab: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: lora_flow",
                "BOUND: Multi-App DAG (Grid -> Write -> PDF -> Mail) Active | Tab: Unbind",
            ),
        }
    } else {
        let st = override_state.unwrap_or(ContinuityRibbonState::StagedRibbonOffer);
        match st {
            ContinuityRibbonState::LocalClipboardOnly => (
                st,
                "LOCAL: inos_ai",
                "Scoped Local Workspace (Cross-App Auto-Sync Paused) | Ctrl+C/V Ready",
            ),
            ContinuityRibbonState::StagedRibbonOffer => (
                st,
                "OFFER: inos_ai",
                "LINK AVAILABLE: Related Workspace Artifact Detected | Tab/Click: Bind",
            ),
            ContinuityRibbonState::BoundLiveSync => (
                st,
                "SYNC: inos_ai",
                "BOUND: Live Semantic Continuity Active | Tab/Click: Unbind",
            ),
        }
    }
}

pub fn get_surface_continuity_state(surface_id: u32, title: &str) -> ContinuityRibbonState {
    let slot = (surface_id as usize) & 31;
    let raw = unsafe { SURFACE_RIBBON_STATES[slot] };
    match raw {
        1 => ContinuityRibbonState::LocalClipboardOnly,
        2 => ContinuityRibbonState::StagedRibbonOffer,
        3 => ContinuityRibbonState::BoundLiveSync,
        _ => {
            let (default_st, _, _) = infer_surface_continuity_profile(title, None);
            set_surface_continuity_state(surface_id, default_st);
            default_st
        }
    }
}

pub fn set_surface_continuity_state(surface_id: u32, state: ContinuityRibbonState) {
    let slot = (surface_id as usize) & 31;
    let encoded = match state {
        ContinuityRibbonState::LocalClipboardOnly => 1,
        ContinuityRibbonState::StagedRibbonOffer => 2,
        ContinuityRibbonState::BoundLiveSync => 3,
    };
    unsafe {
        SURFACE_RIBBON_STATES[slot] = encoded;
    }
}

/// Cycles the 3-State Scoped Continuity Ribbon upon user `Tab` or Ribbon click:
/// `StagedRibbonOffer -> BoundLiveSync -> LocalClipboardOnly -> StagedRibbonOffer`,
/// and submits a structured `CandidateProposal` to the kernel intent engine.
pub fn cycle_surface_continuity_state(surface_id: u32, title: &str) -> ContinuityRibbonState {
    let current = get_surface_continuity_state(surface_id, title);
    let next = match current {
        ContinuityRibbonState::StagedRibbonOffer => ContinuityRibbonState::BoundLiveSync,
        ContinuityRibbonState::BoundLiveSync => ContinuityRibbonState::LocalClipboardOnly,
        ContinuityRibbonState::LocalClipboardOnly => ContinuityRibbonState::StagedRibbonOffer,
    };
    set_surface_continuity_state(surface_id, next);

    let (op_type, op_str) = match next {
        ContinuityRibbonState::BoundLiveSync => (syscall::OP_TYPE_COPY, "continuity.bind"),
        ContinuityRibbonState::LocalClipboardOnly => (syscall::OP_TYPE_UNKNOWN, "continuity.iso"),
        ContinuityRibbonState::StagedRibbonOffer => (syscall::OP_TYPE_ARCHIVE, "continuity.offr"),
    };
    let (_, badge, detail) = infer_surface_continuity_profile(title, Some(next));
    let proposal = syscall::CandidateProposal::new_intent(op_type, op_str, title, badge, detail);
    let _ = crate::submit_intent_proposal(&proposal);
    next
}

/// Renders a scoped Chrome Continuity Ribbon directly onto any raw `surface_id`.
pub fn draw_surface_continuity_ribbon(
    surface_id: u32,
    width: u32,
    height: u32,
    state: ContinuityRibbonState,
    badge: &str,
    detail: &str,
) {
    let y = height.saturating_sub(46);
    let (pill_bg, pill_fg, detail_fg) = match state {
        ContinuityRibbonState::LocalClipboardOnly => (0x1E293B, 0x94A3B8, 0x64748B),
        ContinuityRibbonState::StagedRibbonOffer => (0x78350F, 0xFDE68A, 0xFCD34D),
        ContinuityRibbonState::BoundLiveSync => (0x064E3B, 0x6EE7B7, 0xA7F3D0),
    };
    let _ = fill_rect(surface_id, 0, y, width, 22, 0x0B1325);
    let _ = fill_rect(surface_id, 8, y + 3, 136, 16, pill_bg);
    let _ = draw_text(surface_id, 14, y + 4, pill_fg, badge);
    let _ = draw_text(surface_id, 152, y + 4, detail_fg, detail);
}

pub const OP_INSPECT: u16 = 7;
pub const OP_STAGE_RIBBON: u16 = 8;
pub const OP_BIND_RIBBON: u16 = 9;

pub struct AppShell {
    pub surface_id: u32,
    pub width: u32,
    pub height: u32,
    pub title: &'static str,
    pub dirty_min_x: u32,
    pub dirty_min_y: u32,
    pub dirty_max_x: u32,
    pub dirty_max_y: u32,
    pub dirty_ops: u32,
}

impl AppShell {
    pub fn new(width: u32, height: u32, title: &'static str) -> Result<Self, SyscallError> {
        let surface_id = create_surface(width, height, title)?;
        let _ = set_focus(surface_id);
        let (default_state, _, _) = infer_surface_continuity_profile(title, None);
        set_surface_continuity_state(surface_id, default_state);
        Ok(Self {
            surface_id,
            width,
            height,
            title,
            dirty_min_x: 0,
            dirty_min_y: 0,
            dirty_max_x: width,
            dirty_max_y: height,
            dirty_ops: 0,
        })
    }

    pub fn mark_dirty_rect(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let x2 = x.saturating_add(w).min(self.width);
        let y2 = y.saturating_add(h).min(self.height);
        if self.dirty_ops == 0 {
            self.dirty_min_x = x;
            self.dirty_min_y = y;
            self.dirty_max_x = x2;
            self.dirty_max_y = y2;
        } else {
            if x < self.dirty_min_x {
                self.dirty_min_x = x;
            }
            if y < self.dirty_min_y {
                self.dirty_min_y = y;
            }
            if x2 > self.dirty_max_x {
                self.dirty_max_x = x2;
            }
            if y2 > self.dirty_max_y {
                self.dirty_max_y = y2;
            }
        }
        self.dirty_ops = self.dirty_ops.saturating_add(1);
    }

    pub fn reset_dirty_rect(&mut self) {
        self.dirty_min_x = self.width;
        self.dirty_min_y = self.height;
        self.dirty_max_x = 0;
        self.dirty_max_y = 0;
        self.dirty_ops = 0;
    }

    pub fn continuity_state(&self) -> ContinuityRibbonState {
        get_surface_continuity_state(self.surface_id, self.title)
    }

    pub fn set_continuity_state(&self, state: ContinuityRibbonState) {
        set_surface_continuity_state(self.surface_id, state);
    }

    pub fn current_continuity_profile(&self) -> (ContinuityRibbonState, &'static str, &'static str) {
        let st = self.continuity_state();
        infer_surface_continuity_profile(self.title, Some(st))
    }

    pub fn clear(&self, bg_color: u32) {
        let _ = fill_rect(self.surface_id, 0, 28, self.width, self.height.saturating_sub(28), bg_color);
    }

    pub fn draw_button(&self, x: u32, y: u32, w: u32, h: u32, label: &str, active: bool) {
        let bg = if active { 0x3B82F6 } else { 0x26354D };
        let fg = if active { 0xFFFFFF } else { 0xCBD5E1 };
        let _ = fill_rect(self.surface_id, x, y, w, h, bg);
        let text_x = x + 8;
        let text_y = y + (h.saturating_sub(14)) / 2;
        let _ = draw_text(self.surface_id, text_x, text_y, fg, label);
    }

    pub fn draw_status_bar(&self, status: &str) {
        let (state, badge, detail) = self.current_continuity_profile();
        self.draw_scoped_continuity_ribbon(state, badge, detail);
        let y = self.height.saturating_sub(24);
        let _ = fill_rect(self.surface_id, 0, y, self.width, 24, 0x0F172A);
        let _ = draw_text(self.surface_id, 10, y + 4, 0x94A3B8, status);
    }

    /// Fast-path dirty-rectangle repaint when an application receives a background continuity
    /// ribbon offer (`OP_STAGE_RIBBON` / `OP_BIND_RIBBON`) without full-window layout changes.
    /// Repaints only the 22px top Chrome Continuity Ribbon (`y = 0..22`) and 24px bottom
    /// status bar (`y = height-24..height`) and calls `present_surface`.
    pub fn redraw_ribbon_and_status_only(&mut self, status_msg: &str) -> Result<(), SyscallError> {
        let (state, badge, detail) = self.current_continuity_profile();
        let (pill_bg, pill_fg, detail_fg) = match state {
            ContinuityRibbonState::LocalClipboardOnly => (0x1E293B, 0x94A3B8, 0x64748B),
            ContinuityRibbonState::StagedRibbonOffer => (0x78350F, 0xFDE68A, 0xFCD34D),
            ContinuityRibbonState::BoundLiveSync => (0x064E3B, 0x6EE7B7, 0xA7F3D0),
        };
        let _ = fill_rect(self.surface_id, 0, 0, self.width, 22, 0x0B1325);
        let _ = fill_rect(self.surface_id, 8, 3, 136, 16, pill_bg);
        let _ = draw_text(self.surface_id, 14, 4, pill_fg, badge);
        let _ = draw_text(self.surface_id, 152, 4, detail_fg, detail);

        self.draw_scoped_continuity_ribbon(state, badge, detail);
        let status_y = self.height.saturating_sub(24);
        let _ = fill_rect(self.surface_id, 0, status_y, self.width, 24, 0x0F172A);
        let _ = draw_text(self.surface_id, 10, status_y + 4, 0x94A3B8, status_msg);

        self.mark_dirty_rect(0, 0, self.width, 22);
        self.mark_dirty_rect(0, self.height.saturating_sub(46), self.width, 46);

        present_surface(self.surface_id)
    }

    /// Stages a non-destructive offer in the Chrome Continuity Ribbon
    pub fn stage_ribbon_offer(&self, badge: &str, detail: &str) {
        self.draw_scoped_continuity_ribbon(
            ContinuityRibbonState::StagedRibbonOffer,
            badge,
            detail,
        );
    }

    /// Renders a scoped Chrome Continuity Ribbon according to `ContinuityRibbonState`:
    /// - `LocalClipboardOnly`: calm neutral badge (no cross-app noise on unrelated work)
    /// - `StagedRibbonOffer`: amber staged offer (requires user Tab/Click to bind; zero silent overwrite)
    /// - `BoundLiveSync`: emerald active link (with 1-click Unbind)
    pub fn draw_scoped_continuity_ribbon(
        &self,
        state: ContinuityRibbonState,
        badge: &str,
        detail: &str,
    ) {
        set_surface_continuity_state(self.surface_id, state);
        draw_surface_continuity_ribbon(self.surface_id, self.width, self.height, state, badge, detail);
    }

    /// Renders a non-intrusive Ambient Intelligence & Cross-App Continuity ribbon
    /// directly in the surface chrome (zero chat bubbles; live workspace grounding)
    pub fn draw_ambient_continuity_bar(&self, specialist_badge: &str, continuity_link: &str) {
        self.draw_scoped_continuity_ribbon(
            ContinuityRibbonState::BoundLiveSync,
            specialist_badge,
            continuity_link,
        );
    }

    /// Standard user-controlled Clipboard Copy (`Ctrl+C`), protected by capability token
    pub fn copy_to_clipboard(&self, cap_token: u64, data: &[u8]) -> Result<usize, SyscallError> {
        set_clipboard(self.surface_id, cap_token, data)
    }

    /// Standard user-controlled Clipboard Paste (`Ctrl+V`), protected by capability token
    pub fn paste_from_clipboard(&self, cap_token: u64, out_buf: &mut [u8]) -> Result<usize, SyscallError> {
        get_clipboard(self.surface_id, cap_token, out_buf)
    }

    /// Non-blockingly polls the OS-wide Ambient Intelligence & Automation bus (`0xAA01`)
    pub fn poll_ambient_directive(&self) -> Option<(u64, u32, syscall::AutomationIpcWire)> {
        if self.continuity_state() != ContinuityRibbonState::BoundLiveSync {
            return None;
        }
        crate::recv_automation_ipc().ok()
    }

    /// Publishes a live semantic state update from this surface onto the OS-wide
    /// `AutomationIpcWire` bus when the surface is in `BoundLiveSync` mode.
    pub fn publish_ambient_state(
        &self,
        target_pid: u64,
        request_id: u32,
        op_code: u16,
        cell_ref: &str,
        value_num: i64,
        formula_or_schema: &str,
        target_artifact: &str,
    ) -> Result<u64, SyscallError> {
        if self.continuity_state() != ContinuityRibbonState::BoundLiveSync {
            return Err(SyscallError::PermissionDenied);
        }
        let mut wire = syscall::AutomationIpcWire::empty();
        wire.request_id = request_id;
        wire.op_code = op_code;
        wire.value_num = value_num;

        let c_bytes = cell_ref.as_bytes();
        let c_len = core::cmp::min(c_bytes.len(), 8);
        wire.cell[..c_len].copy_from_slice(&c_bytes[..c_len]);

        let f_bytes = formula_or_schema.as_bytes();
        let f_len = core::cmp::min(f_bytes.len(), 32);
        wire.formula[..f_len].copy_from_slice(&f_bytes[..f_len]);

        let t_bytes = target_artifact.as_bytes();
        let t_len = core::cmp::min(t_bytes.len(), 32);
        wire.target_file[..t_len].copy_from_slice(&t_bytes[..t_len]);

        crate::send_automation_ipc(target_pid, op_code as u32, &wire)
    }

    pub fn present(&self) {
        let _ = present_surface(self.surface_id);
    }

    fn is_ribbon_click(&self, y: u32) -> bool {
        let ribbon_top = self.height.saturating_sub(46);
        let ribbon_bottom = self.height.saturating_sub(24);
        y >= ribbon_top && y < ribbon_bottom
    }

    pub fn run_event_loop<F>(&mut self, mut on_tick: F)
    where
        F: FnMut(&mut Self, Option<InputEvent>) -> bool,
    {
        if !on_tick(self, None) {
            self.present();
            return;
        }
        self.present();

        loop {
            let (raw_opt, interactive) = poll_input_event_ext(self.surface_id);
            if let Some(raw) = raw_opt {
                if raw.event_type == InputEventType::KeyDown && raw.arg2 == KEY_ESC {
                    break;
                }
                if raw.event_type == InputEventType::MouseDown {
                    if raw.arg2 <= 28 && raw.arg1 >= self.width.saturating_sub(32) {
                        break;
                    }
                }
                // Scoped Chrome Continuity Ribbon toggle via Tab or direct Ribbon click
                if raw.event_type == InputEventType::MouseDown && self.is_ribbon_click(raw.arg2) {
                    let _ = cycle_surface_continuity_state(self.surface_id, self.title);
                    let keep_going = on_tick(self, None);
                    let _ = self.present();
                    if !keep_going {
                        break;
                    }
                    continue;
                }
                if raw.event_type == InputEventType::KeyDown && raw.arg2 == KEY_TAB {
                    let _ = cycle_surface_continuity_state(self.surface_id, self.title);
                }
                let mapped = match raw.event_type {
                    InputEventType::KeyDown => Some(InputEvent::KeyPress {
                        scancode: raw.arg2,
                        ascii: (raw.arg1 & 0xFF) as u8,
                    }),
                    InputEventType::MouseDown => Some(InputEvent::MouseEvent {
                        x: raw.arg1 as i32,
                        y: raw.arg2 as i32,
                        buttons: 1,
                    }),
                    InputEventType::MouseMove => Some(InputEvent::MouseEvent {
                        x: raw.arg1 as i32,
                        y: raw.arg2 as i32,
                        buttons: raw.arg3,
                    }),
                    _ => None,
                };
                if let Some(ev) = mapped {
                    let keep_going = on_tick(self, Some(ev));
                    let _ = self.present();
                    if !keep_going {
                        break;
                    }
                }
            } else if !interactive {
                break;
            } else {
                core::hint::spin_loop();
            }
        }
    }

    pub fn run_split_loop<F>(&self, mut on_tick: F)
    where
        F: FnMut(&Self, Option<RawInputEvent>) -> bool,
    {
        let should_quit = on_tick(self, None);
        let _ = self.present();
        if should_quit {
            return;
        }

        loop {
            let (ev_opt, interactive) = poll_input_event_ext(self.surface_id);
            if let Some(ev) = ev_opt {
                if ev.event_type == InputEventType::KeyDown && ev.arg2 == KEY_ESC {
                    break;
                }
                if ev.event_type == InputEventType::MouseDown {
                    if ev.arg2 <= 28 && ev.arg1 >= self.width.saturating_sub(32) {
                        break;
                    }
                }
                // Scoped Chrome Continuity Ribbon toggle via Tab or direct Ribbon click
                if ev.event_type == InputEventType::MouseDown && self.is_ribbon_click(ev.arg2) {
                    let _ = cycle_surface_continuity_state(self.surface_id, self.title);
                    let should_quit = on_tick(self, None);
                    let _ = self.present();
                    if should_quit {
                        break;
                    }
                    continue;
                }
                if ev.event_type == InputEventType::KeyDown && ev.arg2 == KEY_TAB {
                    let _ = cycle_surface_continuity_state(self.surface_id, self.title);
                }
                let should_quit = on_tick(self, Some(ev));
                let _ = self.present();
                if should_quit {
                    break;
                }
            } else if !interactive {
                break;
            } else {
                core::hint::spin_loop();
            }
        }
    }
}

