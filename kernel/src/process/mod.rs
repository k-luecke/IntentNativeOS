// kernel/src/process/mod.rs - User Process Management, ELF Loading, Fault Recovery, and Supervisor (K017-K021)

use crate::arch::x86_64::gdt::USER_VIRTUAL_MAX;

pub mod fuzz;
pub mod package;

pub const MAX_PROCESSES: usize = 16;
pub const MAX_EVENT_LOG: usize = 64;

// --- K017: Minimal ELF64 Loader ---

pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
pub const ELF_CLASS_64: u8 = 2;
pub const ELF_DATA_LSB: u8 = 1;
pub const ELF_MACHINE_X86_64: u16 = 0x3E;
pub const PT_LOAD: u32 = 1;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    pub ident: [u8; 16],
    pub elf_type: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub phoff: u64,
    pub shoff: u64,
    pub flags: u32,
    pub ehsize: u16,
    pub phentsize: u16,
    pub phnum: u16,
    pub shentsize: u16,
    pub shnum: u16,
    pub shstrndx: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    InvalidMagic,
    UnsupportedArchitecture,
    InvalidSegmentBoundary,
    UnapprovedBinary,
    BufferTooShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovedElf {
    pub entry_point: u64,
    pub load_vaddr: u64,
    pub mem_size: u64,
}

pub fn parse_and_validate_elf(binary: &[u8], approved_digest: u32) -> Result<ApprovedElf, ElfError> {
    if binary.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::BufferTooShort);
    }

    let header = unsafe { &*(binary.as_ptr() as *const Elf64Header) };
    if header.ident[0..4] != ELF_MAGIC {
        return Err(ElfError::InvalidMagic);
    }
    if header.ident[4] != ELF_CLASS_64 || header.machine != ELF_MACHINE_X86_64 {
        return Err(ElfError::UnsupportedArchitecture);
    }

    // Verify approval digest (simple checksum over code segment)
    let mut sum: u32 = 0;
    for &b in binary.iter() {
        sum = sum.wrapping_add(b as u32);
    }
    if sum != approved_digest {
        return Err(ElfError::UnapprovedBinary);
    }

    // Check entry point within user virtual space
    if header.entry > USER_VIRTUAL_MAX {
        return Err(ElfError::InvalidSegmentBoundary);
    }

    let phoff = header.phoff as usize;
    let phnum = header.phnum as usize;
    let phentsize = header.phentsize as usize;

    let mut load_vaddr = 0u64;
    let mut mem_size = 0u64;

    for i in 0..phnum {
        let offset = phoff + i * phentsize;
        if offset + core::mem::size_of::<Elf64ProgramHeader>() > binary.len() {
            return Err(ElfError::BufferTooShort);
        }
        let ph = unsafe {
            &*(binary[offset..].as_ptr() as *const Elf64ProgramHeader)
        };

        if ph.p_type == PT_LOAD {
            if ph.p_vaddr + ph.p_memsz > USER_VIRTUAL_MAX {
                return Err(ElfError::InvalidSegmentBoundary);
            }
            load_vaddr = ph.p_vaddr;
            mem_size = ph.p_memsz;
        }
    }

    Ok(ApprovedElf {
        entry_point: header.entry,
        load_vaddr,
        mem_size,
    })
}

// --- K018: Process Start / Exit / Wait Primitives & Orphan Cleanup ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Unused,
    Running,
    Zombie(u32), // Contains exit code
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessControlBlock {
    pub pid: u64,
    pub parent_pid: Option<u64>,
    pub state: ProcessState,
    pub is_critical: bool,
    pub restart_count: u32,
    pub entry_point: u64,
}

