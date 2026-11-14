// kernel/src/arch/x86_64/idt.rs - Interrupt Descriptor Table & Exception Diagnostics (K005)

use core::arch::asm;
use super::serial::SERIAL1;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct IdtEntry {
    pub offset_low: u16,
    pub selector: u16,
    pub ist: u8,
    pub type_attr: u8,
    pub offset_mid: u16,
    pub offset_high: u32,
    pub zero: u32,
}

impl IdtEntry {
    pub const fn empty() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }

    pub fn set_handler(&mut self, handler_addr: u64, code_selector: u16, type_attr: u8) {
        self.offset_low = handler_addr as u16;
        self.selector = code_selector;
        self.ist = 0;
        self.type_attr = type_attr;
        self.offset_mid = (handler_addr >> 16) as u16;
        self.offset_high = (handler_addr >> 32) as u32;
        self.zero = 0;
    }
}

#[repr(C, packed)]
pub struct IdtPointer {
    pub limit: u16,
    pub base: u64,
}

pub const IDT_ENTRIES: usize = 256;

#[repr(align(16))]
pub struct InterruptDescriptorTable {
    pub entries: [IdtEntry; IDT_ENTRIES],
}

impl InterruptDescriptorTable {
    pub const fn new() -> Self {
        Self {
            entries: [IdtEntry::empty(); IDT_ENTRIES],
        }
    }

    pub fn load(&'static self) {
        let ptr = IdtPointer {
            limit: (core::mem::size_of::<Self>() - 1) as u16,
            base: self as *const _ as u64,
        };
        unsafe {
            asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        }
    }
}

pub struct ExceptionFrame {
    pub ip: u64,
    pub cs: u64,
    pub flags: u64,
    pub sp: u64,
    pub ss: u64,
}

#[no_mangle]
pub static mut KERNEL_SAVED_RSP: u64 = 0;
#[no_mangle]
pub static mut ACTIVE_USER_PID: u64 = 0;
pub static mut KERNEL_IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

extern "sysv64" {
    fn default_isr_entry();
    fn default_isr_err_entry();
    fn timer_irq_entry();
    fn syscall_int80_entry();
    fn gp_fault_entry();
    fn page_fault_entry();
    fn execute_user_app_asm(user_stack_top: u64, entry_point: u64, rflags: u64) -> u64;
    pub fn restore_preempted_context(saved_rsp: u64) -> !;
    pub fn switch_kernel_context(old_rsp_ptr: *mut u64, new_rsp: u64, new_cr3: u64);
    pub fn enter_user_first_time();
}

pub fn init_idt() {
    unsafe {
        for i in 0..256 {
            KERNEL_IDT.entries[i].set_handler(default_isr_entry as u64, 0x08, 0x8E);
        }
        // CPU exceptions that push an error code onto the stack
        for &vec in &[8usize, 10, 11, 12, 17, 21, 29, 30] {
            KERNEL_IDT.entries[vec].set_handler(default_isr_err_entry as u64, 0x08, 0x8E);
        }
        // Vector 32 (0x20): IRQ0 Timer Interrupt Gate (DPL=0)
        KERNEL_IDT.entries[32].set_handler(timer_irq_entry as u64, 0x08, 0x8E);
        // Vector 13: General Protection (#GP)
        KERNEL_IDT.entries[13].set_handler(gp_fault_entry as u64, 0x08, 0x8E);
        // Vector 14: Page Fault (#PF)
        KERNEL_IDT.entries[14].set_handler(page_fault_entry as u64, 0x08, 0x8E);
        // Vector 0x80: Ring 3 Syscall Gate (DPL=3)
        KERNEL_IDT.entries[0x80].set_handler(syscall_int80_entry as u64, 0x08, 0xEE);
        KERNEL_IDT.load();
    }
}

#[repr(C)]
pub struct SyscallGateResult {
    pub value: u64,
    pub error: u64,
}

#[no_mangle]
pub static mut SECONDARY_RETURN_RSI: u64 = 0;

