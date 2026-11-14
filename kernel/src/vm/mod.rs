// kernel/src/vm/mod.rs - Virtualization Probe, Guest Capsule Manager, VirtIO, and Legacy Compatibility (Gate G4: V001 - G4T)

use crate::storage::{BlockDevice, BlockError, BLOCK_SIZE};
use crate::graphics::{Rect, Color, WindowSurface};
use crate::input::InputEvent;

// --- V001 & V002: Host CPU Virtualization Probe & Safe Fallback ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuVirtProfile {
    IntelVmxSupported,
    AmdSvmSupported,
    HypervisorGuestPresent,
    UnsupportedSoftwareFallback,
}

pub struct VirtualizationProbe;

impl VirtualizationProbe {
    /// Safe probe for CPU virtualization features via CPUID or simulated detection
    pub fn probe_host() -> (CpuVirtProfile, bool) {
        #[cfg(target_arch = "x86_64")]
        {
            // Execute CPUID leaf 1 safely
            let ecx: u32;
            unsafe {
                core::arch::asm!(
                    "push rbx",
                    "mov eax, 1",
                    "cpuid",
                    "pop rbx",
                    lateout("ecx") ecx,
                    out("eax") _,
                    out("edx") _,
                );
            }

            let vmx_supported = (ecx & (1 << 5)) != 0;
            let hypervisor_present = (ecx & (1 << 31)) != 0;

            if hypervisor_present {
                (CpuVirtProfile::HypervisorGuestPresent, true)
            } else if vmx_supported {
                (CpuVirtProfile::IntelVmxSupported, true)
            } else {
                (CpuVirtProfile::UnsupportedSoftwareFallback, false)
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            (CpuVirtProfile::UnsupportedSoftwareFallback, false)
        }
    }
}

// --- V003: Guest Capsule Specification & Authority Scopes ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceGrant {
    VirtioConsole,
    VirtioBlock,
    VirtioNet,
    SharedDisplayBuffer,
    DirectPhysicalPci(u16), // Forbidden
}

pub struct CapsuleScope {
    pub allow_virtio_console: bool,
    pub allow_virtio_block: bool,
    pub allow_virtio_net: bool,
    pub allow_shared_display: bool,
    pub allow_clipboard_read: bool,
    pub allow_clipboard_write: bool,
}

impl CapsuleScope {
    pub const fn default_isolated() -> Self {
        Self {
            allow_virtio_console: true,
            allow_virtio_block: true,
            allow_virtio_net: false,
            allow_shared_display: true,
            allow_clipboard_read: false,  // Strict host clipboard isolation
            allow_clipboard_write: false,
        }
    }

    pub fn validate_device_request(&self, dev: DeviceGrant) -> bool {
        match dev {
            DeviceGrant::VirtioConsole => self.allow_virtio_console,
            DeviceGrant::VirtioBlock => self.allow_virtio_block,
            DeviceGrant::VirtioNet => self.allow_virtio_net,
            DeviceGrant::SharedDisplayBuffer => self.allow_shared_display,
            DeviceGrant::DirectPhysicalPci(_) => false, // Always rejected
        }
    }
}

// --- V004 & V011: Guest Resource Budgeting & Reclaim Contract ---

pub struct GuestResourceBudget {
    pub host_min_reserved_pages: usize,
    pub guest_allocated_pages: usize,
    pub guest_max_pages: usize,
}

impl GuestResourceBudget {
    pub const fn new(host_reserved: usize, max_guest: usize) -> Self {
        Self {
            host_min_reserved_pages: host_reserved,
            guest_allocated_pages: 0,
            guest_max_pages: max_guest,
        }
    }

    pub fn allocate_guest_pages(&mut self, pages: usize) -> Result<(), ()> {
        if self.guest_allocated_pages + pages > self.guest_max_pages {
            return Err(());
        }
        self.guest_allocated_pages += pages;
        Ok(())
    }

    pub fn reclaim_all_guest_pages(&mut self) -> usize {
        let reclaimed = self.guest_allocated_pages;
        self.guest_allocated_pages = 0;
        reclaimed
    }
}