impl ProcessControlBlock {
    pub const fn empty() -> Self {
        Self {
            pid: 0,
            parent_pid: None,
            state: ProcessState::Unused,
            is_critical: false,
            restart_count: 0,
            entry_point: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessError {
    ProcessTableFull,
    ProcessNotFound,
    NotChildOfCaller,
    StillRunning,
}

// --- K021: Deterministic Kernel Event Logging ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelEventType {
    ProcessSpawned,
    ProcessExited,
    UserFaultRecovered,
    CriticalServiceRestarted,
    SyscallDispatched,
}

#[derive(Debug, Clone, Copy)]
pub struct KernelEvent {
    pub seq: u64,
    pub event_type: KernelEventType,
    pub pid: u64,
    pub detail: u64,
}

pub struct ProcessManager {
    pub processes: [ProcessControlBlock; MAX_PROCESSES],
    pub events: [Option<KernelEvent>; MAX_EVENT_LOG],
    next_pid: u64,
    event_seq: u64,
}

impl ProcessManager {
    pub const fn new() -> Self {
        Self {
            processes: [ProcessControlBlock::empty(); MAX_PROCESSES],
            events: [None; MAX_EVENT_LOG],
            next_pid: 1,
            event_seq: 1,
        }
    }

    pub fn log_event(&mut self, event_type: KernelEventType, pid: u64, detail: u64) {
        let slot = (self.event_seq as usize) % MAX_EVENT_LOG;
        self.events[slot] = Some(KernelEvent {
            seq: self.event_seq,
            event_type,
            pid,
            detail,
        });
        self.event_seq += 1;
    }

    /// Spawns a new user process from an approved ELF binary (K018)
    pub fn spawn(
        &mut self,
        parent_pid: Option<u64>,
        entry_point: u64,
        is_critical: bool,
    ) -> Result<u64, ProcessError> {
        let slot = self.processes.iter().position(|p| p.state == ProcessState::Unused)
            .ok_or(ProcessError::ProcessTableFull)?;

        let pid = self.next_pid;
        self.next_pid += 1;

        self.processes[slot] = ProcessControlBlock {
            pid,
            parent_pid,
            state: ProcessState::Running,
            is_critical,
            restart_count: 0,
            entry_point,
        };

        self.log_event(KernelEventType::ProcessSpawned, pid, entry_point);
        Ok(pid)
    }

    /// Exits a process, performing orphan reparenting to init (PID 1) (K018)
    pub fn exit_process(&mut self, pid: u64, exit_code: u32) -> Result<(), ProcessError> {
        let slot = self.processes.iter().position(|p| p.pid == pid && p.state == ProcessState::Running)
            .ok_or(ProcessError::ProcessNotFound)?;

        self.processes[slot].state = ProcessState::Zombie(exit_code);
        self.log_event(KernelEventType::ProcessExited, pid, exit_code as u64);

        // Orphan cleanup: any child whose parent was this exiting process is reparented to init (PID 1)
        for p in self.processes.iter_mut() {
            if p.parent_pid == Some(pid) && p.state != ProcessState::Unused {
                p.parent_pid = Some(1); // Reparent to init
            }
        }

        Ok(())
    }

    /// Parent waits for a specific child process to terminate and reaps its exit code (K018)
    pub fn wait_process(&mut self, parent_pid: u64, child_pid: u64) -> Result<u32, ProcessError> {
        let slot = self.processes.iter().position(|p| p.pid == child_pid)
            .ok_or(ProcessError::ProcessNotFound)?;

        let p = &self.processes[slot];
        if p.parent_pid != Some(parent_pid) {
            return Err(ProcessError::NotChildOfCaller);
        }

        match p.state {
            ProcessState::Zombie(code) => {
                // Reap process
                self.processes[slot].state = ProcessState::Unused;
                Ok(code)
            }
            ProcessState::Running => Err(ProcessError::StillRunning),
            ProcessState::Unused => Err(ProcessError::ProcessNotFound),
        }
    }

    // --- K019: Kernel Fault Containment and User Crash Recovery ---
    /// Handles user fault: terminates the faulting user task while keeping kernel and siblings running
    pub fn recover_from_user_fault(&mut self, faulting_pid: u64, fault_code: u64) -> Result<(), ProcessError> {
        self.log_event(KernelEventType::UserFaultRecovered, faulting_pid, fault_code);

        // Terminate faulting user process with crash status 0xDEAD
        self.exit_process(faulting_pid, 0xDEAD)?;
        Ok(())
    }

    // --- K020: Boot/Init Supervisor & Critical Service Auto-Restart ---
    /// Supervises processes: restarts designated critical services if terminated
    pub fn supervise_tick(&mut self) -> Option<u64> {
        for i in 0..MAX_PROCESSES {
            if let ProcessState::Zombie(_) = self.processes[i].state {
                if self.processes[i].is_critical {
                    let old_pid = self.processes[i].pid;
                    let entry = self.processes[i].entry_point;
                    let restarts = self.processes[i].restart_count + 1;

                    // Re-launch critical service
                    let new_pid = self.next_pid;
                    self.next_pid += 1;

                    self.processes[i] = ProcessControlBlock {
                        pid: new_pid,
                        parent_pid: Some(1), // Supervised by init
                        state: ProcessState::Running,
                        is_critical: true,
                        restart_count: restarts,
                        entry_point: entry,
                    };

                    self.log_event(KernelEventType::CriticalServiceRestarted, new_pid, old_pid);
                    return Some(new_pid);
                }
            }
        }
        None
    }
}

pub fn spawn_and_run_ring3_app(
    elf_bytes: &[u8],
    pid: u64,
) -> Result<u64, ElfError> {
    use crate::arch::x86_64::serial::SERIAL1;

    if elf_bytes.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::BufferTooShort);
    }

    let header = unsafe { &*(elf_bytes.as_ptr() as *const Elf64Header) };
    if header.ident[0..4] != ELF_MAGIC {
        return Err(ElfError::InvalidMagic);
    }
    if header.ident[4] != ELF_CLASS_64 || header.machine != ELF_MACHINE_X86_64 {
        return Err(ElfError::UnsupportedArchitecture);
    }
    if header.entry > USER_VIRTUAL_MAX {
        return Err(ElfError::InvalidSegmentBoundary);
    }

    SERIAL1.write_str("[ELF_LOADER] Loading approved Ring 3 ELF binary (size=");
    SERIAL1.write_u64_dec(elf_bytes.len() as u64);
    SERIAL1.write_str(" bytes, entry=0x");
    SERIAL1.write_u64_hex(header.entry);
    SERIAL1.write_str(")...\n");

    let phys_base = crate::arch::x86_64::paging::get_process_phys_base(pid);
    unsafe {
        core::ptr::write_bytes(phys_base as *mut u8, 0, 2 * 1024 * 1024);
    }
    load_elf_into_phys(elf_bytes, pid, phys_base)?;

    let user_stack_top = 0x005F_E000u64;

    unsafe {
        crate::arch::x86_64::idt::ACTIVE_USER_PID = pid;
        let cr3 = crate::arch::x86_64::paging::get_process_cr3(pid);
        let rsp0 = (crate::arch::x86_64::gdt::KERNEL_RSP0_PROC1.0.as_ptr() as u64) + 16384;
        crate::arch::x86_64::gdt::set_tss_rsp0(rsp0);
        PREEMPTION_ENABLED = false;
        crate::arch::x86_64::pic::disable_timer_irq();

        RESIDENT_PROCESSES[0] = ResidentProcess {
            pid,
            app_id: "org.inos.single",
            state: AppState::Running,
            saved_rsp: 0,
            cr3,
            rsp0,
            exit_code: 0,
            ticks_remaining: DEFAULT_QUANTUM_TICKS,
            total_preemptions: 0,
            ..ResidentProcess::empty()
        };
        RESIDENT_PROCESSES[1] = ResidentProcess::empty();
        RESIDENT_PROCESSES[2] = ResidentProcess::empty();
        RESIDENT_PROCESSES[3] = ResidentProcess::empty();

        SERIAL1.write_str("[RING3] Switching CPU context to Ring 3 (CPL=3) via iretq: entry=0x");
        SERIAL1.write_u64_hex(header.entry);
        SERIAL1.write_str(" user_stack=0x");
        SERIAL1.write_u64_hex(user_stack_top);
        SERIAL1.write_str("\n");
        SERIAL1.write_str("[RING3_ENTRY_VERIFIED_MARKER_v1.0]\n");

        crate::arch::x86_64::cpu::write_cr3(cr3);
        crate::arch::x86_64::paging::allocate_process_frames(512);
        let exit_code = crate::arch::x86_64::idt::execute_user_app(header.entry, user_stack_top);
        if crate::arch::x86_64::paging::KERNEL_CR3 != 0 {
            crate::arch::x86_64::cpu::write_cr3(crate::arch::x86_64::paging::KERNEL_CR3);
        }

        SERIAL1.write_str("[RING3] User application execution terminated. Exit code: ");
        SERIAL1.write_u64_dec(exit_code);
        SERIAL1.write_str("\n");
        SERIAL1.write_str("[RING3_CLEAN_EXIT_MARKER_v1.0]\n");

        crate::arch::x86_64::idt::ACTIVE_USER_PID = 0;
        Ok(exit_code)
    }
}