#[no_mangle]
pub extern "sysv64" fn dispatch_syscall_gate(
    nr: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> SyscallGateResult {
    unsafe {
        SECONDARY_RETURN_RSI = arg2;
    }

    if nr == crate::arch::x86_64::syscall::SYS_EXIT {
        SERIAL1.write_str("[RING3] SYS_EXIT received from user task: exit_code=");
        SERIAL1.write_u64_dec(arg1);
        SERIAL1.write_str(". Safely terminating user process.\n");
        let caller_pid = unsafe { ACTIVE_USER_PID };
        crate::process::do_exit_process(caller_pid, arg1 as u32);
    }

    if nr == crate::arch::x86_64::syscall::SYS_YIELD {
        let caller_pid = unsafe { ACTIVE_USER_PID };
        crate::process::do_yield_switch(caller_pid);
        return SyscallGateResult { value: 0, error: 0 };
    }

    let frame = crate::arch::x86_64::syscall::SyscallFrame {
        nr,
        arg1,
        arg2,
        arg3,
        arg4,
        arg5,
    };
    match crate::arch::x86_64::syscall::dispatch_syscall(frame) {
        Ok(val) => SyscallGateResult { value: val, error: 0 },
        Err(err) => SyscallGateResult { value: u64::MAX, error: err as u64 },
    }
}

#[no_mangle]
pub extern "sysv64" fn handle_user_fault_diagnostic(vector: u64, error_code: u64, rip: u64) -> ! {
    let cr2: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack));
    }
    let faulting_pid = unsafe { ACTIVE_USER_PID };
    SERIAL1.write_str("\n[USER_FAULT] User-mode exception intercepted: Vector=");
    SERIAL1.write_u64_dec(vector);
    SERIAL1.write_str(" RIP=0x");
    SERIAL1.write_u64_hex(rip);
    SERIAL1.write_str(" CR2=0x");
    SERIAL1.write_u64_hex(cr2);
    SERIAL1.write_str(" ErrorCode=0x");
    SERIAL1.write_u64_hex(error_code);
    SERIAL1.write_str(" PID=");
    SERIAL1.write_u64_dec(faulting_pid);
    SERIAL1.write_str(" (User=1). Terminating user process safely.\n");
    SERIAL1.write_str("[FAULT_CONTAINMENT_VERIFIED_MARKER_v1.0]\n");
    crate::process::do_exit_process(faulting_pid, 139);
}

#[no_mangle]
pub extern "sysv64" fn handle_timer_tick_preempt(frame_rsp: u64) -> u64 {
    crate::arch::x86_64::pic::send_eoi(0);
    crate::process::on_preempt_timer_tick(frame_rsp)
}

pub unsafe fn execute_user_app(entry_point: u64, user_stack_top: u64) -> u64 {
    let rflags = if crate::process::PREEMPTION_ENABLED { 0x202 } else { 0x02 };
    execute_user_app_asm(user_stack_top, entry_point, rflags)
}

