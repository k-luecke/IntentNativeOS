// kernel/src/arch/x86_64/scheduler.rs - Per-Core Task Scheduler with Runnable Queues (K010)

pub const MAX_TASKS_PER_CORE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Yielded,
    Blocked,
    Terminated,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TaskContext {
    pub rsp: u64,
    pub rbp: u64,
    pub rip: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rflags: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct TaskControlBlock {
    pub id: u64,
    pub name: [u8; 16],
    pub name_len: usize,
    pub state: TaskState,
    pub priority: u8,
    pub ticks_run: u64,
    pub yield_count: u64,
    pub context: TaskContext,
}

impl TaskControlBlock {
    pub const fn empty() -> Self {
        Self {
            id: 0,
            name: [0u8; 16],
            name_len: 0,
            state: TaskState::Terminated,
            priority: 0,
            ticks_run: 0,
            yield_count: 0,
            context: TaskContext {
                rsp: 0,
                rbp: 0,
                rip: 0,
                rbx: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0,
                rflags: 0x202, // IF (Interrupt Flag) enabled
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    QueueFull,
    TaskNotFound,
    NoRunnableTasks,
}

pub struct PerCoreScheduler {
    pub core_id: u32,
    pub tasks: [Option<TaskControlBlock>; MAX_TASKS_PER_CORE],
    pub current_idx: Option<usize>,
    pub task_count: usize,
    pub total_context_switches: u64,
}

impl PerCoreScheduler {
    pub const fn new(core_id: u32) -> Self {
        Self {
            core_id,
            tasks: [None; MAX_TASKS_PER_CORE],
            current_idx: None,
            task_count: 0,
            total_context_switches: 0,
        }
    }

    /// Spawns a new task onto this core's runnable queue
    pub fn spawn_task(&mut self, id: u64, name_str: &str, priority: u8) -> Result<usize, SchedulerError> {
        for idx in 0..MAX_TASKS_PER_CORE {
            if self.tasks[idx].is_none() {
                let mut name_bytes = [0u8; 16];
                let copy_len = core::cmp::min(name_str.len(), 16);
                name_bytes[..copy_len].copy_from_slice(&name_str.as_bytes()[..copy_len]);

                let tcb = TaskControlBlock {
                    id,
                    name: name_bytes,
                    name_len: copy_len,
                    state: TaskState::Ready,
                    priority,
                    ticks_run: 0,
                    yield_count: 0,
                    context: TaskContext {
                        rsp: 0,
                        rbp: 0,
                        rip: 0,
                        rbx: 0,
                        r12: 0,
                        r13: 0,
                        r14: 0,
                        r15: 0,
                        rflags: 0x202,
                    },
                };

                self.tasks[idx] = Some(tcb);
                self.task_count += 1;
                return Ok(idx);
            }
        }
        Err(SchedulerError::QueueFull)
    }

    /// Selects next runnable task in round-robin fashion
    pub fn schedule_next(&mut self) -> Result<usize, SchedulerError> {
        if self.task_count == 0 {
            return Err(SchedulerError::NoRunnableTasks);
        }

        let start_idx = match self.current_idx {
            Some(curr) => (curr + 1) % MAX_TASKS_PER_CORE,
            None => 0,
        };

        for offset in 0..MAX_TASKS_PER_CORE {
            let candidate_idx = (start_idx + offset) % MAX_TASKS_PER_CORE;
            if let Some(ref mut task) = self.tasks[candidate_idx] {
                if task.state == TaskState::Ready || task.state == TaskState::Yielded {
                    task.state = TaskState::Running;
                    self.current_idx = Some(candidate_idx);
                    self.total_context_switches += 1;
                    return Ok(candidate_idx);
                }
            }
        }

        // If current task is still Running, continue running it
        if let Some(curr) = self.current_idx {
            if let Some(ref task) = self.tasks[curr] {
                if task.state == TaskState::Running {
                    return Ok(curr);
                }
            }
        }

        Err(SchedulerError::NoRunnableTasks)
    }

    /// Cooperative yield by current task: updates counters and switches to next ready task
    pub fn yield_current(&mut self) -> Result<(usize, usize), SchedulerError> {
        let from_idx = self.current_idx.ok_or(SchedulerError::NoRunnableTasks)?;

        if let Some(ref mut task) = self.tasks[from_idx] {
            task.state = TaskState::Yielded;
            task.yield_count += 1;
            task.ticks_run += 1;
        }

        let next_idx = self.schedule_next()?;
        Ok((from_idx, next_idx))
    }

    /// Simulates ticks for the currently executing task
    pub fn tick_current(&mut self, ticks: u64) {
        if let Some(curr) = self.current_idx {
            if let Some(ref mut task) = self.tasks[curr] {
                task.ticks_run += ticks;
            }
        }
    }

    /// Terminates current task
    pub fn terminate_current(&mut self) {
        if let Some(curr) = self.current_idx {
            if let Some(ref mut task) = self.tasks[curr] {
                task.state = TaskState::Terminated;
            }
            self.current_idx = None;
            self.task_count = self.task_count.saturating_sub(1);
        }
    }
}
