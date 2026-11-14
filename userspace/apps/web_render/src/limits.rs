// userspace/apps/web_render/src/limits.rs
// Deterministic Resource Accounting & Fail-Closed Parser Limits (Milestone WEB-02B)

pub const MAX_INPUT_BYTES: usize = 512 * 1024; // 512 KiB max document payload
pub const MAX_TOKENS: usize = 8192;           // Maximum tokens allowed per document
pub const MAX_DOM_NODES: usize = 2048;         // Maximum nodes in DOM arena
pub const MAX_TREE_DEPTH: usize = 64;          // Maximum element nesting level
pub const MAX_ATTR_NAME_LEN: usize = 64;       // 64 bytes max attribute name
pub const MAX_ATTR_VALUE_LEN: usize = 1024;    // 1024 bytes max attribute value
pub const MAX_ATTRIBUTES_PER_ELEMENT: usize = 32; // Maximum attributes on one tag
pub const MAX_WORK_OPERATIONS: usize = 100_000; // Deterministic operation budget

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    InputBytes,
    Tokens,
    DomNodes,
    TreeDepth,
    AttrNameLength,
    AttrValueLength,
    AttributeCount,
    WorkBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    LimitExceeded(LimitKind),
    MalformedInput(&'static str),
    OutOfMemory,
}

#[derive(Debug, Clone)]
pub struct WorkBudgetTracker {
    operations_charged: usize,
    budget_limit: usize,
}

impl WorkBudgetTracker {
    pub const fn new(limit: usize) -> Self {
        Self {
            operations_charged: 0,
            budget_limit: limit,
        }
    }

    #[inline]
    pub fn charge(&mut self, ops: usize) -> Result<(), ParseError> {
        let new_total = self.operations_charged.checked_add(ops)
            .ok_or(ParseError::LimitExceeded(LimitKind::WorkBudget))?;
        if new_total > self.budget_limit {
            return Err(ParseError::LimitExceeded(LimitKind::WorkBudget));
        }
        self.operations_charged = new_total;
        Ok(())
    }

    #[inline]
    pub fn ops_used(&self) -> usize {
        self.operations_charged
    }
}
