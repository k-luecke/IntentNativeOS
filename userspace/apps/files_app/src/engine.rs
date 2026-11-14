// userspace/apps/files_app/src/engine.rs
// ============================================================================
// Native Ring 3 Files Application Engine — FileEntry & Workspace Stepping-Stone
// Snapshot Directory (`#![no_std]`)
// ============================================================================

#![allow(dead_code)]

#[derive(Clone, Copy)]
pub struct FileEntry {
    pub name: [u8; 40],
    pub name_len: usize,
    pub size: [u8; 16],
    pub size_len: usize,
}

impl FileEntry {
    pub const fn empty() -> Self {
        Self {
            name: [0u8; 40],
            name_len: 0,
            size: [0u8; 16],
            size_len: 0,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    pub fn size_str(&self) -> &str {
        core::str::from_utf8(&self.size[..self.size_len]).unwrap_or("0")
    }
}
