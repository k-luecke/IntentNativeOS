// kernel/src/memory/frame_allocator.rs - Physical Page Frame Allocator (K007)

use super::boot_info::{NormalizedMemoryMap, NormalizedRegionType, PAGE_SIZE};

pub const MAX_FRAMES: usize = 1_048_576; // 1,048,576 frames * 4 KiB = 4,096 MiB (4 GiB) managed physical RAM
const BITMAP_WORDS: usize = MAX_FRAMES / 64; // 16,384 words (128 KiB bitmap)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameAllocError {
    OutOfMemory,
    InvalidFrameAddress,
    FrameAlreadyFree,
    ReservedRegionViolation,
}

pub const MAX_SHARED_COW_ENTRIES: usize = 64;

pub struct PhysicalFrameAllocator {
    usable_bitmap: [u64; BITMAP_WORDS],
    allocated_bitmap: [u64; BITMAP_WORDS],
    managed_frames: usize,
    free_frames: usize,
    allocated_frames: usize,
    next_free_word_hint: usize,
    shared_cow_refs: [(u64, u16); MAX_SHARED_COW_ENTRIES],
}

impl PhysicalFrameAllocator {
    pub const fn empty() -> Self {
        Self {
            usable_bitmap: [0u64; BITMAP_WORDS],
            allocated_bitmap: [0u64; BITMAP_WORDS],
            managed_frames: MAX_FRAMES,
            free_frames: 0,
            allocated_frames: 0,
            next_free_word_hint: 0,
            shared_cow_refs: [(0u64, 0u16); MAX_SHARED_COW_ENTRIES],
        }
    }

    pub fn reset(&mut self) {
        for w in self.usable_bitmap.iter_mut() { *w = 0; }
        for w in self.allocated_bitmap.iter_mut() { *w = 0; }
        self.managed_frames = MAX_FRAMES;
        self.free_frames = 0;
        self.allocated_frames = 0;
        self.next_free_word_hint = 0;
        for entry in self.shared_cow_refs.iter_mut() { *entry = (0, 0); }
    }

    /// Initializes allocator from normalized memory map.
    /// Only `Usable` RAM regions are marked in usable_bitmap. Reserved, ACPI, and MMIO remain 0.
    pub fn init_from_memory_map(&mut self, mem_map: &NormalizedMemoryMap) {
        self.reset();

        for i in 0..mem_map.count {
            let reg = &mem_map.regions[i];
            if reg.region_type == NormalizedRegionType::Usable {
                let start_frame = (reg.start_addr / PAGE_SIZE) as usize;
                let frame_count = (reg.size_bytes / PAGE_SIZE) as usize;
                for f in start_frame..(start_frame + frame_count) {
                    if f < MAX_FRAMES {
                        let word = f / 64;
                        let bit = f % 64;
                        if (self.usable_bitmap[word] & (1u64 << bit)) == 0 {
                            self.usable_bitmap[word] |= 1u64 << bit;
                            self.free_frames += 1;
                        }
                    }
                }
            }
        }
    }

    pub fn is_usable(&self, frame_idx: usize) -> bool {
        if frame_idx >= self.managed_frames {
            return false;
        }
        let word = frame_idx / 64;
        let bit = frame_idx % 64;
        (self.usable_bitmap[word] & (1u64 << bit)) != 0
    }

    pub fn is_allocated(&self, frame_idx: usize) -> bool {
        if frame_idx >= self.managed_frames {
            return true;
        }
        let word = frame_idx / 64;
        let bit = frame_idx % 64;
        (self.allocated_bitmap[word] & (1u64 << bit)) != 0
    }

    /// O(1) amortized Next-Fit physical frame allocation using `next_free_word_hint`.
    pub fn alloc_frame(&mut self) -> Result<u64, FrameAllocError> {
        let start = self.next_free_word_hint % BITMAP_WORDS;
        for offset in 0..BITMAP_WORDS {
            let word_idx = (start + offset) % BITMAP_WORDS;
            let available_mask = self.usable_bitmap[word_idx] & !self.allocated_bitmap[word_idx];
            if available_mask != 0 {
                let bit_idx = available_mask.trailing_zeros() as usize;
                let frame_idx = word_idx * 64 + bit_idx;
                if frame_idx >= self.managed_frames {
                    return Err(FrameAllocError::OutOfMemory);
                }
                self.allocated_bitmap[word_idx] |= 1u64 << bit_idx;
                self.allocated_frames += 1;
                self.free_frames -= 1;
                self.next_free_word_hint = word_idx;
                return Ok((frame_idx as u64) * PAGE_SIZE);
            }
        }
        Err(FrameAllocError::OutOfMemory)
    }

