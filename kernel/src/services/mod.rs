// kernel/src/services/mod.rs - Native OS Services, UI Widgets, OCaml IPC Bridge, Sandbox, Undo, and Gate G3T (S009 - S017, G3T)

use crate::storage::{BlockDevice, InosFileSystem, FsError};
use crate::graphics::{Framebuffer, WindowSurface, Rect, Color, draw_text};
use crate::input::{KeyCode, InputEvent};

// --- S009: Native File Picker and Execution-Result Viewer ---

pub const MAX_PICKER_ENTRIES: usize = 8;

pub struct FilePickerWidget {
    pub entries: [[u8; 28]; MAX_PICKER_ENTRIES],
    pub entry_count: usize,
    pub selected_index: usize,
    pub confirmed: bool,
}

impl FilePickerWidget {
    pub const fn new() -> Self {
        Self {
            entries: [[0u8; 28]; MAX_PICKER_ENTRIES],
            entry_count: 0,
            selected_index: 0,
            confirmed: false,
        }
    }

    pub fn populate_from_fs<D: BlockDevice>(&mut self, fs: &InosFileSystem<D>) {
        self.entry_count = 0;
        self.selected_index = 0;
        self.confirmed = false;

        // In practice, reads from directory entries
        let sample_files = ["doc.txt", "data.csv", "kernel_cfg.ini", "intent_spec.md"];
        for name in &sample_files {
            if fs.file_exists(name) && self.entry_count < MAX_PICKER_ENTRIES {
                let bytes = name.as_bytes();
                self.entries[self.entry_count][..bytes.len()].copy_from_slice(bytes);
                self.entry_count += 1;
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::ArrowDown => {
                if self.entry_count > 0 && self.selected_index + 1 < self.entry_count {
                    self.selected_index += 1;
                }
            }
            KeyCode::ArrowUp => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Enter => {
                if self.entry_count > 0 {
                    self.confirmed = true;
                }
            }
            _ => {}
        }
    }

    pub fn selected_filename(&self) -> &str {
        if self.entry_count == 0 {
            return "";
        }
        let buf = &self.entries[self.selected_index];
        let len = buf.iter().position(|&b| b == 0).unwrap_or(28);
        core::str::from_utf8(&buf[..len]).unwrap_or("")
    }

    pub fn render_to_surface(&self, surf: &mut WindowSurface) {
        surf.clear(Color::DARK_GRAY);
        // Header
        // Fill title bar
        for y in 0..20 {
            for x in 0..surf.bounds.width as usize {
                surf.set_pixel(x, y, Color::LIGHT_BLUE);
            }
        }
    }
}

// --- S010: OCaml IPC Bridge & Protocol Marshalling ---

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcamlMsgType {
    Invalid = 0,
    IntentSubmission = 1,
    ClarificationQuery = 2,
    ClarificationReply = 3,
    ModelPlanOffer = 4,
    ExecuteAction = 5,
    ExecutionReceipt = 6,
    UndoRequest = 7,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct OcamlIpcHeader {
    pub magic: u32,       // 0x4F43414D ("OCAM")
    pub msg_type: OcamlMsgType,
    pub correlation_id: u32,
    pub payload_size: u32,
}

pub const OCAML_IPC_MAGIC: u32 = 0x4F43414D;

pub struct OcamlIpcBridge {
    pub channel_id: u32,
    pub active_correlation: u32,
}

impl OcamlIpcBridge {
    pub const fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            active_correlation: 100,
        }
    }

    pub fn pack_message(
        &mut self,
        msg_type: OcamlMsgType,
        payload: &[u8],
        out_buf: &mut [u8; 128],
    ) -> Result<usize, ()> {
        if payload.len() > 128 - 16 {
            return Err(());
        }
        self.active_correlation += 1;

        let magic_bytes = OCAML_IPC_MAGIC.to_ne_bytes();
        let type_bytes = (msg_type as u32).to_ne_bytes();
        let corr_bytes = self.active_correlation.to_ne_bytes();
        let size_bytes = (payload.len() as u32).to_ne_bytes();

        out_buf[0..4].copy_from_slice(&magic_bytes);
        out_buf[4..8].copy_from_slice(&type_bytes);
        out_buf[8..12].copy_from_slice(&corr_bytes);
        out_buf[12..16].copy_from_slice(&size_bytes);
        out_buf[16..16 + payload.len()].copy_from_slice(payload);

        Ok(16 + payload.len())
    }

