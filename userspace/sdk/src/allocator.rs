// userspace/sdk/src/allocator.rs - Bounded Heap Allocator for Ring 3 Applications
// Provides a thread-safe, preemption-safe bounded allocator implementing GlobalAlloc.

use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicUsize, Ordering};

#[repr(align(4096))]
pub struct BoundedHeapAllocator<const SIZE: usize> {
    arena: [u8; SIZE],
    offset: AtomicUsize,
    allocated_bytes: AtomicUsize,
    active_allocs: AtomicUsize,
}

unsafe impl<const SIZE: usize> Sync for BoundedHeapAllocator<SIZE> {}

impl<const SIZE: usize> BoundedHeapAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            arena: [0u8; SIZE],
            offset: AtomicUsize::new(0),
            allocated_bytes: AtomicUsize::new(0),
            active_allocs: AtomicUsize::new(0),
        }
    }

    /// Returns current allocated bytes in the heap
    pub fn used(&self) -> usize {
        self.allocated_bytes.load(Ordering::Relaxed)
    }

    /// Returns total heap capacity in bytes
    pub const fn capacity(&self) -> usize {
        SIZE
    }

    /// Resets the allocator offset
    pub unsafe fn reset(&self) {
        self.offset.store(0, Ordering::SeqCst);
        self.allocated_bytes.store(0, Ordering::SeqCst);
        self.active_allocs.store(0, Ordering::SeqCst);
    }
}

unsafe impl<const SIZE: usize> GlobalAlloc for BoundedHeapAllocator<SIZE> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();

        let base_addr = (&raw const self.arena) as *const u8 as usize;
        let mut cur = self.offset.load(Ordering::Relaxed);
        loop {
            let cur_addr = base_addr + cur;
            let aligned_addr = (cur_addr + align - 1) & !(align - 1);
            let next_offset = (aligned_addr - base_addr) + size;

            if next_offset > SIZE {
                // Out of memory: fail cleanly without crashing or corrupting memory
                return core::ptr::null_mut();
            }

            match self.offset.compare_exchange_weak(cur, next_offset, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => {
                    self.allocated_bytes.fetch_add(size, Ordering::Relaxed);
                    self.active_allocs.fetch_add(1, Ordering::SeqCst);
                    return aligned_addr as *mut u8;
                }
                Err(actual) => cur = actual,
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let size = layout.size();
        let base_addr = (&raw const self.arena) as *const u8 as usize;
        let ptr_addr = ptr as usize;

        let _ = self.allocated_bytes.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
            Some(cur.saturating_sub(size))
        });

        if ptr_addr >= base_addr && ptr_addr.saturating_add(size) <= base_addr + SIZE {
            let alloc_start = ptr_addr - base_addr;
            let alloc_end = alloc_start + size;
            // LIFO bump rollback if freeing the most recent allocation
            let _ = self.offset.compare_exchange(
                alloc_end,
                alloc_start,
                Ordering::SeqCst,
                Ordering::Relaxed,
            );
        }

        // Reset arena offset when active allocation count drops to 0
        let prev = self.active_allocs.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |cur| {
            Some(cur.saturating_sub(1))
        });
        if prev == Ok(1) {
            self.offset.store(0, Ordering::SeqCst);
            self.allocated_bytes.store(0, Ordering::SeqCst);
        }
    }
}