    /// Increment Copy-on-Write (CoW) reference count for an allocated physical frame.
    pub fn share_frame(&mut self, phys_addr: u64) -> Result<u16, FrameAllocError> {
        if phys_addr % PAGE_SIZE != 0 {
            return Err(FrameAllocError::InvalidFrameAddress);
        }
        let frame_idx = (phys_addr / PAGE_SIZE) as usize;
        if frame_idx >= self.managed_frames || !self.is_allocated(frame_idx) {
            return Err(FrameAllocError::FrameAlreadyFree);
        }
        for entry in self.shared_cow_refs.iter_mut() {
            if entry.1 > 0 && entry.0 == phys_addr {
                entry.1 = entry.1.saturating_add(1);
                return Ok(entry.1);
            }
        }
        for entry in self.shared_cow_refs.iter_mut() {
            if entry.1 == 0 {
                *entry = (phys_addr, 2); // Initial owner + new CoW sharer = 2 refs
                return Ok(2);
            }
        }
        Err(FrameAllocError::OutOfMemory)
    }

    /// Query current reference count of an allocated frame (1 if exclusively owned, >=2 if CoW shared).
    pub fn frame_ref_count(&self, phys_addr: u64) -> u16 {
        let frame_idx = (phys_addr / PAGE_SIZE) as usize;
        if frame_idx >= self.managed_frames || !self.is_allocated(frame_idx) {
            return 0;
        }
        for entry in self.shared_cow_refs.iter() {
            if entry.1 > 0 && entry.0 == phys_addr {
                return entry.1;
            }
        }
        1
    }

    pub fn free_frame(&mut self, phys_addr: u64) -> Result<(), FrameAllocError> {
        if phys_addr % PAGE_SIZE != 0 {
            return Err(FrameAllocError::InvalidFrameAddress);
        }
        let frame_idx = (phys_addr / PAGE_SIZE) as usize;
        if frame_idx >= self.managed_frames || !self.is_usable(frame_idx) {
            return Err(FrameAllocError::ReservedRegionViolation);
        }
        let word = frame_idx / 64;
        let bit = frame_idx % 64;
        if (self.allocated_bitmap[word] & (1u64 << bit)) == 0 {
            return Err(FrameAllocError::FrameAlreadyFree);
        }

        // If frame is Copy-on-Write (CoW) shared across processes, decrement refcount first
        for entry in self.shared_cow_refs.iter_mut() {
            if entry.1 > 0 && entry.0 == phys_addr {
                entry.1 -= 1;
                if entry.1 >= 2 {
                    return Ok(());
                } else {
                    // Dropped to 1 remaining owner; clear shared table slot and keep frame allocated for sole owner
                    *entry = (0, 0);
                    return Ok(());
                }
            }
        }

        self.allocated_bitmap[word] &= !(1u64 << bit);
        self.allocated_frames -= 1;
        self.free_frames += 1;
        if word < self.next_free_word_hint {
            self.next_free_word_hint = word;
        }
        Ok(())
    }

    pub fn alloc_high_frame(&mut self, min_phys_addr: u64) -> Result<u64, FrameAllocError> {
        let min_frame = (min_phys_addr / PAGE_SIZE) as usize;
        let start_word = min_frame / 64;
        for word_idx in start_word..BITMAP_WORDS {
            let available_mask = self.usable_bitmap[word_idx] & !self.allocated_bitmap[word_idx];
            if available_mask != 0 {
                let bit_idx = available_mask.trailing_zeros() as usize;
                let frame_idx = word_idx * 64 + bit_idx;
                if frame_idx >= min_frame && frame_idx < self.managed_frames {
                    self.allocated_bitmap[word_idx] |= 1u64 << bit_idx;
                    self.allocated_frames += 1;
                    self.free_frames -= 1;
                    return Ok((frame_idx as u64) * PAGE_SIZE);
                }
            }
        }
        Err(FrameAllocError::OutOfMemory)
    }