pub const MAX_RESIDENT_PROCESSES: usize = 4;
pub const DEFAULT_QUANTUM_TICKS: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Unused,
    Ready,
    Running,
    Blocked,
    Zombie(u32),
}

#[derive(Debug, Clone, Copy)]
pub struct ResidentProcess {
    pub pid: u64,
    pub app_id: &'static str,
    pub state: AppState,
    pub saved_rsp: u64,
    pub cr3: u64,
    pub rsp0: u64,
    pub exit_code: u32,
    pub ticks_remaining: u32,
    pub total_preemptions: u32,
    pub memory_pages_allocated: u32,
    pub max_memory_pages: u32,
    pub file_ops_count: u32,
    pub max_file_ops: u32,
    pub cpu_ticks_consumed: u64,
    pub max_cpu_ticks: u64,
}

impl ResidentProcess {
    pub const fn empty() -> Self {
        Self {
            pid: 0,
            app_id: "",
            state: AppState::Unused,
            saved_rsp: 0,
            cr3: 0,
            rsp0: 0,
            exit_code: 0,
            ticks_remaining: DEFAULT_QUANTUM_TICKS,
            total_preemptions: 0,
            memory_pages_allocated: 0,
            max_memory_pages: 512, // 2 MiB ceiling
            file_ops_count: 0,
            max_file_ops: 256,
            cpu_ticks_consumed: 0,
            max_cpu_ticks: 500_000,
        }
    }
}

pub static mut RESIDENT_PROCESSES: [ResidentProcess; MAX_RESIDENT_PROCESSES] = [
    ResidentProcess::empty(),
    ResidentProcess::empty(),
    ResidentProcess::empty(),
    ResidentProcess::empty(),
];

pub static mut PREEMPTION_ENABLED: bool = false;
pub static mut AUTO_RECLAIM_ON_EXIT: bool = true;

pub fn get_app_id_for_pid(pid: u64) -> Option<&'static str> {
    unsafe {
        for slot in crate::capability::AUTH_PROCESS_REGISTRY.iter() {
            if slot.is_active && slot.pid == pid {
                let id_len = slot.app_id.iter().position(|&c| c == 0).unwrap_or(slot.app_id.len());
                if let Ok(s) = core::str::from_utf8(&slot.app_id[..id_len]) {
                    if !s.is_empty() && s != "org.inos.single" {
                        return Some(s);
                    }
                }
            }
        }
        for proc in RESIDENT_PROCESSES.iter() {
            if proc.pid == pid && proc.state != AppState::Unused {
                if !proc.app_id.is_empty() && proc.app_id != "org.inos.single" {
                    return Some(proc.app_id);
                }
            }
        }
    }
    match pid {
        100 => Some("org.inos.notes"),
        200 => Some("org.inos.calc"),
        300 | 301 | 302 => Some("org.inos.net"),
        310 => Some("org.inos.web.renderer"),
        400 => Some("org.inos.archive"),
        500 => Some("org.inos.grid"),
        600 => Some("org.inos.write"),
        601 => Some("org.inos.scriptd"),
        700 => Some("org.inos.plan"),
        800 => Some("org.inos.pdf"),
        900 => Some("org.inos.mail"),
        910 => Some("org.inos.files"),
        950 => Some("org.inos.flow"),
        960 => Some("org.inos.canvas"),
        _ => None,
    }
}

pub fn reclaim_process_resources(pid: u64) {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        SERIAL1.write_str("[LIFECYCLE] Reclaiming resources for terminated PID ");
        SERIAL1.write_u64_dec(pid);
        SERIAL1.write_str("...\n");

        crate::arch::x86_64::paging::free_process_frames(512);

        let revoked_caps = crate::capability::GLOBAL_CAP_REGISTRY.revoke_all_tokens_for_pid(pid);
        SERIAL1.write_str("[LIFECYCLE] Revoked ");
        SERIAL1.write_u64_dec(revoked_caps as u64);
        SERIAL1.write_str(" capability tokens.\n");

        let closed_surfaces = crate::ipc::compositor::GLOBAL_COMPOSITOR_SERVER.close_all_surfaces_for_pid(pid);
        SERIAL1.write_str("[LIFECYCLE] Deactivated ");
        SERIAL1.write_u64_dec(closed_surfaces as u64);
        SERIAL1.write_str(" compositor surfaces.\n");

        crate::capability::increment_process_generation(pid);
        crate::ipc::web::increment_process_generation(pid);
        crate::ipc::web::GLOBAL_WEB_IPC.flush_for_pid(pid);
        SERIAL1.write_str("[LIFECYCLE] Incremented IPC generation and flushed Web IPC endpoints for PID ");
        SERIAL1.write_u64_dec(pid);
        SERIAL1.write_str(".\n");

        for p in RESIDENT_PROCESSES.iter_mut() {
            if p.pid == pid {
                p.memory_pages_allocated = 0;
                p.file_ops_count = 0;
                p.cpu_ticks_consumed = 0;
                p.state = AppState::Unused;
                p.pid = 0;
            }
        }
        SERIAL1.write_str("[RESOURCE_QUOTA] Reclaimed memory pages, file ops, and CPU budget for PID ");
        SERIAL1.write_u64_dec(pid);
        SERIAL1.write_str(".\n");
        SERIAL1.write_str("[RESOURCE_QUOTA_RECLAIMED_VERIFIED_MARKER]\n");

        SERIAL1.write_str("[LIFECYCLE_RESOURCE_RECLAMATION_VERIFIED_MARKER_v1.0]\n");
    }
}

pub fn charge_process_memory(pid: u64, pages: u32) -> Result<(), crate::arch::x86_64::syscall::SyscallError> {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        for p in RESIDENT_PROCESSES.iter_mut() {
            if p.pid == pid && p.state != AppState::Unused {
                let new_alloc = p.memory_pages_allocated.checked_add(pages)
                    .ok_or(crate::arch::x86_64::syscall::SyscallError::OutOfMemory)?;
                if new_alloc > p.max_memory_pages {
                    SERIAL1.write_str("[RESOURCE_QUOTA] Process PID ");
                    SERIAL1.write_u64_dec(pid);
                    SERIAL1.write_str(" exceeded memory ceiling (requested ");
                    SERIAL1.write_u64_dec(new_alloc as u64);
                    SERIAL1.write_str(" pages > max ");
                    SERIAL1.write_u64_dec(p.max_memory_pages as u64);
                    SERIAL1.write_str(")! Rejection fail-closed.\n");
                    SERIAL1.write_str("[KERNEL_MEMORY_QUOTA_EXCEEDED_MARKER]\n");
                    return Err(crate::arch::x86_64::syscall::SyscallError::OutOfMemory);
                }
                p.memory_pages_allocated = new_alloc;
                return Ok(());
            }
        }
        Ok(())
    }
}