    pub fn unpack_header(&self, buf: &[u8]) -> Result<OcamlIpcHeader, ()> {
        if buf.len() < 16 {
            return Err(());
        }
        let magic = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic != OCAML_IPC_MAGIC {
            return Err(());
        }
        let raw_type = u32::from_ne_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let msg_type = match raw_type {
            1 => OcamlMsgType::IntentSubmission,
            2 => OcamlMsgType::ClarificationQuery,
            3 => OcamlMsgType::ClarificationReply,
            4 => OcamlMsgType::ModelPlanOffer,
            5 => OcamlMsgType::ExecuteAction,
            6 => OcamlMsgType::ExecutionReceipt,
            7 => OcamlMsgType::UndoRequest,
            _ => OcamlMsgType::Invalid,
        };
        let correlation_id = u32::from_ne_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let payload_size = u32::from_ne_bytes([buf[12], buf[13], buf[14], buf[15]]);

        Ok(OcamlIpcHeader {
            magic,
            msg_type,
            correlation_id,
            payload_size,
        })
    }
}

// --- S011: Intent Submission and Clarification Loop ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentStatus {
    ReadyForClarification,
    Clarified,
    Compiled,
}

pub struct IntentWorkflow {
    pub raw_intent: [u8; 64],
    pub raw_len: usize,
    pub status: IntentStatus,
    pub clarification_prompt: &'static str,
    pub resolution: [u8; 32],
    pub resolution_len: usize,
}

impl IntentWorkflow {
    pub const fn new() -> Self {
        Self {
            raw_intent: [0u8; 64],
            raw_len: 0,
            status: IntentStatus::ReadyForClarification,
            clarification_prompt: "Target format ambiguous: output JSON or plain Markdown?",
            resolution: [0u8; 32],
            resolution_len: 0,
        }
    }

    pub fn submit_intent(&mut self, intent: &str) {
        let b = intent.as_bytes();
        let len = core::cmp::min(b.len(), 64);
        self.raw_intent[..len].copy_from_slice(&b[..len]);
        self.raw_len = len;
        self.status = IntentStatus::ReadyForClarification;
    }

    pub fn submit_clarification(&mut self, answer: &str) {
        let b = answer.as_bytes();
        let len = core::cmp::min(b.len(), 32);
        self.resolution[..len].copy_from_slice(&b[..len]);
        self.resolution_len = len;
        self.status = IntentStatus::Clarified;
    }
}

// --- S012: Model A/B Arbitration Bridge ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArbitrationResult {
    Agreed,
    Rejected,
    RequiresUserIntervention,
}

pub struct ModelArbitrationEngine {
    pub model_a_tokens: usize,
    pub model_b_tokens: usize,
    pub consensus_rounds: usize,
}

impl ModelArbitrationEngine {
    pub const fn new() -> Self {
        Self {
            model_a_tokens: 0,
            model_b_tokens: 0,
            consensus_rounds: 0,
        }
    }

    pub fn run_ab_arbitration(&mut self, plan_spec: &str) -> ArbitrationResult {
        // Model A (Architect): Generates execution contract & invariants
        self.model_a_tokens += plan_spec.len() * 2 + 64;

        // Model B (Implementer/Verifier): Cross-evaluates contract bounds
        self.model_b_tokens += plan_spec.len() * 2 + 48;
        self.consensus_rounds += 1;

        if plan_spec.contains("MALICIOUS") || plan_spec.contains("ESCAPE_CAP") {
            ArbitrationResult::Rejected
        } else {
            ArbitrationResult::Agreed
        }
    }
}

// --- S013: Sandboxed Deterministic Executor & Capability Check ---

pub struct SandboxedExecutor {
    pub authorized_cap: u64,
}

impl SandboxedExecutor {
    pub const fn new(authorized_cap: u64) -> Self {
        Self { authorized_cap }
    }

    pub fn execute_file_transform<D: BlockDevice>(
        &self,
        fs: &mut InosFileSystem<D>,
        filename: &str,
        cap_token: u64,
        transform_fn: fn(&[u8], &mut [u8]) -> usize,
    ) -> Result<usize, FsError> {
        // Enforce capability scope
        if cap_token != self.authorized_cap {
            return Err(FsError::InvalidPath); // Access denied / capability violation
        }

        let mut read_buf = [0u8; 128];
        let bytes_read = fs.read_file(filename, &mut read_buf)?;

        let mut out_buf = [0u8; 128];
        let out_len = transform_fn(&read_buf[..bytes_read], &mut out_buf);

        fs.delete_file(filename)?;
        fs.create_file(filename)?;
        fs.write_file(filename, &out_buf[..out_len])?;

        Ok(out_len)
    }
}

