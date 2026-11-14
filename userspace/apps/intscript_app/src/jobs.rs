// userspace/apps/script_app/src/jobs.rs - Persistent Job Queue & Event Store

pub const TRIGGER_MANUAL: u8 = 0;
pub const TRIGGER_INTERVAL: u8 = 1;
pub const TRIGGER_TIMESTAMP: u8 = 2;
pub const TRIGGER_FILE_CHANGED: u8 = 3;
pub const TRIGGER_WORKBOOK_CHANGED: u8 = 4;

pub const STATUS_PENDING: u8 = 0;
pub const STATUS_RUNNING: u8 = 1;
pub const STATUS_SUCCESS: u8 = 2;
pub const STATUS_FAILED: u8 = 3;
pub const STATUS_CANCELLED: u8 = 4;
pub const STATUS_MISSED: u8 = 5;

pub const MISSED_POLICY_RUN_IMMEDIATE: u8 = 0;
pub const MISSED_POLICY_SKIP: u8 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobRecord {
    pub job_id: u32,
    pub trigger_type: u8,
    pub is_active: u8,
    pub status: u8,
    pub missed_policy: u8,
    pub retry_count: u8,
    pub max_retries: u8,
    pub reserved: u16,
    pub schedule_time: u64,
    pub interval_seconds: u32,
    pub last_run_time: u64,
    pub script_id: [u8; 32],
    pub target_len: u16,
    pub target_path: [u8; 64],
    pub script_len: u16,
    pub script_path: [u8; 64],
}

impl JobRecord {
    pub const fn empty() -> Self {
        Self {
            job_id: 0,
            trigger_type: 0,
            is_active: 0,
            status: STATUS_PENDING,
            missed_policy: MISSED_POLICY_RUN_IMMEDIATE,
            retry_count: 0,
            max_retries: 3,
            reserved: 0,
            schedule_time: 0,
            interval_seconds: 0,
            last_run_time: 0,
            script_id: [0u8; 32],
            target_len: 0,
            target_path: [0u8; 64],
            script_len: 0,
            script_path: [0u8; 64],
        }
    }
}

pub struct JobDatabase {
    pub count: u32,
    pub records: [JobRecord; 16],
}

impl JobDatabase {
    pub const fn new() -> Self {
        Self {
            count: 0,
            records: [JobRecord::empty(); 16],
        }
    }

    pub fn add_job(&mut self, trigger_type: u8, target: &str, script: &str) -> u32 {
        self.add_job_ext(
            trigger_type,
            target,
            script,
            0,
            0,
            MISSED_POLICY_RUN_IMMEDIATE,
            &[0u8; 32],
        )
    }

    pub fn add_job_ext(
        &mut self,
        trigger_type: u8,
        target: &str,
        script: &str,
        schedule_time: u64,
        interval_seconds: u32,
        missed_policy: u8,
        script_id: &[u8; 32],
    ) -> u32 {
        if (self.count as usize) < self.records.len() {
            let id = self.count + 1;
            let mut rec = JobRecord::empty();
            rec.job_id = id;
            rec.trigger_type = trigger_type;
            rec.is_active = 1;
            rec.status = STATUS_PENDING;
            rec.schedule_time = schedule_time;
            rec.interval_seconds = interval_seconds;
            rec.missed_policy = missed_policy;
            rec.retry_count = 0;
            rec.max_retries = 3;
            rec.script_id.copy_from_slice(script_id);

            let t_bytes = target.as_bytes();
            let t_len = core::cmp::min(t_bytes.len(), 64);
            rec.target_path[..t_len].copy_from_slice(&t_bytes[..t_len]);
            rec.target_len = t_len as u16;

            let s_bytes = script.as_bytes();
            let s_len = core::cmp::min(s_bytes.len(), 64);
            rec.script_path[..s_len].copy_from_slice(&s_bytes[..s_len]);
            rec.script_len = s_len as u16;

            self.records[self.count as usize] = rec;
            self.count += 1;
            id
        } else {
            0
        }
    }

    pub fn cancel_job(&mut self, job_id: u32) -> bool {
        for i in 0..self.count as usize {
            if self.records[i].job_id == job_id {
                self.records[i].is_active = 0;
                self.records[i].status = STATUS_CANCELLED;
                return true;
            }
        }
        false
    }

    pub fn mark_status(&mut self, job_id: u32, status: u8, run_time: u64) {
        for i in 0..self.count as usize {
            if self.records[i].job_id == job_id {
                self.records[i].status = status;
                self.records[i].last_run_time = run_time;
                if status == STATUS_FAILED {
                    self.records[i].retry_count += 1;
                    if self.records[i].retry_count >= self.records[i].max_retries {
                        self.records[i].is_active = 0;
                    }
                } else if status == STATUS_SUCCESS {
                    if self.records[i].trigger_type != TRIGGER_INTERVAL {
                        self.records[i].is_active = 0;
                    }
                }
                break;
            }
        }
    }