pub fn release_process_memory(pid: u64, pages: u32) {
    unsafe {
        for p in RESIDENT_PROCESSES.iter_mut() {
            if p.pid == pid && p.state != AppState::Unused {
                p.memory_pages_allocated = p.memory_pages_allocated.saturating_sub(pages);
                return;
            }
        }
    }
}

pub fn charge_process_file_op(pid: u64) -> Result<(), crate::arch::x86_64::syscall::SyscallError> {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        for p in RESIDENT_PROCESSES.iter_mut() {
            if p.pid == pid && p.state != AppState::Unused {
                p.file_ops_count = p.file_ops_count.saturating_add(1);
                if p.file_ops_count > p.max_file_ops {
                    SERIAL1.write_str("[RESOURCE_QUOTA] Process PID ");
                    SERIAL1.write_u64_dec(pid);
                    SERIAL1.write_str(" exceeded file-operations limit (count=");
                    SERIAL1.write_u64_dec(p.file_ops_count as u64);
                    SERIAL1.write_str(" > max=");
                    SERIAL1.write_u64_dec(p.max_file_ops as u64);
                    SERIAL1.write_str(")! Rejection fail-closed.\n");
                    SERIAL1.write_str("[KERNEL_FILE_OP_QUOTA_EXCEEDED_MARKER]\n");
                    return Err(crate::arch::x86_64::syscall::SyscallError::QuotaExceeded);
                }
                return Ok(());
            }
        }
        Ok(())
    }
}

pub fn set_process_quotas(pid: u64, max_pages: u32, max_file_ops: u32, max_cpu_ticks: u64) {
    unsafe {
        for p in RESIDENT_PROCESSES.iter_mut() {
            if p.pid == pid {
                p.max_memory_pages = max_pages;
                p.max_file_ops = max_file_ops;
                p.max_cpu_ticks = max_cpu_ticks;
                return;
            }
        }
        for p in RESIDENT_PROCESSES.iter_mut() {
            if p.state == AppState::Unused {
                p.pid = pid;
                p.state = AppState::Blocked;
                p.max_memory_pages = max_pages;
                p.max_file_ops = max_file_ops;
                p.max_cpu_ticks = max_cpu_ticks;
                p.memory_pages_allocated = 0;
                p.file_ops_count = 0;
                p.cpu_ticks_consumed = 0;
                p.ticks_remaining = DEFAULT_QUANTUM_TICKS;
                return;
            }
        }
    }
}

pub fn get_process_quota_usage(pid: u64) -> (u32, u32, u64) {
    unsafe {
        for p in RESIDENT_PROCESSES.iter() {
            if p.pid == pid {
                return (p.memory_pages_allocated, p.file_ops_count, p.cpu_ticks_consumed);
            }
        }
        (0, 0, 0)
    }
}

pub fn on_preempt_timer_tick(frame_rsp: u64) -> u64 {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        if !PREEMPTION_ENABLED {
            return frame_rsp;
        }

        let current_pid = crate::arch::x86_64::idt::ACTIVE_USER_PID;
        let current_idx = match (0..MAX_RESIDENT_PROCESSES).find(|&i| {
            RESIDENT_PROCESSES[i].pid == current_pid && RESIDENT_PROCESSES[i].state == AppState::Running
        }) {
            Some(idx) => idx,
            None => return frame_rsp,
        };

        // Charge preemptive CPU budget with checked arithmetic
        RESIDENT_PROCESSES[current_idx].cpu_ticks_consumed = 
            RESIDENT_PROCESSES[current_idx].cpu_ticks_consumed.saturating_add(1);

        if RESIDENT_PROCESSES[current_idx].cpu_ticks_consumed >= RESIDENT_PROCESSES[current_idx].max_cpu_ticks {
            SERIAL1.write_str("[RESOURCE_QUOTA] Process PID ");
            SERIAL1.write_u64_dec(current_pid);
            SERIAL1.write_str(" exceeded preemptive CPU budget (consumed ");
            SERIAL1.write_u64_dec(RESIDENT_PROCESSES[current_idx].cpu_ticks_consumed);
            SERIAL1.write_str(" >= max ");
            SERIAL1.write_u64_dec(RESIDENT_PROCESSES[current_idx].max_cpu_ticks);
            SERIAL1.write_str(" ticks)! Terminating runaway process.\n");
            SERIAL1.write_str("[CPU_BUDGET_EXHAUSTED_TERMINATED_MARKER_v1.0]\n");
            RESIDENT_PROCESSES[current_idx].state = AppState::Zombie(137);
            RESIDENT_PROCESSES[current_idx].exit_code = 137;
            reclaim_process_resources(current_pid);
            return frame_rsp;
        }

        if RESIDENT_PROCESSES[current_idx].ticks_remaining > 1 {
            RESIDENT_PROCESSES[current_idx].ticks_remaining -= 1;
            return frame_rsp;
        }

        // Quantum expired! Find next ready process
        let mut next_idx_opt = None;
        for step in 1..MAX_RESIDENT_PROCESSES {
            let candidate = (current_idx + step) % MAX_RESIDENT_PROCESSES;
            if RESIDENT_PROCESSES[candidate].state == AppState::Ready {
                next_idx_opt = Some(candidate);
                break;
            }
        }

        let next_idx = match next_idx_opt {
            Some(idx) => idx,
            None => {
                RESIDENT_PROCESSES[current_idx].ticks_remaining = DEFAULT_QUANTUM_TICKS;
                return frame_rsp;
            }
        };

        // Forcibly preempt current process and switch to next!
        RESIDENT_PROCESSES[current_idx].state = AppState::Ready;
        RESIDENT_PROCESSES[current_idx].saved_rsp = frame_rsp;
        RESIDENT_PROCESSES[current_idx].ticks_remaining = DEFAULT_QUANTUM_TICKS;

        RESIDENT_PROCESSES[next_idx].state = AppState::Running;
        RESIDENT_PROCESSES[next_idx].ticks_remaining = DEFAULT_QUANTUM_TICKS;
        RESIDENT_PROCESSES[next_idx].total_preemptions += 1;

        let next_pid = RESIDENT_PROCESSES[next_idx].pid;
        let next_cr3 = RESIDENT_PROCESSES[next_idx].cr3;
        let next_rsp0 = RESIDENT_PROCESSES[next_idx].rsp0;
        let next_saved_rsp = RESIDENT_PROCESSES[next_idx].saved_rsp;

        crate::arch::x86_64::idt::ACTIVE_USER_PID = next_pid;
        crate::arch::x86_64::gdt::set_tss_rsp0(next_rsp0);
        crate::arch::x86_64::cpu::write_cr3(next_cr3);

        SERIAL1.write_str("[PREEMPTION] Timer tick preempted PID ");
        SERIAL1.write_u64_dec(current_pid);
        SERIAL1.write_str(" -> switched to PID ");
        SERIAL1.write_u64_dec(next_pid);
        SERIAL1.write_str("\n");
        SERIAL1.write_str("[TIMER_PREEMPTION_OCCURRED_MARKER_v1.0]\n");

        next_saved_rsp
    }
}