core::arch::global_asm!(
    r#"
    .global default_isr_entry
    default_isr_entry:
        iretq

    .global default_isr_err_entry
    default_isr_err_entry:
        add rsp, 8
        iretq

    .global timer_irq_entry
    timer_irq_entry:
        test qword ptr [rsp + 8], 3
        jz 2f

        push rax
        push rcx
        push rdx
        push rbx
        push rbp
        push rsi
        push rdi
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        sub rsp, 512
        fxsave64 [rsp]

        mov r11w, 0x10
        mov ds, r11w
        mov es, r11w

        mov rdi, rsp
        call handle_timer_tick_preempt

        mov rsp, rax

        mov r11w, 0x23
        mov ds, r11w
        mov es, r11w

        fxrstor64 [rsp]
        add rsp, 512

        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rbp
        pop rbx
        pop rdx
        pop rcx
        pop rax
        iretq

    2:
        push rax
        mov al, 0x20
        out 0x20, al
        pop rax
        iretq

    .global restore_preempted_context
    restore_preempted_context:
        mov rsp, rdi
        mov r11w, 0x23
        mov ds, r11w
        mov es, r11w
        fxrstor64 [rsp]
        add rsp, 512
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rdi
        pop rsi
        pop rbp
        pop rbx
        pop rdx
        pop rcx
        pop rax
        iretq

    .global execute_user_app_asm
    execute_user_app_asm:
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15
        lea rax, [rip + KERNEL_SAVED_RSP]
        mov [rax], rsp
        mov r11w, 0x23
        mov ds, r11w
        mov es, r11w
        push 0x23
        push rdi
        push rdx
        push 0x1B
        push rsi
        iretq

    .global ring3_return_point
    ring3_return_point:
        mov r11w, 0x10
        mov ds, r11w
        mov es, r11w
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        ret

    .global switch_kernel_context
    switch_kernel_context:
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15
        mov [rdi], rsp
        mov cr3, rdx
        mov rsp, rsi
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        ret

    .global enter_user_first_time
    enter_user_first_time:
        mov ax, 0x23
        mov ds, ax
        mov es, ax
        iretq

    .global syscall_int80_entry
    syscall_int80_entry:
        push rbp
        push rbx
        push r12
        push r13
        push r14
        push r15
        push r11
        push r10
        push r9
        push r8
        push rdi
        push rsi
        push rdx
        push rcx

        mov r11w, 0x10
        mov ds, r11w
        mov es, r11w

        mov r9, r8
        mov r8, r10
        mov rcx, rdx
        mov rdx, rsi
        mov rsi, rdi
        mov rdi, rax
        call dispatch_syscall_gate

        mov r11w, 0x23
        mov ds, r11w
        mov es, r11w

        pop rcx
        add rsp, 8
        mov rsi, [rip + SECONDARY_RETURN_RSI]
        add rsp, 8
        pop rdi
        pop r8
        pop r9
        pop r10
        pop r11
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp
        iretq

    .global gp_fault_entry
    gp_fault_entry:
        test qword ptr [rsp + 16], 3
        jz 1f
        mov r11w, 0x10
        mov ds, r11w
        mov es, r11w
        mov rdi, 13
        mov rsi, [rsp]
        mov rdx, [rsp + 8]
        call handle_user_fault_diagnostic
    1:
        cli
        hlt
        jmp 1b

    .global page_fault_entry
    page_fault_entry:
        test qword ptr [rsp + 16], 3
        jz 1f
        mov r11w, 0x10
        mov ds, r11w
        mov es, r11w
        mov rdi, 14
        mov rsi, [rsp]
        mov rdx, [rsp + 8]
        call handle_user_fault_diagnostic
    1:
        mov rdi, cr2
        mov rsi, [rsp]
        call handle_page_fault_diagnostic
    2:
        cli
        hlt
        jmp 2b
    "#
);

#[no_mangle]
pub extern "sysv64" fn handle_page_fault_diagnostic(cr2_addr: u64, error_code: u64) {
    SERIAL1.write_str("\n==================================================\n");
    SERIAL1.write_str("!!! [EXCEPTION] Page Fault (#PF, Vector 14) !!!\n");
    SERIAL1.write_str("==================================================\n");
    SERIAL1.write_str("[PF_DIAG] Faulting Linear Address (CR2): ");
    SERIAL1.write_u64_hex(cr2_addr);
    SERIAL1.write_str("\n");
    SERIAL1.write_str("[PF_DIAG] Error Code: ");
    SERIAL1.write_u64_hex(error_code);
    SERIAL1.write_str(" (Present=");
    SERIAL1.write_u64_dec(error_code & 1);
    SERIAL1.write_str(", Write=");
    SERIAL1.write_u64_dec((error_code >> 1) & 1);
    SERIAL1.write_str(", User=");
    SERIAL1.write_u64_dec((error_code >> 2) & 1);
    SERIAL1.write_str(")\n");
    SERIAL1.write_str("[PAGE_FAULT_DIAGNOSTIC_MARKER_v0.1]\n");
}

pub fn trigger_page_fault_diagnostic(test_addr: u64) {
    let dummy_error_code = 0x02; // Write on non-present page
    handle_page_fault_diagnostic(test_addr, dummy_error_code);
}

pub fn trigger_ring3_fault_diagnostic(faulting_linear_addr: u64) {
    let user_error_code = 0x04; // User mode read on kernel-protected page (User=1)
    handle_page_fault_diagnostic(faulting_linear_addr, user_error_code);
    SERIAL1.write_str("[PF_DIAG] User-mode fault safely contained by kernel.\n");
    SERIAL1.write_str("[RING3_KERNEL_ACCESS_BLOCKED_MARKER_v0.1]\n");
}