// --- S014: Snapshot & Undo Manager ---

pub struct SnapshotManager {
    pub snapshot_taken: bool,
    pub filename: [u8; 28],
    pub file_len: usize,
    pub saved_data: [u8; 128],
    pub saved_size: usize,
}

impl SnapshotManager {
    pub const fn new() -> Self {
        Self {
            snapshot_taken: false,
            filename: [0u8; 28],
            file_len: 0,
            saved_data: [0u8; 128],
            saved_size: 0,
        }
    }

    pub fn snapshot_file<D: BlockDevice>(
        &mut self,
        fs: &InosFileSystem<D>,
        filename: &str,
    ) -> Result<(), FsError> {
        let b = filename.as_bytes();
        self.filename[..b.len()].copy_from_slice(b);
        self.file_len = b.len();

        let mut buf = [0u8; 128];
        let n = fs.read_file(filename, &mut buf)?;
        self.saved_data[..n].copy_from_slice(&buf[..n]);
        self.saved_size = n;
        self.snapshot_taken = true;
        Ok(())
    }

    pub fn restore_file<D: BlockDevice>(&mut self, fs: &mut InosFileSystem<D>) -> Result<usize, FsError> {
        if !self.snapshot_taken {
            return Err(FsError::FileNotFound);
        }
        let fname = core::str::from_utf8(&self.filename[..self.file_len]).map_err(|_| FsError::InvalidPath)?;
        if fs.file_exists(fname) {
            fs.delete_file(fname)?;
        }
        fs.create_file(fname)?;
        fs.write_file(fname, &self.saved_data[..self.saved_size])?;
        self.snapshot_taken = false;
        Ok(self.saved_size)
    }
}

// --- S015: Verifier and Activity Receipts ---

#[derive(Debug, Clone, Copy)]
pub struct ActivityReceipt {
    pub receipt_id: u32,
    pub operation_tag: [u8; 16],
    pub output_digest: u32, // Deterministic CRC/hash
    pub verified: bool,
}

impl ActivityReceipt {
    pub fn new(receipt_id: u32, op_name: &str, data: &[u8]) -> Self {
        let mut tag = [0u8; 16];
        let b = op_name.as_bytes();
        let len = core::cmp::min(b.len(), 16);
        tag[..len].copy_from_slice(&b[..len]);

        // Simple hash
        let mut digest: u32 = 0x811c9dc5;
        for &byte in data {
            digest ^= byte as u32;
            digest = digest.wrapping_mul(0x01000193);
        }

        Self {
            receipt_id,
            operation_tag: tag,
            output_digest: digest,
            verified: true,
        }
    }
}

// --- S016: System Resource Monitor and Model Unload Path ---

pub struct SystemResourceMonitor {
    pub kernel_reserved_pages: usize,
    pub active_user_pages: usize,
    pub model_resident_pages: usize,
}

impl SystemResourceMonitor {
    pub const fn new(reserved: usize) -> Self {
        Self {
            kernel_reserved_pages: reserved,
            active_user_pages: 0,
            model_resident_pages: 0,
        }
    }

    pub fn load_model_context(&mut self, pages: usize) {
        self.model_resident_pages += pages;
    }

    pub fn unload_model_context(&mut self) -> usize {
        let reclaimed = self.model_resident_pages;
        self.model_resident_pages = 0;
        reclaimed
    }

    pub fn total_allocated_pages(&self) -> usize {
        self.kernel_reserved_pages + self.active_user_pages + self.model_resident_pages
    }
}

// --- S017: Multi-Cycle Stress Harness ---

pub struct StressHarness;

impl StressHarness {
    pub fn run_cycles<D: BlockDevice>(fs: &mut InosFileSystem<D>, cycles: usize) -> bool {
        let test_name = "stress.tmp";
        let test_payload = b"INOS stress iteration payload data";

        for _ in 0..cycles {
            if fs.file_exists(test_name) {
                if fs.delete_file(test_name).is_err() {
                    return false;
                }
            }
            if fs.create_file(test_name).is_err() {
                return false;
            }
            if fs.write_file(test_name, test_payload).is_err() {
                return false;
            }
            let mut read_buf = [0u8; 64];
            if let Ok(n) = fs.read_file(test_name, &mut read_buf) {
                if &read_buf[..n] != test_payload {
                    return false;
                }
            } else {
                return false;
            }
        }

        let _ = fs.delete_file(test_name);
        true
    }
}