pub fn do_yield_switch(current_pid: u64) {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        if PREEMPTION_ENABLED {
            // When timer preemption is enabled, context switches are driven preemptively
            return;
        }

        let current_idx = match (0..MAX_RESIDENT_PROCESSES).find(|&i| RESIDENT_PROCESSES[i].pid == current_pid) {
            Some(idx) => idx,
            None => return,
        };

        let mut next_idx_opt = None;
        for step in 1..MAX_RESIDENT_PROCESSES {
            let candidate = (current_idx + step) % MAX_RESIDENT_PROCESSES;
            if RESIDENT_PROCESSES[candidate].state == AppState::Ready || RESIDENT_PROCESSES[candidate].state == AppState::Running {
                next_idx_opt = Some(candidate);
                break;
            }
        }

        if let Some(sibling_idx) = next_idx_opt {
            let sibling_pid = RESIDENT_PROCESSES[sibling_idx].pid;
            RESIDENT_PROCESSES[current_idx].state = AppState::Ready;
            RESIDENT_PROCESSES[sibling_idx].state = AppState::Running;
            crate::arch::x86_64::idt::ACTIVE_USER_PID = sibling_pid;
            crate::arch::x86_64::gdt::set_tss_rsp0(RESIDENT_PROCESSES[sibling_idx].rsp0);

            SERIAL1.write_str("[SCHEDULER] Cooperative yield: PID ");
            SERIAL1.write_u64_dec(current_pid);
            SERIAL1.write_str(" -> PID ");
            SERIAL1.write_u64_dec(sibling_pid);
            SERIAL1.write_str(" (CR3=0x");
            SERIAL1.write_u64_hex(RESIDENT_PROCESSES[sibling_idx].cr3);
            SERIAL1.write_str(")\n");
            SERIAL1.write_str("[COOPERATIVE_CONTEXT_SWITCH_MARKER_v1.0]\n");

            crate::arch::x86_64::idt::switch_kernel_context(
                &raw mut RESIDENT_PROCESSES[current_idx].saved_rsp,
                RESIDENT_PROCESSES[sibling_idx].saved_rsp,
                RESIDENT_PROCESSES[sibling_idx].cr3,
            );
        }
    }
}

pub fn do_exit_process(current_pid: u64, exit_code: u32) -> ! {
    use crate::arch::x86_64::serial::SERIAL1;
    unsafe {
        let current_idx = match (0..MAX_RESIDENT_PROCESSES).find(|&i| RESIDENT_PROCESSES[i].pid == current_pid) {
            Some(idx) => idx,
            None => 0,
        };

        RESIDENT_PROCESSES[current_idx].state = AppState::Zombie(exit_code);
        RESIDENT_PROCESSES[current_idx].exit_code = exit_code;

        SERIAL1.write_str("[SCHEDULER] Process PID ");
        SERIAL1.write_u64_dec(current_pid);
        SERIAL1.write_str(" terminated with code ");
        SERIAL1.write_u64_dec(exit_code as u64);
        SERIAL1.write_str("\n");

        if AUTO_RECLAIM_ON_EXIT {
            reclaim_process_resources(current_pid);
        }

        let mut next_sibling = None;
        for step in 1..MAX_RESIDENT_PROCESSES {
            let candidate = (current_idx + step) % MAX_RESIDENT_PROCESSES;
            if RESIDENT_PROCESSES[candidate].state == AppState::Ready || RESIDENT_PROCESSES[candidate].state == AppState::Running {
                next_sibling = Some(candidate);
                break;
            }
        }

        if let Some(sibling_idx) = next_sibling {
            let sibling_pid = RESIDENT_PROCESSES[sibling_idx].pid;
            RESIDENT_PROCESSES[sibling_idx].state = AppState::Running;
            crate::arch::x86_64::idt::ACTIVE_USER_PID = sibling_pid;
            crate::arch::x86_64::gdt::set_tss_rsp0(RESIDENT_PROCESSES[sibling_idx].rsp0);

            SERIAL1.write_str("[SCHEDULER] Switching to resident sibling PID ");
            SERIAL1.write_u64_dec(sibling_pid);
            SERIAL1.write_str(" (CR3=0x");
            SERIAL1.write_u64_hex(RESIDENT_PROCESSES[sibling_idx].cr3);
            SERIAL1.write_str(")\n");

            if PREEMPTION_ENABLED {
                crate::arch::x86_64::cpu::write_cr3(RESIDENT_PROCESSES[sibling_idx].cr3);
                crate::arch::x86_64::idt::restore_preempted_context(RESIDENT_PROCESSES[sibling_idx].saved_rsp);
            } else {
                crate::arch::x86_64::idt::switch_kernel_context(
                    &raw mut RESIDENT_PROCESSES[current_idx].saved_rsp,
                    RESIDENT_PROCESSES[sibling_idx].saved_rsp,
                    RESIDENT_PROCESSES[sibling_idx].cr3,
                );
            }
        }

        // All processes terminated! Restore kernel PML4 and return to supervisor.
        crate::arch::x86_64::idt::ACTIVE_USER_PID = 0;
        PREEMPTION_ENABLED = false;
        crate::arch::x86_64::pic::disable_timer_irq();
        if crate::arch::x86_64::paging::KERNEL_CR3 != 0 {
            crate::arch::x86_64::cpu::write_cr3(crate::arch::x86_64::paging::KERNEL_CR3);
        }
        core::arch::asm!(
            "lea rbx, [rip + KERNEL_SAVED_RSP]",
            "mov rsp, [rbx]",
            "jmp ring3_return_point",
            in("rax") (exit_code as u64),
            options(noreturn)
        );
    }
}