// --- V005: Test Hypervisor & Ephemeral Guest Execution ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuestState {
    Uninitialized,
    Running(u32),
    Stopped(i32),
    Reclaimed,
}

pub struct TestHypervisor {
    pub guest_id: u32,
    pub state: GuestState,
    pub budget: GuestResourceBudget,
}

impl TestHypervisor {
    pub const fn new(guest_id: u32, host_reserved_pages: usize, max_pages: usize) -> Self {
        Self {
            guest_id,
            state: GuestState::Uninitialized,
            budget: GuestResourceBudget::new(host_reserved_pages, max_pages),
        }
    }

    pub fn launch(&mut self, pages: usize) -> Result<(), ()> {
        self.budget.allocate_guest_pages(pages)?;
        self.state = GuestState::Running(self.guest_id);
        Ok(())
    }

    pub fn stop(&mut self, exit_code: i32) -> Result<(), ()> {
        match self.state {
            GuestState::Running(_) => {
                self.state = GuestState::Stopped(exit_code);
                Ok(())
            }
            _ => Err(()),
        }
    }

    pub fn teardown_and_reclaim(&mut self) -> usize {
        let reclaimed = self.budget.reclaim_all_guest_pages();
        self.state = GuestState::Reclaimed;
        reclaimed
    }
}

// --- V006: VirtIO Abstractions (Console, Storage, Network) ---

pub struct VirtioAbstractions {
    pub console_buf: [u8; 128],
    pub console_len: usize,
    pub packets_transmitted: usize,
}

impl VirtioAbstractions {
    pub const fn new() -> Self {
        Self {
            console_buf: [0u8; 128],
            console_len: 0,
            packets_transmitted: 0,
        }
    }

    pub fn console_write(&mut self, msg: &[u8]) {
        let len = core::cmp::min(msg.len(), 128);
        self.console_buf[..len].copy_from_slice(&msg[..len]);
        self.console_len = len;
    }

    pub fn net_send_packet(&mut self, len: usize) {
        let _ = len;
        self.packets_transmitted += 1;
    }
}

// --- V007: Disposable Guest Disk Overlays ---

pub const OVERLAY_MAX_SECTORS: usize = 32;

pub struct DisposableOverlayDisk<B: BlockDevice> {
    pub base_device: B,
    pub overlay_lbas: [u64; OVERLAY_MAX_SECTORS],
    pub overlay_data: [[u8; BLOCK_SIZE]; OVERLAY_MAX_SECTORS],
    pub overlay_count: usize,
}

impl<B: BlockDevice> DisposableOverlayDisk<B> {
    pub const fn new(base_device: B) -> Self {
        Self {
            base_device,
            overlay_lbas: [0; OVERLAY_MAX_SECTORS],
            overlay_data: [[0u8; BLOCK_SIZE]; OVERLAY_MAX_SECTORS],
            overlay_count: 0,
        }
    }

    pub fn discard_overlay(&mut self) {
        self.overlay_count = 0;
    }
}

impl<B: BlockDevice> BlockDevice for DisposableOverlayDisk<B> {
    fn read_block(&self, lba: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        // Check overlay first
        for i in 0..self.overlay_count {
            if self.overlay_lbas[i] == lba {
                buf.copy_from_slice(&self.overlay_data[i]);
                return Ok(());
            }
        }
        // Fall back to base
        self.base_device.read_block(lba, buf)
    }

    fn write_block(&mut self, lba: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        // Writes always go to ephemeral overlay; base disk never touched
        for i in 0..self.overlay_count {
            if self.overlay_lbas[i] == lba {
                self.overlay_data[i].copy_from_slice(buf);
                return Ok(());
            }
        }
        if self.overlay_count < OVERLAY_MAX_SECTORS {
            let idx = self.overlay_count;
            self.overlay_lbas[idx] = lba;
            self.overlay_data[idx].copy_from_slice(buf);
            self.overlay_count += 1;
            Ok(())
        } else {
            Err(BlockError::IoError)
        }
    }

    fn block_count(&self) -> u64 {
        self.base_device.block_count()
    }
}

// --- V008: Shared Display Buffer & Compositor Integration ---

pub struct GuestDisplayAdapter;

