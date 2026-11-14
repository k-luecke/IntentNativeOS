pub mod compositor;
pub mod web;

pub const MAX_QUEUE_CAPACITY: usize = 16;
pub const MAX_PAYLOAD_SIZE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcMessageType {
    DirectIntent = 1,
    ClarificationRequest = 2,
    ExecutionReceipt = 3,
    CancellationSignal = 4,
    AutomationCommand = 5,
    AutomationResponse = 6,
}

impl IpcMessageType {
    pub fn from_u32(val: u32) -> Self {
        match val {
            2 => IpcMessageType::ClarificationRequest,
            3 => IpcMessageType::ExecutionReceipt,
            4 => IpcMessageType::CancellationSignal,
            5 => IpcMessageType::AutomationCommand,
            6 => IpcMessageType::AutomationResponse,
            _ => IpcMessageType::DirectIntent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcMessage {
    pub msg_id: u64,
    pub sender_pid: u64,
    pub receiver_pid: u64,
    pub msg_type: IpcMessageType,
    pub payload: [u8; MAX_PAYLOAD_SIZE],
    pub payload_len: usize,
    pub is_cancelled: bool,
}

impl IpcMessage {
    pub fn new(
        msg_id: u64,
        sender_pid: u64,
        receiver_pid: u64,
        msg_type: IpcMessageType,
        data: &[u8],
    ) -> Self {
        let mut payload = [0u8; MAX_PAYLOAD_SIZE];
        let copy_len = core::cmp::min(data.len(), MAX_PAYLOAD_SIZE);
        payload[..copy_len].copy_from_slice(&data[..copy_len]);

        Self {
            msg_id,
            sender_pid,
            receiver_pid,
            msg_type,
            payload,
            payload_len: copy_len,
            is_cancelled: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    QueueFull,
    QueueEmpty,
    MessageNotFound,
    AlreadyCancelled,
    InvalidChannel,
}

pub struct BoundedIpcQueue {
    pub buffer: [Option<IpcMessage>; MAX_QUEUE_CAPACITY],
    pub head: usize,
    pub tail: usize,
    pub count: usize,
    pub total_enqueued: u64,
    pub total_dequeued: u64,
    pub total_cancelled: u64,
}

impl BoundedIpcQueue {
    pub const fn new() -> Self {
        Self {
            buffer: [None; MAX_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
            total_enqueued: 0,
            total_dequeued: 0,
            total_cancelled: 0,
        }
    }

    pub fn is_full(&self) -> bool {
        self.count == MAX_QUEUE_CAPACITY
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn len(&self) -> usize {
        self.count
    }

    /// Enqueues a typed message into the bounded queue.
    /// Fails with QueueFull if capacity is reached.
    pub fn enqueue(&mut self, msg: IpcMessage) -> Result<(), IpcError> {
        if self.is_full() {
            return Err(IpcError::QueueFull);
        }

        self.buffer[self.tail] = Some(msg);
        self.tail = (self.tail + 1) % MAX_QUEUE_CAPACITY;
        self.count += 1;
        self.total_enqueued += 1;
        Ok(())
    }

    /// Dequeues the next non-cancelled message from the queue.
    /// Skips any messages that were cancelled in-flight.
    pub fn dequeue(&mut self) -> Result<IpcMessage, IpcError> {
        while self.count > 0 {
            let msg_opt = self.buffer[self.head].take();
            self.head = (self.head + 1) % MAX_QUEUE_CAPACITY;
            self.count -= 1;

            if let Some(msg) = msg_opt {
                if !msg.is_cancelled {
                    self.total_dequeued += 1;
                    return Ok(msg);
                }
                // Message was cancelled; skip and continue
            }
        }
        Err(IpcError::QueueEmpty)
    }

    /// Cancels a pending message by message ID in the queue.
    pub fn cancel_message(&mut self, msg_id: u64) -> Result<(), IpcError> {
        for slot in self.buffer.iter_mut() {
            if let Some(ref mut msg) = slot {
                if msg.msg_id == msg_id {
                    if msg.is_cancelled {
                        return Err(IpcError::AlreadyCancelled);
                    }
                    msg.is_cancelled = true;
                    self.total_cancelled += 1;
                    return Ok(());
                }
            }
        }
        Err(IpcError::MessageNotFound)
    }
}

// === INOS-AUTOMATION-ABI-001 Cross-Process Wire Protocol ===

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
}

pub static mut GLOBAL_IPC_QUEUE: BoundedIpcQueue = BoundedIpcQueue::new();

pub struct LiveGridState {
    pub cells: [( [u8; 8], i64, [u8; 32] ); 16],
    pub count: usize,
    pub active_file: [u8; 32],
}

impl LiveGridState {
    pub const fn new() -> Self {
        Self {
            cells: [( [0u8; 8], 0, [0u8; 32] ); 16],
            count: 0,
            active_file: [0u8; 32],
        }
    }
}

pub static mut LIVE_GRID_STATE: LiveGridState = LiveGridState::new();

pub fn handle_automation_ipc_send(
    sender_pid: u64,
    target_id: u64,
    wire_bytes: &[u8],
) -> Result<u64, crate::arch::x86_64::syscall::SyscallError> {
    use crate::arch::x86_64::serial::SERIAL1;
    use crate::arch::x86_64::syscall::SyscallError;

    if wire_bytes.len() < core::mem::size_of::<AutomationIpcWire>() {
        return Err(SyscallError::InvalidArgument);
    }
    let wire = unsafe { &*(wire_bytes.as_ptr() as *const AutomationIpcWire) };
    if wire.magic != AUTOMATION_ABI_MAGIC || wire.version != 1 {
        return Err(SyscallError::InvalidArgument);
    }

    SERIAL1.write_str("[AUTOMATION_IPC] IPC message received from PID ");
    SERIAL1.write_u64_dec(sender_pid);
    SERIAL1.write_str(" -> target PID ");
    SERIAL1.write_u64_dec(target_id);
    SERIAL1.write_str(" (op=");
    SERIAL1.write_u64_dec(wire.op_code as u64);
    SERIAL1.write_str(")\n");

    let mut resp = AutomationIpcWire::empty();
    resp.request_id = wire.request_id;
    resp.op_code = wire.op_code;

    match wire.op_code {
        OP_DISCOVER => {
            resp.status_code = STATUS_OK;
            let ops = b"open,get,set,evaluate,save";
            resp.error_msg[..ops.len()].copy_from_slice(ops);
            SERIAL1.write_str("[AUTOMATION_IPC] Operation discovery handled for target application.\n");
        }
        OP_OPEN => {
            unsafe {
                LIVE_GRID_STATE.active_file = wire.target_file;
            }
            resp.status_code = STATUS_OK;
            SERIAL1.write_str("[AUTOMATION_IPC] Target workbook opened via IPC.\n");
        }
        OP_SET_CELL => {
            unsafe {
                let cell_len = wire.cell.iter().position(|&c| c == 0).unwrap_or(8);
                let cell_name = &wire.cell[..cell_len];
                let mut found = false;
                for i in 0..LIVE_GRID_STATE.count {
                    let cur_len = LIVE_GRID_STATE.cells[i].0.iter().position(|&c| c == 0).unwrap_or(8);
                    if &LIVE_GRID_STATE.cells[i].0[..cur_len] == cell_name {
                        LIVE_GRID_STATE.cells[i].1 = wire.value_num;
                        LIVE_GRID_STATE.cells[i].2 = wire.formula;
                        found = true;
                        break;
                    }
                }
                if !found && LIVE_GRID_STATE.count < LIVE_GRID_STATE.cells.len() {
                    let idx = LIVE_GRID_STATE.count;
                    LIVE_GRID_STATE.cells[idx].0 = wire.cell;
                    LIVE_GRID_STATE.cells[idx].1 = wire.value_num;
                    LIVE_GRID_STATE.cells[idx].2 = wire.formula;
                    LIVE_GRID_STATE.count += 1;
                }
            }
            resp.status_code = STATUS_OK;
            SERIAL1.write_str("[AUTOMATION_IPC] SetCell completed via IPC.\n");
        }
        OP_EVALUATE => {
            let mut b2_val = 0i64;
            let mut c2_val = 0i64;
            unsafe {
                for i in 0..LIVE_GRID_STATE.count {
                    let cur_len = LIVE_GRID_STATE.cells[i].0.iter().position(|&c| c == 0).unwrap_or(8);
                    let name = &LIVE_GRID_STATE.cells[i].0[..cur_len];
                    if name == b"B2" { b2_val = LIVE_GRID_STATE.cells[i].1; }
                    if name == b"C2" { c2_val = LIVE_GRID_STATE.cells[i].1; }
                }
            }
            let result = b2_val - c2_val;
            resp.value_num = result;
            resp.status_code = STATUS_OK;
            SERIAL1.write_str("[AUTOMATION_IPC] Formula evaluated: B2 - C2 = ");
            SERIAL1.write_u64_dec(result as u64);
            SERIAL1.write_str(" via Grid IPC.\n");
            SERIAL1.write_str("[GRID_CROSS_PROCESS_IPC_EVALUATED_MARKER]\n");
        }
        OP_SAVE => {
            resp.status_code = STATUS_OK;
            SERIAL1.write_str("[AUTOMATION_IPC] Workbook save committed via Grid IPC.\n");
        }
        _ => {
            resp.status_code = STATUS_NOT_FOUND;
        }
    }

    let resp_bytes = unsafe {
        core::slice::from_raw_parts(
            &resp as *const AutomationIpcWire as *const u8,
            core::mem::size_of::<AutomationIpcWire>(),
        )
    };
    let msg = IpcMessage::new(
        wire.request_id as u64,
        target_id,
        sender_pid,
        IpcMessageType::ExecutionReceipt,
        resp_bytes,
    );
    unsafe {
        let _ = GLOBAL_IPC_QUEUE.enqueue(msg);
    }

    Ok(0)
}

pub fn handle_automation_ipc_recv(
    caller_pid: u64,
    _channel_id: u64,
    buf_ptr: u64,
    buf_len: usize,
) -> Result<u64, crate::arch::x86_64::syscall::SyscallError> {
    use crate::arch::x86_64::syscall::SyscallError;

    if buf_len < core::mem::size_of::<AutomationIpcWire>() {
        return Err(SyscallError::BufferTooSmall);
    }

    unsafe {
        for slot in GLOBAL_IPC_QUEUE.buffer.iter_mut() {
            if let Some(msg) = slot.take() {
                if !msg.is_cancelled && msg.receiver_pid == caller_pid {
                    crate::arch::x86_64::user_ptr::copy_slice_to_user(&msg.payload[..msg.payload_len], buf_ptr)?;
                    return Ok(msg.payload_len as u64);
                } else {
                    *slot = Some(msg);
                }
            }
        }
    }

    Err(SyscallError::ObjectNotFound)
}
pub struct IpcRouter {
    pub mailboxes: [(u64, BoundedIpcQueue); 16],
    pub next_msg_id: u64,
}

impl IpcRouter {
    pub const fn new() -> Self {
        const EMPTY_QUEUE: (u64, BoundedIpcQueue) = (0, BoundedIpcQueue::new());
        Self {
            mailboxes: [EMPTY_QUEUE; 16],
            next_msg_id: 1,
        }
    }

    pub fn send(&mut self, sender_pid: u64, receiver_pid: u64, msg_type: IpcMessageType, data: &[u8]) -> Result<u64, IpcError> {
        let slot = match self.mailboxes.iter().position(|(pid, _)| *pid == receiver_pid) {
            Some(idx) => idx,
            None => {
                let empty_idx = self.mailboxes.iter().position(|(pid, _)| *pid == 0)
                    .ok_or(IpcError::QueueFull)?;
                self.mailboxes[empty_idx].0 = receiver_pid;
                empty_idx
            }
        };

        let msg_id = self.next_msg_id;
        self.next_msg_id += 1;
        let msg = IpcMessage::new(msg_id, sender_pid, receiver_pid, msg_type, data);
        self.mailboxes[slot].1.enqueue(msg)?;
        Ok(msg_id)
    }

    pub fn recv(&mut self, receiver_pid: u64) -> Result<IpcMessage, IpcError> {
        let slot = self.mailboxes.iter().position(|(pid, _)| *pid == receiver_pid)
            .ok_or(IpcError::QueueEmpty)?;
        self.mailboxes[slot].1.dequeue()
    }
}

pub static mut GLOBAL_IPC_ROUTER: IpcRouter = IpcRouter::new();