    pub fn find_due_job(&self, current_time: u64) -> Option<usize> {
        for i in 0..self.count as usize {
            let rec = &self.records[i];
            if rec.is_active == 1 && rec.status != STATUS_RUNNING && rec.status != STATUS_CANCELLED {
                if rec.trigger_type == TRIGGER_TIMESTAMP {
                    if current_time >= rec.schedule_time {
                        return Some(i);
                    }
                } else if rec.trigger_type == TRIGGER_INTERVAL {
                    if rec.last_run_time == 0 || (current_time >= rec.last_run_time + rec.interval_seconds as u64) {
                        return Some(i);
                    }
                } else if rec.trigger_type == TRIGGER_WORKBOOK_CHANGED || rec.trigger_type == TRIGGER_FILE_CHANGED || rec.trigger_type == TRIGGER_MANUAL {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn suppress_duplicate(&mut self, trigger_type: u8, target: &str, current_time: u64, window: u64) -> bool {
        let t_bytes = target.as_bytes();
        for i in 0..self.count as usize {
            let rec = &self.records[i];
            if rec.trigger_type == trigger_type
                && (rec.target_len as usize) == t_bytes.len()
                && &rec.target_path[..rec.target_len as usize] == t_bytes
            {
                if rec.last_run_time != 0 && (current_time < rec.last_run_time + window) {
                    return true; // Duplicate suppressed!
                }
            }
        }
        false
    }

    pub fn serialize(&self, buf: &mut [u8]) -> usize {
        let mut pos = 0usize;
        let magic = b"INOS_JOB_DB_v1.0";
        buf[pos..pos + magic.len()].copy_from_slice(magic);
        pos += magic.len();

        buf[pos..pos + 4].copy_from_slice(&self.count.to_le_bytes());
        pos += 4;

        let rec_size = core::mem::size_of::<JobRecord>();
        for i in 0..self.count as usize {
            let bytes = unsafe {
                core::slice::from_raw_parts(&self.records[i] as *const JobRecord as *const u8, rec_size)
            };
            buf[pos..pos + rec_size].copy_from_slice(bytes);
            pos += rec_size;
        }

        pos
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        let magic = b"INOS_JOB_DB_v1.0";
        if buf.len() < magic.len() + 4 || &buf[..magic.len()] != magic {
            return None;
        }
        let mut pos = magic.len();
        let count = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        pos += 4;

        let mut db = Self::new();
        db.count = count;
        let rec_size = core::mem::size_of::<JobRecord>();

        for i in 0..count as usize {
            if pos + rec_size > buf.len() {
                break;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf[pos..pos + rec_size].as_ptr(),
                    &mut db.records[i] as *mut JobRecord as *mut u8,
                    rec_size,
                );
            }
            pos += rec_size;
        }

        Some(db)
    }
}

// === Durable Execution Receipt Ledger ===

pub const RECEIPT_MAGIC: &[u8; 16] = b"INOS_RCPT_DB_v10";

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiptRecord {
    pub receipt_id: u32,
    pub job_id: u32,
    pub trigger_type: u8,
    pub status: u8, // 2 = Success, 3 = Failed, 4 = Cancelled, 5 = Missed
    pub exit_code: i16,
    pub reserved: u32,
    pub timestamp: u64,
    pub instructions_executed: u64,
    pub output_hash: [u8; 32],
}

impl ReceiptRecord {
    pub const fn empty() -> Self {
        Self {
            receipt_id: 0,
            job_id: 0,
            trigger_type: 0,
            status: 0,
            exit_code: 0,
            reserved: 0,
            timestamp: 0,
            instructions_executed: 0,
            output_hash: [0u8; 32],
        }
    }
}

pub struct ReceiptDatabase {
    pub count: u32,
    pub records: [ReceiptRecord; 16],
}

impl ReceiptDatabase {
    pub const fn new() -> Self {
        Self {
            count: 0,
            records: [ReceiptRecord::empty(); 16],
        }
    }

    pub fn add_receipt(
        &mut self,
        job_id: u32,
        trigger_type: u8,
        status: u8,
        exit_code: i16,
        timestamp: u64,
        instructions: u64,
        output_hash: &[u8; 32],
    ) -> u32 {
        if (self.count as usize) < self.records.len() {
            let id = self.count + 1;
            let mut rec = ReceiptRecord::empty();
            rec.receipt_id = id;
            rec.job_id = job_id;
            rec.trigger_type = trigger_type;
            rec.status = status;
            rec.exit_code = exit_code;
            rec.timestamp = timestamp;
            rec.instructions_executed = instructions;
            rec.output_hash.copy_from_slice(output_hash);

            self.records[self.count as usize] = rec;
            self.count += 1;
            id
        } else {
            0
        }
    }

    pub fn serialize(&self, buf: &mut [u8]) -> usize {
        let mut pos = 0usize;
        buf[pos..pos + RECEIPT_MAGIC.len()].copy_from_slice(RECEIPT_MAGIC);
        pos += RECEIPT_MAGIC.len();

        buf[pos..pos + 4].copy_from_slice(&self.count.to_le_bytes());
        pos += 4;

        let rec_size = core::mem::size_of::<ReceiptRecord>();
        for i in 0..self.count as usize {
            let bytes = unsafe {
                core::slice::from_raw_parts(&self.records[i] as *const ReceiptRecord as *const u8, rec_size)
            };
            buf[pos..pos + rec_size].copy_from_slice(bytes);
            pos += rec_size;
        }

        pos
    }

    pub fn deserialize(buf: &[u8]) -> Option<Self> {
        if buf.len() < RECEIPT_MAGIC.len() + 4 || &buf[..RECEIPT_MAGIC.len()] != RECEIPT_MAGIC {
            return None;
        }
        let mut pos = RECEIPT_MAGIC.len();
        let count = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        pos += 4;

        let mut db = Self::new();
        db.count = count;
        let rec_size = core::mem::size_of::<ReceiptRecord>();

        for i in 0..count as usize {
            if pos + rec_size > buf.len() {
                break;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf[pos..pos + rec_size].as_ptr(),
                    &mut db.records[i] as *mut ReceiptRecord as *mut u8,
                    rec_size,
                );
            }
            pos += rec_size;
        }

        Some(db)
    }
}