impl GuestDisplayAdapter {
    pub fn render_guest_window(
        surf: &mut WindowSurface,
        title: &str,
        content_color: Color,
    ) {
        surf.clear(content_color);
        // Draw title bar
        for y in 0..12 {
            for x in 0..surf.bounds.width as usize {
                surf.set_pixel(x, y, Color::DARK_GRAY);
            }
        }
        let _ = title;
    }
}

// --- V009: Keyboard/Pointer Forwarding & Clipboard Isolation ---

pub struct GuestInputForwarder {
    pub events_forwarded: usize,
}

impl GuestInputForwarder {
    pub const fn new() -> Self {
        Self { events_forwarded: 0 }
    }

    pub fn forward_event(&mut self, ev: InputEvent, scope: &CapsuleScope) -> Result<(), ()> {
        match ev {
            InputEvent::Keyboard { .. } => {
                self.events_forwarded += 1;
                Ok(())
            }
            InputEvent::Pointer { .. } => {
                self.events_forwarded += 1;
                Ok(())
            }
        }
    }

    pub fn transfer_clipboard(&self, scope: &CapsuleScope) -> Result<(), ()> {
        if scope.allow_clipboard_read || scope.allow_clipboard_write {
            Ok(())
        } else {
            Err(()) // Forbidden
        }
    }
}

// --- V010: File Broker with Document Grants ---

pub const MAX_DOCUMENT_GRANTS: usize = 4;

pub struct DocumentGrant {
    pub filename: [u8; 32],
    pub len: usize,
    pub writable: bool,
}

pub struct CapsuleFileBroker {
    pub grants: [Option<DocumentGrant>; MAX_DOCUMENT_GRANTS],
}

impl CapsuleFileBroker {
    pub const fn new() -> Self {
        Self {
            grants: [None, None, None, None],
        }
    }

    pub fn grant_file(&mut self, path: &str, writable: bool) -> Result<(), ()> {
        let b = path.as_bytes();
        if b.len() > 32 {
            return Err(());
        }
        for slot in self.grants.iter_mut() {
            if slot.is_none() {
                let mut buf = [0u8; 32];
                buf[..b.len()].copy_from_slice(b);
                *slot = Some(DocumentGrant {
                    filename: buf,
                    len: b.len(),
                    writable,
                });
                return Ok(());
            }
        }
        Err(())
    }

    pub fn is_access_granted(&self, path: &str, is_write: bool) -> bool {
        let b = path.as_bytes();
        for slot in self.grants.iter().flatten() {
            if slot.len == b.len() && &slot.filename[..slot.len] == b {
                return if is_write { slot.writable } else { true };
            }
        }
        false
    }
}

// --- V012 & V013: Compatibility Resolver & Lightweight Windows API Emulation ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryType {
    NativeElf64,
    WindowsPe32Plus,
    Unknown,
}

pub struct CompatibilityResolver;

impl CompatibilityResolver {
    pub fn scan_binary(header: &[u8]) -> BinaryType {
        if header.len() >= 4 && &header[0..4] == b"\x7fELF" {
            BinaryType::NativeElf64
        } else if header.len() >= 2 && &header[0..2] == b"MZ" {
            BinaryType::WindowsPe32Plus
        } else {
            BinaryType::Unknown
        }
    }

    /// Emulates lightweight Win32 API calls in isolated sandbox
    pub fn dispatch_win32_call(api_id: u32, arg0: u64) -> Result<u64, &'static str> {
        match api_id {
            0x01 => Ok(10), // GetVersionExW -> Windows 10
            0x02 => Ok(0x0040_0000), // GetModuleHandleW
            0x03 => Ok(arg0), // ExitProcess(code)
            _ => Err("UNSUPPORTED_WIN32_API_DIAGNOSTIC: call not implemented in microkernel sandbox"),
        }
    }
}

// --- V016: Resource Benchmark Telemetry ---

#[derive(Debug, Clone, Copy)]
pub struct GuestBenchmarkMetrics {
    pub cpu_cycles: u64,
    pub host_ram_headroom_mb: usize,
    pub guest_allocated_ram_mb: usize,
    pub model_unloaded_pages: usize,
}