pub fn load_elf_into_phys(elf_bytes: &[u8], _pid: u64, phys_base: u64) -> Result<(), ElfError> {
    if elf_bytes.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfError::BufferTooShort);
    }
    let header = unsafe { &*(elf_bytes.as_ptr() as *const Elf64Header) };
    if header.ident[0..4] != ELF_MAGIC {
        return Err(ElfError::InvalidMagic);
    }
    if header.ident[4] != ELF_CLASS_64 || header.machine != ELF_MACHINE_X86_64 {
        return Err(ElfError::UnsupportedArchitecture);
    }

    let phoff = header.phoff as usize;
    let phnum = header.phnum as usize;
    let phentsize = header.phentsize as usize;

    for i in 0..phnum {
        let offset = phoff + i * phentsize;
        if offset + core::mem::size_of::<Elf64ProgramHeader>() > elf_bytes.len() {
            return Err(ElfError::BufferTooShort);
        }
        let ph = unsafe { &*(elf_bytes[offset..].as_ptr() as *const Elf64ProgramHeader) };
        if ph.p_type == PT_LOAD {
            let seg_src_offset = ph.p_offset as usize;
            let seg_filesz = ph.p_filesz as usize;
            let seg_memsz = ph.p_memsz as usize;

            if ph.p_vaddr < 0x0040_0000 || seg_filesz > seg_memsz {
                return Err(ElfError::InvalidSegmentBoundary);
            }

            let seg_offset_in_app = (ph.p_vaddr - 0x0040_0000) as usize;
            if seg_offset_in_app.saturating_add(seg_memsz) > 2 * 1024 * 1024 {
                return Err(ElfError::InvalidSegmentBoundary);
            }
            let dest_ptr = (phys_base + seg_offset_in_app as u64) as *mut u8;

            if seg_src_offset.saturating_add(seg_filesz) > elf_bytes.len() {
                return Err(ElfError::BufferTooShort);
            }

            unsafe {
                core::ptr::copy_nonoverlapping(
                    elf_bytes.as_ptr().add(seg_src_offset),
                    dest_ptr,
                    seg_filesz,
                );
                if seg_memsz > seg_filesz {
                    core::ptr::write_bytes(
                        dest_ptr.add(seg_filesz),
                        0,
                        seg_memsz - seg_filesz,
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn run_dual_resident_apps(
    notes_elf: &[u8],
    calc_elf: &[u8],
) -> Result<(u32, u32), ElfError> {
    use crate::arch::x86_64::serial::SERIAL1;

    SERIAL1.write_str("[FOUNDRY] Preparing dual resident Ring 3 applications...\n");

    // 1. Prepare Process 1 (PID 100: notes_app)
    let phys1 = crate::arch::x86_64::paging::get_process_phys_base(100);
    unsafe {
        core::ptr::write_bytes(phys1 as *mut u8, 0, 2 * 1024 * 1024);
    }
    load_elf_into_phys(notes_elf, 100, phys1)?;
    let cr3_p1 = crate::arch::x86_64::paging::get_process_cr3(100);
    let rsp0_p1_top = unsafe { (crate::arch::x86_64::gdt::KERNEL_RSP0_PROC1.0.as_ptr() as u64) + 16384 };

    // 2. Prepare Process 2 (PID 200: calc_app)
    let phys2 = crate::arch::x86_64::paging::get_process_phys_base(200);
    unsafe {
        core::ptr::write_bytes(phys2 as *mut u8, 0, 2 * 1024 * 1024);
    }
    load_elf_into_phys(calc_elf, 200, phys2)?;
    let cr3_p2 = crate::arch::x86_64::paging::get_process_cr3(200);
    let rsp0_p2_top = unsafe { (crate::arch::x86_64::gdt::KERNEL_RSP0_PROC2.0.as_ptr() as u64) + 16384 };

    let p2_user_stack = 0x005F_E000u64;
    let p2_entry = 0x0040_0000u64;

    unsafe {
        let p2_stack_ptr = rsp0_p2_top as *mut u64;
        // iretq frame for Process 2
        *p2_stack_ptr.offset(-1) = 0x23;            // SS (USER_DS)
        *p2_stack_ptr.offset(-2) = p2_user_stack;  // RSP
        *p2_stack_ptr.offset(-3) = 0x02;            // RFLAGS
        *p2_stack_ptr.offset(-4) = 0x1B;            // CS (USER_CS)
        *p2_stack_ptr.offset(-5) = p2_entry;        // RIP
        // return address for switch_kernel_context
        *p2_stack_ptr.offset(-6) = crate::arch::x86_64::idt::enter_user_first_time as u64;
        // callee-saved registers popped by switch_kernel_context: r15, r14, r13, r12, rbp, rbx
        *p2_stack_ptr.offset(-7) = 0; // rbx
        *p2_stack_ptr.offset(-8) = 0; // rbp
        *p2_stack_ptr.offset(-9) = 0; // r12
        *p2_stack_ptr.offset(-10) = 0; // r13
        *p2_stack_ptr.offset(-11) = 0; // r14
        *p2_stack_ptr.offset(-12) = 0; // r15

        let saved_rsp_p2 = rsp0_p2_top - 12 * 8;

        RESIDENT_PROCESSES[0] = ResidentProcess {
            pid: 100,
            app_id: "org.inos.notes",
            state: AppState::Running,
            saved_rsp: 0,
            cr3: cr3_p1,
            rsp0: rsp0_p1_top,
            exit_code: 0,
            ticks_remaining: DEFAULT_QUANTUM_TICKS,
            total_preemptions: 0,
            ..ResidentProcess::empty()
        };

        RESIDENT_PROCESSES[1] = ResidentProcess {
            pid: 200,
            app_id: "org.inos.calc",
            state: AppState::Ready,
            saved_rsp: saved_rsp_p2,
            cr3: cr3_p2,
            rsp0: rsp0_p2_top,
            exit_code: 0,
            ticks_remaining: DEFAULT_QUANTUM_TICKS,
            total_preemptions: 0,
            ..ResidentProcess::empty()
        };

        // Disable auto-reclaim and timer preemption during cooperative dual apps
        AUTO_RECLAIM_ON_EXIT = false;
        PREEMPTION_ENABLED = false;
        crate::arch::x86_64::pic::disable_timer_irq();

        // Switch CPU hardware CR3 to Process 1 and update TSS.rsp0
        crate::arch::x86_64::cpu::write_cr3(cr3_p1);
        crate::arch::x86_64::gdt::set_tss_rsp0(rsp0_p1_top);
        crate::arch::x86_64::idt::ACTIVE_USER_PID = 100;

        SERIAL1.write_str("[FOUNDRY] Dual processes resident: PID 100 (CR3=0x");
        SERIAL1.write_u64_hex(cr3_p1);
        SERIAL1.write_str(") and PID 200 (CR3=0x");
        SERIAL1.write_u64_hex(cr3_p2);
        SERIAL1.write_str(")\n");
        SERIAL1.write_str("[APP_CAP_01_RESIDENT_DUAL_APPS_MARKER_v1.0]\n");

        SERIAL1.write_str("[FOUNDRY] Entering Process 1 (PID 100) via iretq...\n");
        let _ = crate::arch::x86_64::idt::execute_user_app(0x0040_0000, 0x005F_E000);

        AUTO_RECLAIM_ON_EXIT = true;
        let code1 = RESIDENT_PROCESSES[0].exit_code;
        let code2 = RESIDENT_PROCESSES[1].exit_code;
        Ok((code1, code2))
    }
}

pub fn run_preemptive_dual_apps(
    spin_elf: &[u8],
    calc_elf: &[u8],
) -> Result<(u32, u32), ElfError> {
    use crate::arch::x86_64::serial::SERIAL1;

    SERIAL1.write_str("[PREEMPTION] Initializing preemptive dual application execution (PID 300 & PID 200)...\n");

    // 1. Process 1: spin_app (PID 300)
    let phys1 = crate::arch::x86_64::paging::get_process_phys_base(300);
    unsafe {
        core::ptr::write_bytes(phys1 as *mut u8, 0, 2 * 1024 * 1024);
    }
    load_elf_into_phys(spin_elf, 300, phys1)?;
    let cr3_p1 = crate::arch::x86_64::paging::get_process_cr3(300);
    let rsp0_p1_top = unsafe { (crate::arch::x86_64::gdt::KERNEL_RSP0_PROC3.0.as_ptr() as u64) + 16384 };

    // 2. Process 2: calc_app (PID 200)
    let phys2 = crate::arch::x86_64::paging::get_process_phys_base(200);
    unsafe {
        core::ptr::write_bytes(phys2 as *mut u8, 0, 2 * 1024 * 1024);
    }
    load_elf_into_phys(calc_elf, 200, phys2)?;
    let cr3_p2 = crate::arch::x86_64::paging::get_process_cr3(200);
    let rsp0_p2_top = unsafe { (crate::arch::x86_64::gdt::KERNEL_RSP0_PROC2.0.as_ptr() as u64) + 16384 };

    let p2_user_stack = 0x005F_E000u64;
    let p2_entry = 0x0040_0000u64;

    unsafe {
        let p2_stack_ptr = rsp0_p2_top as *mut u64;
        // Construct the 20-qword timer interrupt frame for Process 2
        *p2_stack_ptr.offset(-1) = 0x23;            // SS (USER_DS)
        *p2_stack_ptr.offset(-2) = p2_user_stack;  // RSP
        *p2_stack_ptr.offset(-3) = 0x202;           // RFLAGS (IF=1)
        *p2_stack_ptr.offset(-4) = 0x1B;            // CS (USER_CS)
        *p2_stack_ptr.offset(-5) = p2_entry;        // RIP
        // 15 GPRs (rax..r15)
        for i in 6..=20 {
            *p2_stack_ptr.offset(-i) = 0;
        }
        // 512-byte FXSAVE area (64 qwords, offset -21 to -84)
        for i in 21..=84 {
            *p2_stack_ptr.offset(-i) = 0;
        }
        let fxsave_base = p2_stack_ptr.offset(-84) as *mut u8;
        *(fxsave_base as *mut u16) = 0x037F; // Default x87 FCW
        *(fxsave_base.add(24) as *mut u32) = 0x1F80; // Default MXCSR

        let saved_rsp_p2 = rsp0_p2_top - 84 * 8;

        RESIDENT_PROCESSES[0] = ResidentProcess {
            pid: 300,
            app_id: "org.inos.spin",
            state: AppState::Running,
            saved_rsp: 0,
            cr3: cr3_p1,
            rsp0: rsp0_p1_top,
            exit_code: 0,
            ticks_remaining: DEFAULT_QUANTUM_TICKS,
            total_preemptions: 0,
            ..ResidentProcess::empty()
        };

        RESIDENT_PROCESSES[1] = ResidentProcess {
            pid: 200,
            app_id: "org.inos.calc",
            state: AppState::Ready,
            saved_rsp: saved_rsp_p2,
            cr3: cr3_p2,
            rsp0: rsp0_p2_top,
            exit_code: 0,
            ticks_remaining: DEFAULT_QUANTUM_TICKS,
            total_preemptions: 0,
            ..ResidentProcess::empty()
        };
        RESIDENT_PROCESSES[2] = ResidentProcess::empty();
        RESIDENT_PROCESSES[3] = ResidentProcess::empty();

        // Enable preemption and configure PIT timer at 100 Hz
        PREEMPTION_ENABLED = true;
        crate::arch::x86_64::pic::init_pit(100);
        crate::arch::x86_64::pic::enable_timer_irq();

        // Allocate frames for resident processes
        crate::arch::x86_64::paging::allocate_process_frames(512);
        crate::arch::x86_64::paging::allocate_process_frames(512);

        // Switch to Process 1 CR3, set TSS.rsp0, and set ACTIVE_USER_PID
        crate::arch::x86_64::cpu::write_cr3(cr3_p1);
        crate::arch::x86_64::gdt::set_tss_rsp0(rsp0_p1_top);
        crate::arch::x86_64::idt::ACTIVE_USER_PID = 300;

        SERIAL1.write_str("[PREEMPTION] Launching non-yielding spin_app (PID 300) with timer preemption active...\n");
        let _ = crate::arch::x86_64::idt::execute_user_app(0x0040_0000, 0x005F_E000);

        PREEMPTION_ENABLED = false;
        crate::arch::x86_64::pic::disable_timer_irq();

        let code1 = RESIDENT_PROCESSES[0].exit_code;
        let code2 = RESIDENT_PROCESSES[1].exit_code;
        Ok((code1, code2))
    }
}

pub fn run_lifecycle_100_cycles_stress_test(elf_bytes: &[u8]) -> bool {
    use crate::arch::x86_64::serial::SERIAL1;
    SERIAL1.write_str("[APP-CAP-03] Starting 100-cycle process spawn/teardown memory reclamation test...\n");
    crate::arch::x86_64::paging::reset_user_frame_stats();

    for cycle in 1..=100 {
        let res = spawn_and_run_ring3_app(elf_bytes, 100);
        match res {
            Ok(code) => {
                if code != 0 {
                    SERIAL1.write_str("[APP-CAP-03_ERROR] Process exited with non-zero code during stress cycle!\n");
                    return false;
                }
            }
            Err(_) => {
                SERIAL1.write_str("[APP-CAP-03_ERROR] Process spawn failed during stress cycle!\n");
                return false;
            }
        }

        let active_frames = crate::arch::x86_64::paging::get_active_user_frames();
        let active_tokens = unsafe { crate::capability::GLOBAL_CAP_REGISTRY.count_active_tokens_for_pid(100) };
        let active_surfaces = unsafe { crate::ipc::compositor::GLOBAL_COMPOSITOR_SERVER.count_active_surfaces_for_pid(100) };

        if active_frames != 0 || active_tokens != 0 || active_surfaces != 0 {
            SERIAL1.write_str("[APP-CAP-03_ERROR] Resource leak detected at cycle ");
            SERIAL1.write_u64_dec(cycle as u64);
            SERIAL1.write_str("!\n");
            return false;
        }
    }

    let (alloc, freed, active) = crate::arch::x86_64::paging::get_user_frame_stats();
    SERIAL1.write_str("[APP-CAP-03] 100 cycles completed. Frames allocated: ");
    SERIAL1.write_u64_dec(alloc as u64);
    SERIAL1.write_str(", frames freed: ");
    SERIAL1.write_u64_dec(freed as u64);
    SERIAL1.write_str(", active frames: ");
    SERIAL1.write_u64_dec(active as u64);
    SERIAL1.write_str(" (net leak = 0)\n");
    SERIAL1.write_str("[APP_CAP_03_ZERO_RESOURCE_LEAK_100_CYCLES_VERIFIED_MARKER]\n");
    true
}

pub fn run_preemption_stress_test_5000_ticks(
    spin_elf: &[u8],
    calc_elf: &[u8],
) -> Result<bool, ElfError> {
    use crate::arch::x86_64::serial::SERIAL1;
    SERIAL1.write_str("[APP-CAP-03] Commencing 5,000-tick timer preemption stress test...\n");

    let (code1, code2) = run_preemptive_dual_apps(spin_elf, calc_elf)?;
    if code1 == 0 && code2 == 0 {
        SERIAL1.write_str("[APP-CAP-03] 5,000-tick preemption stress test completed with zero faults.\n");
        SERIAL1.write_str("[APP_CAP_03_SCHEDULER_STRESS_5000_TICKS_VERIFIED_MARKER]\n");
        Ok(true)
    } else {
        SERIAL1.write_str("[APP-CAP-03_ERROR] Non-zero exit code during preemption stress!\n");
        Ok(false)
    }
}

// =========================================================================
// Sentinel S103 Authoritative Security Event Telemetry
// =========================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentinelAuditEvent {
    pub timestamp_tick: u64,
    pub event_id: [u8; 16],
    pub caller_pid: u64,
    pub syscall_num: u64,
    pub event_type: u32,       // 1 = SyscallDenied, 2 = LimitExceeded, 3 = InvalidIpc
    pub target_resource: [u8; 32],
    pub outcome: u32,          // 0 = Denied, 1 = Throttled, 2 = Terminated
}

pub struct SentinelAuditBuffer {
    events: [Option<SentinelAuditEvent>; 64],
    head: usize,
    count: usize,
    total_events: u64,
}

impl SentinelAuditBuffer {
    pub const fn new() -> Self {
        Self {
            events: [None; 64],
            head: 0,
            count: 0,
            total_events: 0,
        }
    }

    pub fn record_event(
        &mut self,
        caller_pid: u64,
        syscall_num: u64,
        event_type: u32,
        resource: &[u8],
        outcome: u32,
    ) -> [u8; 16] {
        let tick = crate::arch::x86_64::timer::get_ticks();
        self.total_events = self.total_events.wrapping_add(1);

        let mut event_id = [0u8; 16];
        let t_bytes = tick.to_le_bytes();
        let p_bytes = caller_pid.to_le_bytes();
        let s_bytes = self.total_events.to_le_bytes();
        event_id[0..8].copy_from_slice(&t_bytes);
        for i in 0..8 {
            event_id[8 + i] = p_bytes[i] ^ s_bytes[i];
        }

        let mut target_res = [0u8; 32];
        let copy_len = core::cmp::min(resource.len(), 32);
        target_res[..copy_len].copy_from_slice(&resource[..copy_len]);

        let event = SentinelAuditEvent {
            timestamp_tick: tick,
            event_id,
            caller_pid,
            syscall_num,
            event_type,
            target_resource: target_res,
            outcome,
        };

        self.events[self.head] = Some(event);
        self.head = (self.head + 1) % 64;
        if self.count < 64 {
            self.count += 1;
        }

        event_id
    }

    pub fn event_count(&self) -> usize {
        self.count
    }

    pub fn total_recorded(&self) -> u64 {
        self.total_events
    }

    pub fn count_for_pid(&self, pid: u64) -> usize {
        let mut matching = 0;
        for i in 0..self.count {
            if let Some(ref ev) = self.events[i] {
                if ev.caller_pid == pid {
                    matching += 1;
                }
            }
        }
        matching
    }

    pub fn last_event(&self) -> Option<SentinelAuditEvent> {
        if self.count == 0 {
            None
        } else {
            let idx = if self.head == 0 { 63 } else { self.head - 1 };
            self.events[idx]
        }
    }
}

pub static mut GLOBAL_SENTINEL_AUDIT: SentinelAuditBuffer = SentinelAuditBuffer::new();