    pub fn alloc_contiguous_frames(&mut self, count: usize) -> Result<u64, FrameAllocError> {
        if count == 0 {
            return Err(FrameAllocError::OutOfMemory);
        }
        if count == 1 {
            return self.alloc_frame();
        }

        let mut current_start = 0;
        let mut consecutive = 0;

        for frame_idx in 0..self.managed_frames {
            let word = frame_idx / 64;
            let bit = frame_idx % 64;
            let is_free = (self.usable_bitmap[word] & (1u64 << bit) != 0) &&
                          (self.allocated_bitmap[word] & (1u64 << bit) == 0);
            if is_free {
                if consecutive == 0 {
                    current_start = frame_idx;
                }
                consecutive += 1;
                if consecutive == count {
                    // Mark all contiguous frames as allocated
                    for f in current_start..(current_start + count) {
                        let w = f / 64;
                        let b = f % 64;
                        self.allocated_bitmap[w] |= 1u64 << b;
                    }
                    self.allocated_frames += count;
                    self.free_frames -= count;
                    return Ok((current_start as u64) * PAGE_SIZE);
                }
            } else {
                consecutive = 0;
            }
        }
        Err(FrameAllocError::OutOfMemory)
    }

    pub fn alloc_contiguous_high_frames(&mut self, count: usize, min_phys_addr: u64) -> Result<u64, FrameAllocError> {
        if count == 0 {
            return Err(FrameAllocError::OutOfMemory);
        }
        let min_frame = (min_phys_addr / PAGE_SIZE) as usize;
        let mut current_start = min_frame;
        let mut consecutive = 0;

        for frame_idx in min_frame..self.managed_frames {
            let word = frame_idx / 64;
            let bit = frame_idx % 64;
            let is_free = (self.usable_bitmap[word] & (1u64 << bit) != 0) &&
                          (self.allocated_bitmap[word] & (1u64 << bit) == 0);
            if is_free {
                if consecutive == 0 {
                    current_start = frame_idx;
                }
                consecutive += 1;
                if consecutive == count {
                    for f in current_start..(current_start + count) {
                        let w = f / 64;
                        let b = f % 64;
                        self.allocated_bitmap[w] |= 1u64 << b;
                    }
                    self.allocated_frames += count;
                    self.free_frames -= count;
                    return Ok((current_start as u64) * PAGE_SIZE);
                }
            } else {
                consecutive = 0;
            }
        }
        Err(FrameAllocError::OutOfMemory)
    }

    pub fn free_contiguous_frames(&mut self, start_phys: u64, count: usize) -> Result<(), FrameAllocError> {
        for i in 0..count {
            self.free_frame(start_phys + (i as u64) * PAGE_SIZE)?;
        }
        Ok(())
    }

    /// Allocates a 2 MiB-aligned (512 contiguous 4 KiB frames, 8 bitmap words) HugePage frame
    /// for low-TLB-pressure tensor/heap buffers.
    pub fn alloc_huge_frame_2mb(&mut self) -> Result<u64, FrameAllocError> {
        let mut word_base = 0;
        while word_base + 8 <= BITMAP_WORDS {
            let mut all_free = true;
            for w in word_base..(word_base + 8) {
                let avail = self.usable_bitmap[w] & !self.allocated_bitmap[w];
                if avail != u64::MAX {
                    all_free = false;
                    break;
                }
            }
            if all_free {
                for w in word_base..(word_base + 8) {
                    self.allocated_bitmap[w] = u64::MAX;
                }
                self.allocated_frames += 512;
                self.free_frames -= 512;
                let frame_idx = word_base * 64;
                return Ok((frame_idx as u64) * PAGE_SIZE);
            }
            word_base += 8;
        }
        Err(FrameAllocError::OutOfMemory)
    }

    pub fn free_frames_count(&self) -> usize {
        self.free_frames
    }

    pub fn allocated_frames_count(&self) -> usize {
        self.allocated_frames
    }
}
