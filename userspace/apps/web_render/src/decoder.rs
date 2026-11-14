// userspace/apps/web_render/src/decoder.rs
// Streaming UTF-8 Decoder with Strict Incomplete vs. Invalid Sequence Semantics (Milestone WEB-02B)

extern crate alloc;

use alloc::vec::Vec;
use crate::limits::{LimitKind, ParseError, WorkBudgetTracker};

pub const REPLACEMENT_CHAR: char = '\u{FFFD}';

pub struct Utf8StreamDecoder {
    pending_buf: [u8; 4],
    pending_len: usize,
    total_bytes_processed: usize,
    max_input_bytes: usize,
}

impl Utf8StreamDecoder {
    pub const fn new(max_input_bytes: usize) -> Self {
        Self {
            pending_buf: [0u8; 4],
            pending_len: 0,
            total_bytes_processed: 0,
            max_input_bytes,
        }
    }

    /// Feeds a chunk of raw bytes, decoding complete UTF-8 codepoints into `out_chars`.
    /// Incomplete sequences at the chunk boundary are preserved in `pending_buf`.
    /// Invalid sequences are deterministically replaced with U+FFFD.
    pub fn feed_chunk(
        &mut self,
        chunk: &[u8],
        out_chars: &mut Vec<char>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        let new_total = self.total_bytes_processed.checked_add(chunk.len())
            .ok_or(ParseError::LimitExceeded(LimitKind::InputBytes))?;
        if new_total > self.max_input_bytes {
            return Err(ParseError::LimitExceeded(LimitKind::InputBytes));
        }
        self.total_bytes_processed = new_total;

        let mut idx = 0;
        let chunk_len = chunk.len();

        // 1. If we have pending bytes from a previous chunk, try to complete them first
        if self.pending_len > 0 {
            while self.pending_len < 4 && idx < chunk_len {
                self.pending_buf[self.pending_len] = chunk[idx];
                self.pending_len += 1;
                idx += 1;

                match check_utf8_prefix(&self.pending_buf[..self.pending_len]) {
                    Utf8CheckResult::Complete(c, consumed) => {
                        budget.charge(1)?;
                        out_chars.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
                        out_chars.push(c);

                        // If consumed < pending_len, push leftovers back
                        if consumed < self.pending_len {
                            let excess = self.pending_len - consumed;
                            idx -= excess;
                        }
                        self.pending_len = 0;
                        break;
                    }
                    Utf8CheckResult::Incomplete => {
                        // Keep accumulating up to expected size
                        continue;
                    }
                    Utf8CheckResult::Invalid(consumed) => {
                        budget.charge(1)?;
                        out_chars.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
                        out_chars.push(REPLACEMENT_CHAR);

                        if consumed < self.pending_len {
                            let excess = self.pending_len - consumed;
                            idx -= excess;
                        }
                        self.pending_len = 0;
                        break;
                    }
                }
            }
        }

        // 2. Process main chunk body
        while idx < chunk_len {
            budget.charge(1)?;
            let slice = &chunk[idx..];
            match check_utf8_prefix(slice) {
                Utf8CheckResult::Complete(c, consumed) => {
                    out_chars.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
                    out_chars.push(c);
                    idx += consumed;
                }
                Utf8CheckResult::Incomplete => {
                    // Buffer remaining bytes for the next chunk
                    let remaining = chunk_len - idx;
                    self.pending_buf[..remaining].copy_from_slice(&chunk[idx..]);
                    self.pending_len = remaining;
                    break;
                }
                Utf8CheckResult::Invalid(consumed) => {
                    out_chars.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
                    out_chars.push(REPLACEMENT_CHAR);
                    idx += consumed;
                }
            }
        }

        Ok(())
    }

    /// Signals end-of-stream. Any dangling incomplete bytes are flushed as U+FFFD.
    pub fn finish(
        &mut self,
        out_chars: &mut Vec<char>,
        budget: &mut WorkBudgetTracker,
    ) -> Result<(), ParseError> {
        if self.pending_len > 0 {
            budget.charge(1)?;
            out_chars.try_reserve(1).map_err(|_| ParseError::OutOfMemory)?;
            out_chars.push(REPLACEMENT_CHAR);
            self.pending_len = 0;
        }
        Ok(())
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes_processed
    }
}

enum Utf8CheckResult {
    Complete(char, usize),
    Incomplete,
    Invalid(usize),
}

fn check_utf8_prefix(bytes: &[u8]) -> Utf8CheckResult {
    if bytes.is_empty() {
        return Utf8CheckResult::Incomplete;
    }

    let b0 = bytes[0];
    if b0 <= 0x7F {
        // 1-byte ASCII
        return Utf8CheckResult::Complete(b0 as char, 1);
    }

    let expected_len = if (b0 & 0xE0) == 0xC0 {
        if b0 < 0xC2 {
            // Overlong encoding (0xC0, 0xC1)
            return Utf8CheckResult::Invalid(1);
        }
        2
    } else if (b0 & 0xF0) == 0xE0 {
        3
    } else if (b0 & 0xF8) == 0xF0 {
        if b0 > 0xF4 {
            // Out of Unicode range
            return Utf8CheckResult::Invalid(1);
        }
        4
    } else {
        // Stray continuation byte or 0xF5..0xFF
        return Utf8CheckResult::Invalid(1);
    };

    if bytes.len() < expected_len {
        // Check if existing continuation bytes are valid so far
        for &cb in &bytes[1..] {
            if (cb & 0xC0) != 0x80 {
                return Utf8CheckResult::Invalid(1);
            }
        }
        return Utf8CheckResult::Incomplete;
    }

    // Full sequence is present, attempt decode
    match core::str::from_utf8(&bytes[..expected_len]) {
        Ok(s) => {
            let c = s.chars().next().unwrap_or(REPLACEMENT_CHAR);
            Utf8CheckResult::Complete(c, expected_len)
        }
        Err(_) => Utf8CheckResult::Invalid(1),
    }
}
