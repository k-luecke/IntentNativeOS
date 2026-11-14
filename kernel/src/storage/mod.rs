// kernel/src/storage/mod.rs - Block Device Interface, Inode Filesystem, and Atomic WAL (S001, S002, S003)

pub const BLOCK_SIZE: usize = 512;
pub const MAX_BLOCKS: usize = 256; // 128 KB storage volume
pub const MAX_INODES: usize = 64;
pub const MAX_NAME_LEN: usize = 28;
pub const DIRECT_BLOCKS_PER_INODE: usize = 8;
pub const FS_MAGIC: u32 = 0x494E4F53; // "INOS"

pub mod transport;
pub mod workspace;
pub mod transaction;

// --- S001: Block Device Abstraction ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    InvalidLba,
    IoError,
    DeviceBusy,
}

pub trait BlockDevice {
    fn read_block(&self, lba: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError>;
    fn write_block(&mut self, lba: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), BlockError>;
    fn block_count(&self) -> u64;
}

pub struct MemoryBlockDevice {
    data: [[u8; BLOCK_SIZE]; MAX_BLOCKS],
    blocks: u64,
}

impl MemoryBlockDevice {
    pub const fn new() -> Self {
        Self {
            data: [[0u8; BLOCK_SIZE]; MAX_BLOCKS],
            blocks: MAX_BLOCKS as u64,
        }
    }
}

impl BlockDevice for MemoryBlockDevice {
    fn read_block(&self, lba: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        if lba >= self.blocks {
            return Err(BlockError::InvalidLba);
        }
        buf.copy_from_slice(&self.data[lba as usize]);
        Ok(())
    }

    fn write_block(&mut self, lba: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        if lba >= self.blocks {
            return Err(BlockError::InvalidLba);
        }
        self.data[lba as usize].copy_from_slice(buf);
        Ok(())
    }

    fn block_count(&self) -> u64 {
        self.blocks
    }
}

impl<'a, B: BlockDevice> BlockDevice for &'a mut B {
    fn read_block(&self, lba: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        (**self).read_block(lba, buf)
    }

    fn write_block(&mut self, lba: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        (**self).write_block(lba, buf)
    }

    fn block_count(&self) -> u64 {
        (**self).block_count()
    }
}

// --- G8: Hardware ATA PIO Block Device Driver ---

pub struct AtaPioBlockDevice {
    pub base_port: u16,
    pub drive_idx: u8,
    pub blocks: u64,
}

impl AtaPioBlockDevice {
    pub const fn new(base_port: u16, drive_idx: u8, blocks: u64) -> Self {
        Self {
            base_port,
            drive_idx,
            blocks,
        }
    }

    /// Probes if an ATA device is present at base_port and drive_idx
    pub fn probe(base_port: u16, drive_idx: u8) -> Option<Self> {
        unsafe {
            let drive_sel = 0xA0 | ((drive_idx & 1) << 4);
            crate::arch::x86_64::cpu::outb(base_port + 6, drive_sel);
            for _ in 0..100 {
                let _ = crate::arch::x86_64::cpu::inb(base_port + 7);
            }

            let status = crate::arch::x86_64::cpu::inb(base_port + 7);
            // 0xFF means floating bus / no drive; 0x00 means drive not present
            if status == 0xFF || status == 0x00 {
                return None;
            }

            Some(Self {
                base_port,
                drive_idx,
                blocks: 32768, // 16 MB default
            })
        }
    }

    fn wait_not_busy(&self) -> Result<u8, BlockError> {
        for _ in 0..2_000_000 {
            let status = unsafe { crate::arch::x86_64::cpu::inb(self.base_port + 7) };
            if (status & 0x80) == 0 {
                return Ok(status);
            }
        }
        Err(BlockError::DeviceBusy)
    }

    fn wait_ready(&self) -> Result<(), BlockError> {
        for _ in 0..2_000_000 {
            let status = unsafe { crate::arch::x86_64::cpu::inb(self.base_port + 7) };
            if (status & 0x80) == 0 && (status & 0x08) != 0 {
                return Ok(());
            }
            if (status & 0x01) != 0 {
                return Err(BlockError::IoError);
            }
        }
        Err(BlockError::DeviceBusy)
    }
}

impl BlockDevice for AtaPioBlockDevice {
    fn read_block(&self, lba: u64, buf: &mut [u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        unsafe {
            self.wait_not_busy()?;
            let drive_sel = 0xE0 | ((self.drive_idx & 1) << 4) | (((lba >> 24) & 0x0F) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 6, drive_sel);
            crate::arch::x86_64::cpu::outb(self.base_port + 2, 1); // 1 sector
            crate::arch::x86_64::cpu::outb(self.base_port + 3, (lba & 0xFF) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 4, ((lba >> 8) & 0xFF) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 5, ((lba >> 16) & 0xFF) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 7, 0x20); // READ SECTORS

            self.wait_ready()?;

            let ptr = buf.as_mut_ptr() as *mut u16;
            core::arch::asm!(
                "cld",
                "rep insw",
                in("dx") self.base_port,
                inout("rdi") ptr => _,
                inout("rcx") 256usize => _,
                options(nostack, preserves_flags)
            );
            Ok(())
        }
    }

    fn write_block(&mut self, lba: u64, buf: &[u8; BLOCK_SIZE]) -> Result<(), BlockError> {
        unsafe {
            self.wait_not_busy()?;
            let drive_sel = 0xE0 | ((self.drive_idx & 1) << 4) | (((lba >> 24) & 0x0F) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 6, drive_sel);
            crate::arch::x86_64::cpu::outb(self.base_port + 2, 1); // 1 sector
            crate::arch::x86_64::cpu::outb(self.base_port + 3, (lba & 0xFF) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 4, ((lba >> 8) & 0xFF) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 5, ((lba >> 16) & 0xFF) as u8);
            crate::arch::x86_64::cpu::outb(self.base_port + 7, 0x30); // WRITE SECTORS

            self.wait_ready()?;

            let ptr = buf.as_ptr() as *const u16;
            core::arch::asm!(
                "cld",
                "rep outsw",
                in("dx") self.base_port,
                inout("rsi") ptr => _,
                inout("rcx") 256usize => _,
                options(nostack, preserves_flags)
            );

            // Flush cache command (0xE7: Flush Cache)
            crate::arch::x86_64::cpu::outb(self.base_port + 7, 0xE7);
            self.wait_not_busy()?;
            Ok(())
        }
    }

    fn block_count(&self) -> u64 {
        self.blocks
    }
}

impl AtaPioBlockDevice {
    /// Batched multi-sector ATA PIO read: issues command 0x20 once for up to `count` sectors,
    /// servicing DRQ/BSY sector-by-sector to minimize ATA register write exits.
    pub fn read_multi_blocks_batched(&self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), BlockError> {
        if count == 0 {
            return Ok(());
        }
        unsafe {
            let mut retries = 3;
            while retries > 0 {
                retries -= 1;
                let res: Result<(), BlockError> = (|| {
                    self.wait_not_busy()?;
                    let drive_sel = 0xE0 | ((self.drive_idx & 1) << 4) | (((start_lba >> 24) & 0x0F) as u8);
                    crate::arch::x86_64::cpu::outb(self.base_port + 6, drive_sel);
                    crate::arch::x86_64::cpu::outb(self.base_port + 2, count);
                    crate::arch::x86_64::cpu::outb(self.base_port + 3, (start_lba & 0xFF) as u8);
                    crate::arch::x86_64::cpu::outb(self.base_port + 4, ((start_lba >> 8) & 0xFF) as u8);
                    crate::arch::x86_64::cpu::outb(self.base_port + 5, ((start_lba >> 16) & 0xFF) as u8);
                    crate::arch::x86_64::cpu::outb(self.base_port + 7, 0x20); // READ SECTORS (multi-sector)

                    for s in 0..(count as usize) {
                        self.wait_ready()?;
                        let ptr = dest.add(s * 512) as *mut u16;
                        core::arch::asm!(
                            "cld",
                            "rep insw",
                            in("dx") self.base_port,
                            inout("rdi") ptr => _,
                            inout("rcx") 256usize => _,
                            options(nostack, preserves_flags)
                        );
                    }
                    Ok(())
                })();

                if res.is_ok() {
                    return Ok(());
                } else if retries == 0 {
                    return res;
                }
            }
        }
        Err(BlockError::DeviceBusy)
    }

    /// Single-sector reference fallback implementation
    pub fn read_multi_blocks_legacy(&self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), BlockError> {
        if count == 0 {
            return Ok(());
        }
        for i in 0..count {
            unsafe {
                let mut retries = 3;
                while retries > 0 {
                    retries -= 1;
                    if let Ok(()) = (|| -> Result<(), BlockError> {
                        self.wait_not_busy()?;
                        let lba = start_lba + i as u64;
                        let drive_sel = 0xE0 | ((self.drive_idx & 1) << 4) | (((lba >> 24) & 0x0F) as u8);
                        crate::arch::x86_64::cpu::outb(self.base_port + 6, drive_sel);
                        crate::arch::x86_64::cpu::outb(self.base_port + 2, 1); // 1 sector
                        crate::arch::x86_64::cpu::outb(self.base_port + 3, (lba & 0xFF) as u8);
                        crate::arch::x86_64::cpu::outb(self.base_port + 4, ((lba >> 8) & 0xFF) as u8);
                        crate::arch::x86_64::cpu::outb(self.base_port + 5, ((lba >> 16) & 0xFF) as u8);
                        crate::arch::x86_64::cpu::outb(self.base_port + 7, 0x20); // READ SECTORS

                        self.wait_ready()?;

                        let ptr = dest.add((i as usize) * 512) as *mut u16;
                        core::arch::asm!(
                            "cld",
                            "rep insw",
                            in("dx") self.base_port,
                            inout("rdi") ptr => _,
                            inout("rcx") 256usize => _,
                            options(nostack, preserves_flags)
                        );
                        Ok(())
                    })() {
                        break;
                    } else if retries == 0 {
                        return Err(BlockError::DeviceBusy);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn read_multi_blocks(&self, start_lba: u64, count: u8, dest: *mut u8) -> Result<(), BlockError> {
        // Attempt batched multi-sector read first; fall back to legacy single-sector on failure
        match self.read_multi_blocks_batched(start_lba, count, dest) {
            Ok(()) => Ok(()),
            Err(_) => self.read_multi_blocks_legacy(start_lba, count, dest),
        }
    }
}

// --- S002: Inode Filesystem (INOS-FS) ---

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    Unused = 0,
    RegularFile = 1,
    Directory = 2,
    ExtendedFile = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Inode {
    pub inode_type: InodeType,
    pub size: u32,
    pub direct_blocks: [u32; DIRECT_BLOCKS_PER_INODE],
    pub direct_count: u32,
}

impl Inode {
    pub const fn empty() -> Self {
        Self {
            inode_type: InodeType::Unused,
            size: 0,
            direct_blocks: [0; DIRECT_BLOCKS_PER_INODE],
            direct_count: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct DirEntry {
    pub inode_id: u32,
    pub name_len: u8,
    pub name: [u8; MAX_NAME_LEN],
}

impl DirEntry {
    pub const fn empty() -> Self {
        Self {
            inode_id: 0,
            name_len: 0,
            name: [0u8; MAX_NAME_LEN],
        }
    }
}

pub const INODE_START_LBA: u64 = 1;
pub const INODE_BLOCKS: u64 = 6;
pub const DIR_START_LBA: u64 = 7;
pub const DIR_BLOCKS: u64 = 5;
pub const SNAPSHOT_START_LBA: u64 = 12;
pub const SNAPSHOT_BLOCKS: u64 = 4;
pub const WAL_START_LBA: u64 = 16;
pub const WAL_BLOCKS: u64 = 4;
pub const DATA_START_LBA: u64 = 20;
pub const FS_DATA_END_LBA: u64 = 24576; // 12 MB filesystem data ceiling (reserves top 4 MB)
pub const INTENT_SPOOL_START_LBA: u64 = 24576; // Dedicated model transport region
pub const INTENT_SPOOL_MAX_SECTORS: u16 = 8; // Bounded variable length up to 4,096 bytes
pub const INTENT_SPOOL_MAGIC: u32 = 0x494E545F; // "INT_"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    DiskFull,
    InodeTableFull,
    FileNotFound,
    FileAlreadyExists,
    InvalidPath,
    IoFailure,
    BufferTooSmall,
    CorruptData,
}

pub struct InosFileSystem<D: BlockDevice> {
    pub device: D,
    pub inodes: [Inode; MAX_INODES],
    pub dir_entries: [DirEntry; MAX_INODES],
    pub free_block_cursor: u64,
}

impl<D: BlockDevice> InosFileSystem<D> {
    pub fn new(device: D) -> Self {
        Self {
            device,
            inodes: [Inode::empty(); MAX_INODES],
            dir_entries: [DirEntry::empty(); MAX_INODES],
            free_block_cursor: DATA_START_LBA,
        }
    }

    pub fn format(&mut self) -> Result<(), FsError> {
        let mut sb_buf = [0u8; BLOCK_SIZE];
        sb_buf[0..4].copy_from_slice(&FS_MAGIC.to_ne_bytes());
        sb_buf[4..8].copy_from_slice(&(BLOCK_SIZE as u32).to_ne_bytes());
        sb_buf[8..12].copy_from_slice(&(MAX_BLOCKS as u32).to_ne_bytes());
        let stage: u32 = 0;
        sb_buf[12..16].copy_from_slice(&stage.to_ne_bytes());
        sb_buf[16..24].copy_from_slice(&DATA_START_LBA.to_ne_bytes());

        self.device.write_block(0, &sb_buf).map_err(|_| FsError::IoFailure)?;

        for i in 0..MAX_INODES {
            self.inodes[i] = Inode::empty();
            self.dir_entries[i] = DirEntry::empty();
        }
        self.free_block_cursor = DATA_START_LBA;
        self.sync_metadata(0)?;
        Ok(())
    }

    /// Flushes superblock, inode table, and directory table blocks to disk
    pub fn sync_metadata(&mut self, stage: u32) -> Result<(), FsError> {
        // 1. Superblock (LBA 0)
        let mut sb_buf = [0u8; BLOCK_SIZE];
        sb_buf[0..4].copy_from_slice(&FS_MAGIC.to_ne_bytes());
        sb_buf[4..8].copy_from_slice(&(BLOCK_SIZE as u32).to_ne_bytes());
        sb_buf[8..12].copy_from_slice(&(MAX_BLOCKS as u32).to_ne_bytes());
        sb_buf[12..16].copy_from_slice(&stage.to_ne_bytes());
        sb_buf[16..24].copy_from_slice(&self.free_block_cursor.to_ne_bytes());
        self.device.write_block(0, &sb_buf).map_err(|_| FsError::IoFailure)?;

        // 2. Inodes (LBA 1..6)
        let inode_bytes = unsafe {
            core::slice::from_raw_parts(
                self.inodes.as_ptr() as *const u8,
                core::mem::size_of_val(&self.inodes),
            )
        };
        for (i, chunk) in inode_bytes.chunks(BLOCK_SIZE).enumerate() {
            let mut block = [0u8; BLOCK_SIZE];
            block[..chunk.len()].copy_from_slice(chunk);
            self.device.write_block(INODE_START_LBA + i as u64, &block).map_err(|_| FsError::IoFailure)?;
        }

        // 3. Dir Entries (LBA 7..11)
        let dir_bytes = unsafe {
            core::slice::from_raw_parts(
                self.dir_entries.as_ptr() as *const u8,
                core::mem::size_of_val(&self.dir_entries),
            )
        };
        for (i, chunk) in dir_bytes.chunks(BLOCK_SIZE).enumerate() {
            let mut block = [0u8; BLOCK_SIZE];
            block[..chunk.len()].copy_from_slice(chunk);
            self.device.write_block(DIR_START_LBA + i as u64, &block).map_err(|_| FsError::IoFailure)?;
        }

        Ok(())
    }

    /// Mounts existing filesystem from disk, returning (is_formatted, stage)
    pub fn mount(&mut self) -> Result<(bool, u32), FsError> {
        let mut sb_buf = [0u8; BLOCK_SIZE];
        self.device.read_block(0, &mut sb_buf).map_err(|_| FsError::IoFailure)?;

        let mut magic_bytes = [0u8; 4];
        magic_bytes.copy_from_slice(&sb_buf[0..4]);
        if u32::from_ne_bytes(magic_bytes) != FS_MAGIC {
            return Ok((false, 0));
        }

        let mut stage_bytes = [0u8; 4];
        stage_bytes.copy_from_slice(&sb_buf[12..16]);
        let stage = u32::from_ne_bytes(stage_bytes);

        let mut cursor_bytes = [0u8; 8];
        cursor_bytes.copy_from_slice(&sb_buf[16..24]);
        let cursor = u64::from_ne_bytes(cursor_bytes);
        self.free_block_cursor = if cursor >= DATA_START_LBA { cursor } else { DATA_START_LBA };

        // Recover or discard pending WAL transactions before reading metadata tables
        let wal = AtomicJournal::new(WAL_START_LBA);
        let _ = wal.recover_on_boot(&mut self.device);

        // Read Inodes (LBA 1..6)
        let inode_slice = unsafe {
            core::slice::from_raw_parts_mut(
                self.inodes.as_mut_ptr() as *mut u8,
                core::mem::size_of_val(&self.inodes),
            )
        };
        for (i, chunk) in inode_slice.chunks_mut(BLOCK_SIZE).enumerate() {
            let mut block = [0u8; BLOCK_SIZE];
            self.device.read_block(INODE_START_LBA + i as u64, &mut block).map_err(|_| FsError::IoFailure)?;
            chunk.copy_from_slice(&block[..chunk.len()]);
        }

        // Read Dir Entries (LBA 7..11)
        let dir_slice = unsafe {
            core::slice::from_raw_parts_mut(
                self.dir_entries.as_mut_ptr() as *mut u8,
                core::mem::size_of_val(&self.dir_entries),
            )
        };
        for (i, chunk) in dir_slice.chunks_mut(BLOCK_SIZE).enumerate() {
            let mut block = [0u8; BLOCK_SIZE];
            self.device.read_block(DIR_START_LBA + i as u64, &mut block).map_err(|_| FsError::IoFailure)?;
            chunk.copy_from_slice(&block[..chunk.len()]);
        }
 
        // Recover any pending transaction from disk journal now that filesystem is mounted
        unsafe {
            let _ = transaction::GLOBAL_TX_MANAGER.recover_on_boot(&mut self.device);
        }

        Ok((true, stage))
    }

    /// Preserves a snapshot of a file's metadata and data blocks to dedicated snapshot disk blocks
    pub fn snapshot_file(&mut self, name: &str) -> Result<(), FsError> {
        let inode_id = self.find_inode(name)?;
        let inode = self.inodes[(inode_id - 1) as usize];
        if (inode.direct_count as usize) > (SNAPSHOT_BLOCKS as usize - 1) {
            return Err(FsError::BufferTooSmall);
        }

        // LBA 12: Snapshot header
        let mut snap_meta = [0u8; BLOCK_SIZE];
        snap_meta[0..4].copy_from_slice(&inode_id.to_ne_bytes());
        snap_meta[4..8].copy_from_slice(&inode.size.to_ne_bytes());
        snap_meta[8..12].copy_from_slice(&inode.direct_count.to_ne_bytes());
        snap_meta[12] = name.len() as u8;
        snap_meta[13..13 + name.len()].copy_from_slice(name.as_bytes());
        self.device.write_block(SNAPSHOT_START_LBA, &snap_meta).map_err(|_| FsError::IoFailure)?;

        // LBA 13..15: Snapshot data blocks
        for b in 0..(inode.direct_count as usize) {
            let lba = inode.direct_blocks[b] as u64;
            let mut data_block = [0u8; BLOCK_SIZE];
            self.device.read_block(lba, &mut data_block).map_err(|_| FsError::IoFailure)?;
            self.device.write_block(SNAPSHOT_START_LBA + 1 + b as u64, &data_block).map_err(|_| FsError::IoFailure)?;
        }

        Ok(())
    }

    /// Restores a file from its on-disk snapshot and flushes restored blocks to disk
    pub fn undo_file(&mut self, name: &str, current_stage: u32) -> Result<(), FsError> {
        let inode_id = self.find_inode(name)?;

        // Read snapshot header from LBA 12
        let mut snap_meta = [0u8; BLOCK_SIZE];
        self.device.read_block(SNAPSHOT_START_LBA, &mut snap_meta).map_err(|_| FsError::IoFailure)?;

        let snap_name_len = snap_meta[12] as usize;
        if snap_name_len == 0
            || snap_name_len > MAX_NAME_LEN
            || &snap_meta[13..13 + snap_name_len] != name.as_bytes()
        {
            return Err(FsError::FileNotFound);
        }

        let mut size_bytes = [0u8; 4];
        size_bytes.copy_from_slice(&snap_meta[4..8]);
        let orig_size = u32::from_ne_bytes(size_bytes);

        let mut count_bytes = [0u8; 4];
        count_bytes.copy_from_slice(&snap_meta[8..12]);
        let orig_count = u32::from_ne_bytes(count_bytes);
        if (orig_count as usize) > (SNAPSHOT_BLOCKS as usize - 1) {
            return Err(FsError::BufferTooSmall);
        }

        // Restore snapshot data blocks
        let inode = &mut self.inodes[(inode_id - 1) as usize];
        for b in 0..(orig_count as usize) {
            let target_lba = inode.direct_blocks[b] as u64;
            let mut data_block = [0u8; BLOCK_SIZE];
            self.device.read_block(SNAPSHOT_START_LBA + 1 + b as u64, &mut data_block).map_err(|_| FsError::IoFailure)?;
            self.device.write_block(target_lba, &data_block).map_err(|_| FsError::IoFailure)?;
        }

        inode.size = orig_size;
        inode.direct_count = orig_count;

        // Flush restored metadata to disk
        self.sync_metadata(current_stage)?;
        Ok(())
    }

    pub fn create_file(&mut self, name: &str) -> Result<u32, FsError> {
        if name.is_empty() || name.len() > MAX_NAME_LEN {
            return Err(FsError::InvalidPath);
        }

        // Check if exists
        for entry in self.dir_entries.iter() {
            if entry.name_len as usize == name.len() && &entry.name[..name.len()] == name.as_bytes() {
                return Err(FsError::FileAlreadyExists);
            }
        }

        // Allocate inode
        let inode_idx = self.inodes.iter().position(|i| i.inode_type == InodeType::Unused)
            .ok_or(FsError::InodeTableFull)?;

        // Allocate dir entry
        let dir_idx = self.dir_entries.iter().position(|d| d.inode_id == 0)
            .ok_or(FsError::InodeTableFull)?;

        self.inodes[inode_idx] = Inode {
            inode_type: InodeType::RegularFile,
            size: 0,
            direct_blocks: [0; DIRECT_BLOCKS_PER_INODE],
            direct_count: 0,
        };

        let mut name_buf = [0u8; MAX_NAME_LEN];
        name_buf[..name.len()].copy_from_slice(name.as_bytes());

        self.dir_entries[dir_idx] = DirEntry {
            inode_id: (inode_idx + 1) as u32,
            name_len: name.len() as u8,
            name: name_buf,
        };

        Ok((inode_idx + 1) as u32)
    }

    pub fn write_file(&mut self, name: &str, data: &[u8]) -> Result<usize, FsError> {
        let inode_id = self.find_inode(name)?;
        let blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;

        if blocks_needed <= DIRECT_BLOCKS_PER_INODE {
            let inode = &mut self.inodes[(inode_id - 1) as usize];
            let mut bytes_written = 0;
            for b in 0..blocks_needed {
                let block_lba = self.free_block_cursor;
                if block_lba >= FS_DATA_END_LBA || block_lba >= self.device.block_count() {
                    return Err(FsError::DiskFull);
                }
                self.free_block_cursor += 1;

                let mut block_data = [0u8; BLOCK_SIZE];
                let start = b * BLOCK_SIZE;
                let end = core::cmp::min(start + BLOCK_SIZE, data.len());
                let chunk_len = end - start;
                block_data[..chunk_len].copy_from_slice(&data[start..end]);

                self.device.write_block(block_lba, &block_data).map_err(|_| FsError::IoFailure)?;

                inode.direct_blocks[b] = block_lba as u32;
                bytes_written += chunk_len;
            }

            inode.inode_type = InodeType::RegularFile;
            inode.direct_count = blocks_needed as u32;
            inode.size = data.len() as u32;
            Ok(bytes_written)
        } else if blocks_needed <= 128 {
            // Extended file path with indirect block (FS-CAPACITY-01)
            let indirect_lba = self.free_block_cursor;
            if indirect_lba + (blocks_needed as u64) + 1 >= FS_DATA_END_LBA
                || indirect_lba + (blocks_needed as u64) + 1 >= self.device.block_count()
            {
                return Err(FsError::DiskFull);
            }
            self.free_block_cursor += 1;

            let mut ind_table = [0u8; BLOCK_SIZE];
            let mut bytes_written = 0;

            for b in 0..blocks_needed {
                let data_lba = self.free_block_cursor;
                self.free_block_cursor += 1;

                let off = b * 4;
                ind_table[off..off + 4].copy_from_slice(&(data_lba as u32).to_le_bytes());

                let mut block_data = [0u8; BLOCK_SIZE];
                let start = b * BLOCK_SIZE;
                let end = core::cmp::min(start + BLOCK_SIZE, data.len());
                let chunk_len = end - start;
                block_data[..chunk_len].copy_from_slice(&data[start..end]);

                self.device.write_block(data_lba, &block_data).map_err(|_| FsError::IoFailure)?;
                bytes_written += chunk_len;
            }

            self.device.write_block(indirect_lba, &ind_table).map_err(|_| FsError::IoFailure)?;

            let inode = &mut self.inodes[(inode_id - 1) as usize];
            inode.inode_type = InodeType::ExtendedFile;
            inode.direct_blocks[0] = indirect_lba as u32;
            inode.direct_count = blocks_needed as u32;
            inode.size = data.len() as u32;
            Ok(bytes_written)
        } else {
            Err(FsError::DiskFull)
        }
    }

    /// Writes file using atomic WAL transactions to guarantee crash consistency across power loss
    pub fn write_file_atomic(&mut self, name: &str, data: &[u8]) -> Result<usize, FsError> {
        let inode_id = self.find_inode(name)?;
        let blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;

        let wal = AtomicJournal::new(WAL_START_LBA);

        if blocks_needed <= DIRECT_BLOCKS_PER_INODE {
            // Pre-allocate blocks
            let mut target_lbas = [0u64; DIRECT_BLOCKS_PER_INODE];
            for b in 0..blocks_needed {
                let block_lba = self.free_block_cursor;
                if block_lba >= FS_DATA_END_LBA || block_lba >= self.device.block_count() {
                    return Err(FsError::DiskFull);
                }
                self.free_block_cursor += 1;
                target_lbas[b] = block_lba;
            }

            let mut bytes_written = 0;
            for b in 0..blocks_needed {
                let mut block_data = [0u8; BLOCK_SIZE];
                let start = b * BLOCK_SIZE;
                let end = core::cmp::min(start + BLOCK_SIZE, data.len());
                let chunk_len = end - start;
                block_data[..chunk_len].copy_from_slice(&data[start..end]);

                // Atomic WAL write: log -> commit -> write block
                wal.atomic_write(&mut self.device, target_lbas[b], &block_data, false)
                    .map_err(|_| FsError::IoFailure)?;

                self.inodes[(inode_id - 1) as usize].direct_blocks[b] = target_lbas[b] as u32;
                bytes_written += chunk_len;
            }

            self.inodes[(inode_id - 1) as usize].inode_type = InodeType::RegularFile;
            self.inodes[(inode_id - 1) as usize].direct_count = blocks_needed as u32;
            self.inodes[(inode_id - 1) as usize].size = data.len() as u32;

            self.sync_metadata(1)?;
            Ok(bytes_written)
        } else if blocks_needed <= 128 {
            let indirect_lba = self.free_block_cursor;
            if indirect_lba + (blocks_needed as u64) + 1 >= FS_DATA_END_LBA
                || indirect_lba + (blocks_needed as u64) + 1 >= self.device.block_count()
            {
                return Err(FsError::DiskFull);
            }
            self.free_block_cursor += 1;

            let mut ind_table = [0u8; BLOCK_SIZE];
            let mut bytes_written = 0;

            for b in 0..blocks_needed {
                let data_lba = self.free_block_cursor;
                self.free_block_cursor += 1;

                let off = b * 4;
                ind_table[off..off + 4].copy_from_slice(&(data_lba as u32).to_le_bytes());

                let mut block_data = [0u8; BLOCK_SIZE];
                let start = b * BLOCK_SIZE;
                let end = core::cmp::min(start + BLOCK_SIZE, data.len());
                let chunk_len = end - start;
                block_data[..chunk_len].copy_from_slice(&data[start..end]);

                wal.atomic_write(&mut self.device, data_lba, &block_data, false)
                    .map_err(|_| FsError::IoFailure)?;
                bytes_written += chunk_len;
            }

            wal.atomic_write(&mut self.device, indirect_lba, &ind_table, false)
                .map_err(|_| FsError::IoFailure)?;

            self.inodes[(inode_id - 1) as usize].inode_type = InodeType::ExtendedFile;
            self.inodes[(inode_id - 1) as usize].direct_blocks[0] = indirect_lba as u32;
            self.inodes[(inode_id - 1) as usize].direct_count = blocks_needed as u32;
            self.inodes[(inode_id - 1) as usize].size = data.len() as u32;

            self.sync_metadata(1)?;
            Ok(bytes_written)
        } else {
            Err(FsError::DiskFull)
        }
    }

    pub fn read_file(&self, name: &str, buf: &mut [u8]) -> Result<usize, FsError> {
        let inode_id = self.find_inode(name)?;
        let inode = &self.inodes[(inode_id - 1) as usize];

        if buf.len() < inode.size as usize {
            return Err(FsError::BufferTooSmall);
        }

        let mut bytes_read = 0;
        let mut remaining = inode.size as usize;
        let max_blocks = self.device.block_count();

        if inode.inode_type == InodeType::ExtendedFile {
            let indirect_lba = inode.direct_blocks[0] as u64;
            if indirect_lba < DATA_START_LBA || indirect_lba >= max_blocks {
                return Err(FsError::CorruptData);
            }
            let mut ind_table = [0u8; BLOCK_SIZE];
            self.device.read_block(indirect_lba, &mut ind_table).map_err(|_| FsError::IoFailure)?;

            for b in 0..(inode.direct_count as usize) {
                let off = b * 4;
                let lba = u32::from_le_bytes([
                    ind_table[off], ind_table[off + 1], ind_table[off + 2], ind_table[off + 3]
                ]) as u64;
                if lba < DATA_START_LBA || lba >= max_blocks {
                    return Err(FsError::CorruptData);
                }

                let mut block_data = [0u8; BLOCK_SIZE];
                self.device.read_block(lba, &mut block_data).map_err(|_| FsError::IoFailure)?;

                let to_read = core::cmp::min(remaining, BLOCK_SIZE);
                buf[bytes_read..bytes_read + to_read].copy_from_slice(&block_data[..to_read]);
                bytes_read += to_read;
                remaining -= to_read;
                if remaining == 0 {
                    break;
                }
            }
            return Ok(bytes_read);
        }

        for b in 0..(inode.direct_count as usize) {
            let lba = inode.direct_blocks[b] as u64;
            if lba < DATA_START_LBA || lba >= max_blocks {
                return Err(FsError::CorruptData);
            }
            let mut block_data = [0u8; BLOCK_SIZE];
            self.device.read_block(lba, &mut block_data).map_err(|_| FsError::IoFailure)?;

            let to_read = core::cmp::min(remaining, BLOCK_SIZE);
            buf[bytes_read..bytes_read + to_read].copy_from_slice(&block_data[..to_read]);
            bytes_read += to_read;
            remaining -= to_read;
        }

        Ok(bytes_read)
    }

    /// Writes file with explicit crash injection point for Lane B (FS-CAPACITY-02 durability testing):
    /// crash_stage:
    ///   1 = Crash during WAL prepare of first data block (uncommitted write discarded)
    ///   2 = Crash after writing 2 payload blocks, before indirect block commit
    ///   3 = Crash after indirect block written via WAL, before inode publication (sync_metadata)
    ///   4 = Full commit including inode publication
    pub fn write_file_atomic_crash_at(&mut self, name: &str, data: &[u8], crash_stage: u8) -> Result<bool, FsError> {
        let inode_id = self.find_inode(name)?;
        let blocks_needed = (data.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;
        let wal = AtomicJournal::new(WAL_START_LBA);

        if blocks_needed <= DIRECT_BLOCKS_PER_INODE {
            let mut target_lbas = [0u64; DIRECT_BLOCKS_PER_INODE];
            for b in 0..blocks_needed {
                let block_lba = self.free_block_cursor;
                if block_lba >= FS_DATA_END_LBA || block_lba >= self.device.block_count() {
                    return Err(FsError::DiskFull);
                }
                self.free_block_cursor += 1;
                target_lbas[b] = block_lba;
            }

            if crash_stage == 1 {
                let mut block_data = [0u8; BLOCK_SIZE];
                let end = core::cmp::min(BLOCK_SIZE, data.len());
                block_data[..end].copy_from_slice(&data[..end]);
                let _ = wal.atomic_write_crash_at(&mut self.device, target_lbas[0], &block_data, 1);
                return Ok(false);
            }

            for b in 0..blocks_needed {
                let mut block_data = [0u8; BLOCK_SIZE];
                let start = b * BLOCK_SIZE;
                let end = core::cmp::min(start + BLOCK_SIZE, data.len());
                let chunk_len = end - start;
                block_data[..chunk_len].copy_from_slice(&data[start..end]);
                wal.atomic_write(&mut self.device, target_lbas[b], &block_data, false)
                    .map_err(|_| FsError::IoFailure)?;
                self.inodes[(inode_id - 1) as usize].direct_blocks[b] = target_lbas[b] as u32;
            }

            if crash_stage == 3 {
                return Ok(false);
            }

            self.inodes[(inode_id - 1) as usize].inode_type = InodeType::RegularFile;
            self.inodes[(inode_id - 1) as usize].direct_count = blocks_needed as u32;
            self.inodes[(inode_id - 1) as usize].size = data.len() as u32;
            self.sync_metadata(1)?;
            Ok(true)
        } else if blocks_needed <= 128 {
            let indirect_lba = self.free_block_cursor;
            if indirect_lba + (blocks_needed as u64) + 1 >= FS_DATA_END_LBA
                || indirect_lba + (blocks_needed as u64) + 1 >= self.device.block_count()
            {
                return Err(FsError::DiskFull);
            }
            self.free_block_cursor += 1;

            if crash_stage == 1 {
                let block_data = [0x55u8; BLOCK_SIZE];
                let _ = wal.atomic_write_crash_at(&mut self.device, self.free_block_cursor, &block_data, 1);
                return Ok(false);
            }

            let mut ind_table = [0u8; BLOCK_SIZE];
            for b in 0..blocks_needed {
                let data_lba = self.free_block_cursor;
                self.free_block_cursor += 1;

                let off = b * 4;
                ind_table[off..off + 4].copy_from_slice(&(data_lba as u32).to_le_bytes());

                let mut block_data = [0u8; BLOCK_SIZE];
                let start = b * BLOCK_SIZE;
                let end = core::cmp::min(start + BLOCK_SIZE, data.len());
                let chunk_len = end - start;
                block_data[..chunk_len].copy_from_slice(&data[start..end]);

                wal.atomic_write(&mut self.device, data_lba, &block_data, false)
                    .map_err(|_| FsError::IoFailure)?;

                if crash_stage == 2 && b == 1 {
                    return Ok(false);
                }
            }

            wal.atomic_write(&mut self.device, indirect_lba, &ind_table, false)
                .map_err(|_| FsError::IoFailure)?;

            if crash_stage == 3 {
                return Ok(false);
            }

            self.inodes[(inode_id - 1) as usize].inode_type = InodeType::ExtendedFile;
            self.inodes[(inode_id - 1) as usize].direct_blocks[0] = indirect_lba as u32;
            self.inodes[(inode_id - 1) as usize].direct_count = blocks_needed as u32;
            self.inodes[(inode_id - 1) as usize].size = data.len() as u32;

            self.sync_metadata(1)?;
            Ok(true)
        } else {
            Err(FsError::DiskFull)
        }
    }

    /// Audits filesystem consistency: verifies indirect pointers, block bounds, and detects double allocations
    pub fn audit_integrity(&self) -> Result<(), FsError> {
        let max_blocks = self.device.block_count();
        if self.free_block_cursor < DATA_START_LBA || self.free_block_cursor > max_blocks {
            return Err(FsError::CorruptData);
        }

        let mut used_lbas = [0u32; 256];
        let mut used_count = 0usize;

        for inode in self.inodes.iter() {
            if inode.inode_type == InodeType::Unused {
                continue;
            }
            if inode.inode_type == InodeType::RegularFile {
                if inode.direct_count as usize > DIRECT_BLOCKS_PER_INODE {
                    return Err(FsError::CorruptData);
                }
                for b in 0..(inode.direct_count as usize) {
                    let lba = inode.direct_blocks[b] as u64;
                    if lba < DATA_START_LBA || lba >= max_blocks {
                        return Err(FsError::CorruptData);
                    }
                    let lba_u32 = lba as u32;
                    for i in 0..used_count {
                        if used_lbas[i] == lba_u32 {
                            return Err(FsError::CorruptData);
                        }
                    }
                    if used_count < used_lbas.len() {
                        used_lbas[used_count] = lba_u32;
                        used_count += 1;
                    }
                }
            } else if inode.inode_type == InodeType::ExtendedFile {
                let ind_lba = inode.direct_blocks[0] as u64;
                if ind_lba < DATA_START_LBA || ind_lba >= max_blocks {
                    return Err(FsError::CorruptData);
                }
                let ind_u32 = ind_lba as u32;
                for i in 0..used_count {
                    if used_lbas[i] == ind_u32 {
                        return Err(FsError::CorruptData);
                    }
                }
                if used_count < used_lbas.len() {
                    used_lbas[used_count] = ind_u32;
                    used_count += 1;
                }

                if inode.direct_count as usize > 128 {
                    return Err(FsError::CorruptData);
                }

                let mut ind_table = [0u8; BLOCK_SIZE];
                self.device.read_block(ind_lba, &mut ind_table).map_err(|_| FsError::IoFailure)?;

                for b in 0..(inode.direct_count as usize) {
                    let off = b * 4;
                    let lba = u32::from_le_bytes([
                        ind_table[off], ind_table[off + 1], ind_table[off + 2], ind_table[off + 3]
                    ]) as u64;
                    if lba < DATA_START_LBA || lba >= max_blocks {
                        return Err(FsError::CorruptData);
                    }
                    let lba_u32 = lba as u32;
                    for i in 0..used_count {
                        if used_lbas[i] == lba_u32 {
                            return Err(FsError::CorruptData);
                        }
                    }
                    if used_count < used_lbas.len() {
                        used_lbas[used_count] = lba_u32;
                        used_count += 1;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn rename_file(&mut self, old_name: &str, new_name: &str) -> Result<(), FsError> {
        if new_name.is_empty() || new_name.len() > MAX_NAME_LEN {
            return Err(FsError::InvalidPath);
        }

        // Check if new name already exists
        if self.find_inode(new_name).is_ok() {
            return Err(FsError::FileAlreadyExists);
        }

        let dir_idx = self.find_dir_entry_idx(old_name)?;
        let mut name_buf = [0u8; MAX_NAME_LEN];
        name_buf[..new_name.len()].copy_from_slice(new_name.as_bytes());

        self.dir_entries[dir_idx].name = name_buf;
        self.dir_entries[dir_idx].name_len = new_name.len() as u8;

        let mut sb_buf = [0u8; BLOCK_SIZE];
        let stage = if self.device.read_block(0, &mut sb_buf).is_ok() {
            let mut stage_bytes = [0u8; 4];
            stage_bytes.copy_from_slice(&sb_buf[12..16]);
            u32::from_ne_bytes(stage_bytes)
        } else {
            1
        };
        let _ = self.sync_metadata(stage);
        Ok(())
    }

    pub fn delete_file(&mut self, name: &str) -> Result<(), FsError> {
        let dir_idx = self.find_dir_entry_idx(name)?;
        let inode_id = self.dir_entries[dir_idx].inode_id;
        let inode = self.inodes[(inode_id - 1) as usize];

        let zero_block = [0u8; BLOCK_SIZE];
        if inode.inode_type == InodeType::RegularFile {
            for b in 0..core::cmp::min(inode.direct_count as usize, DIRECT_BLOCKS_PER_INODE) {
                let lba = inode.direct_blocks[b] as u64;
                if lba >= DATA_START_LBA && lba < self.device.block_count() {
                    let _ = self.device.write_block(lba, &zero_block);
                }
            }
        } else if inode.inode_type == InodeType::ExtendedFile {
            let indirect_lba = inode.direct_blocks[0] as u64;
            if indirect_lba >= DATA_START_LBA && indirect_lba < self.device.block_count() {
                let mut ind_table = [0u8; BLOCK_SIZE];
                if self.device.read_block(indirect_lba, &mut ind_table).is_ok() {
                    for b in 0..core::cmp::min(inode.direct_count as usize, 128) {
                        let off = b * 4;
                        let lba = u32::from_le_bytes([
                            ind_table[off], ind_table[off + 1], ind_table[off + 2], ind_table[off + 3]
                        ]) as u64;
                        if lba >= DATA_START_LBA && lba < self.device.block_count() {
                            let _ = self.device.write_block(lba, &zero_block);
                        }
                    }
                }
                let _ = self.device.write_block(indirect_lba, &zero_block);
            }
        }

        self.dir_entries[dir_idx] = DirEntry::empty();
        self.inodes[(inode_id - 1) as usize] = Inode::empty();
        let _ = self.sync_metadata(1);
        Ok(())
    }

    pub fn file_exists(&self, name: &str) -> bool {
        self.find_inode(name).is_ok()
    }

    pub fn list_files<'a>(&'a self, out: &mut [(&'a str, u32)]) -> usize {
        let mut count = 0;
        for entry in self.dir_entries.iter() {
            if entry.inode_id != 0 && (entry.name_len as usize) <= MAX_NAME_LEN {
                if let Ok(name_str) = core::str::from_utf8(&entry.name[..entry.name_len as usize]) {
                    if !name_str.starts_with('.') {
                        let size = self.inodes[(entry.inode_id - 1) as usize].size;
                        if count < out.len() {
                            out[count] = (name_str, size);
                            count += 1;
                        }
                    }
                }
            }
        }
        count
    }

    fn find_inode(&self, name: &str) -> Result<u32, FsError> {
        for entry in self.dir_entries.iter() {
            if entry.name_len as usize == name.len() && &entry.name[..name.len()] == name.as_bytes() {
                return Ok(entry.inode_id);
            }
        }
        Err(FsError::FileNotFound)
    }

    fn find_dir_entry_idx(&self, name: &str) -> Result<usize, FsError> {
        for (idx, entry) in self.dir_entries.iter().enumerate() {
            if entry.name_len as usize == name.len() && &entry.name[..name.len()] == name.as_bytes() {
                return Ok(idx);
            }
        }
        Err(FsError::FileNotFound)
    }
}

// --- S003: Atomic Write & WAL Crash Recovery ---

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalState {
    Empty = 0,
    Prepared = 1,
    Committed = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalRecord {
    pub state: WalState,
    pub target_lba: u64,
    pub data: [u8; BLOCK_SIZE],
}

impl WalRecord {
    pub const fn empty() -> Self {
        Self {
            state: WalState::Empty,
            target_lba: 0,
            data: [0u8; BLOCK_SIZE],
        }
    }
}

pub struct AtomicJournal {
    wal_lba: u64,
}

impl AtomicJournal {
    pub const fn new(wal_lba: u64) -> Self {
        Self { wal_lba }
    }

    /// Performs atomic write: logs transaction to WAL with Prepared, then commits, then writes target
    pub fn atomic_write<D: BlockDevice>(
        &self,
        device: &mut D,
        target_lba: u64,
        payload: &[u8; BLOCK_SIZE],
        inject_crash_before_commit: bool,
    ) -> Result<bool, BlockError> {
        // 1. Write full 512-byte payload to WAL data block (wal_lba + 1) and header in Prepared state to wal_lba
        device.write_block(self.wal_lba + 1, payload)?;

        let mut wal_block = [0u8; BLOCK_SIZE];
        wal_block[0] = WalState::Prepared as u8;
        wal_block[1..9].copy_from_slice(&target_lba.to_ne_bytes());
        wal_block[9..BLOCK_SIZE].copy_from_slice(&payload[..BLOCK_SIZE - 9]);

        device.write_block(self.wal_lba, &wal_block)?;

        // Injected power loss: crash occurred before commit flag was flushed
        if inject_crash_before_commit {
            return Ok(false); // Uncommitted transaction
        }

        // 2. Commit transaction
        wal_block[0] = WalState::Committed as u8;
        device.write_block(self.wal_lba, &wal_block)?;

        // 3. Write target block and clear WAL state
        device.write_block(target_lba, payload)?;
        wal_block[0] = WalState::Empty as u8;
        device.write_block(self.wal_lba, &wal_block)?;
        Ok(true)
    }

    /// Performs atomic write with injected crash points:
    /// 1 = crash after WAL prepare (uncommitted)
    /// 2 = crash after WAL commit before target write
    pub fn atomic_write_crash_at<D: BlockDevice>(
        &self,
        device: &mut D,
        target_lba: u64,
        payload: &[u8; BLOCK_SIZE],
        crash_point: u8,
    ) -> Result<bool, BlockError> {
        device.write_block(self.wal_lba + 1, payload)?;

        let mut wal_block = [0u8; BLOCK_SIZE];
        wal_block[0] = WalState::Prepared as u8;
        wal_block[1..9].copy_from_slice(&target_lba.to_ne_bytes());
        wal_block[9..BLOCK_SIZE].copy_from_slice(&payload[..BLOCK_SIZE - 9]);

        device.write_block(self.wal_lba, &wal_block)?;

        if crash_point == 1 {
            return Ok(false);
        }

        wal_block[0] = WalState::Committed as u8;
        device.write_block(self.wal_lba, &wal_block)?;

        if crash_point == 2 {
            return Ok(false);
        }

        device.write_block(target_lba, payload)?;
        wal_block[0] = WalState::Empty as u8;
        device.write_block(self.wal_lba, &wal_block)?;
        Ok(true)
    }

    /// Replays WAL on reboot/recovery: if Committed, writes to target; if Prepared, discards
    pub fn recover_on_boot<D: BlockDevice>(&self, device: &mut D) -> Result<bool, BlockError> {
        let mut wal_block = [0u8; BLOCK_SIZE];
        device.read_block(self.wal_lba, &mut wal_block)?;

        let state = wal_block[0];
        if state == (WalState::Committed as u8) {
            let mut target_bytes = [0u8; 8];
            target_bytes.copy_from_slice(&wal_block[1..9]);
            let target_lba = u64::from_ne_bytes(target_bytes);

            let mut payload = [0u8; BLOCK_SIZE];
            device.read_block(self.wal_lba + 1, &mut payload)?;

            device.write_block(target_lba, &payload)?;

            // Clear WAL
            wal_block[0] = WalState::Empty as u8;
            device.write_block(self.wal_lba, &wal_block)?;
            return Ok(true); // Replayed committed write
        } else if state == (WalState::Prepared as u8) {
            // Discard uncommitted transaction
            wal_block[0] = WalState::Empty as u8;
            device.write_block(self.wal_lba, &wal_block)?;
            return Ok(false); // Discarded partial write
        }

        Ok(false)
    }
}

// --- G12: Dedicated Intent Spool Transport Region & Variable-Length Payloads ---

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct IntentSpoolHeader {
    pub magic: u32,               // 0x494E545F ("INT_")
    pub version: u16,             // 1
    pub status: u16,              // 0=Empty, 1=Ready, 2=Processed, 3=Rejected
    pub filename_len: u16,        // length of target filename
    pub filename: [u8; 32],       // target filename
    pub total_content_len: u32,   // variable length up to 4096 bytes
    pub total_sectors: u16,       // number of payload sectors (1..8)
    pub provenance_hash: [u8; 32],// SHA-256 digest of generated tokens
    pub prompt_hash: [u8; 32],    // SHA-256 digest of unseen input prompt
    pub is_ambiguous: u8,         // 1 if ambiguous, 0 if concrete
    pub reserved: [u8; 399],      // Exact 512-byte sector padding
}

impl IntentSpoolHeader {
    pub const fn empty() -> Self {
        Self {
            magic: 0,
            version: 1,
            status: 0,
            filename_len: 0,
            filename: [0; 32],
            total_content_len: 0,
            total_sectors: 0,
            provenance_hash: [0; 32],
            prompt_hash: [0; 32],
            is_ambiguous: 0,
            reserved: [0; 399],
        }
    }
}

pub struct IntentSpoolTransport;

impl IntentSpoolTransport {
    pub fn read_header<D: BlockDevice>(dev: &D) -> Result<IntentSpoolHeader, BlockError> {
        let mut block = [0u8; BLOCK_SIZE];
        dev.read_block(INTENT_SPOOL_START_LBA, &mut block)?;
        let header: IntentSpoolHeader = unsafe { core::ptr::read_unaligned(block.as_ptr() as *const IntentSpoolHeader) };
        Ok(header)
    }

    pub fn write_header<D: BlockDevice>(dev: &mut D, header: &IntentSpoolHeader) -> Result<(), BlockError> {
        let block_bytes = unsafe {
            core::slice::from_raw_parts(header as *const _ as *const u8, BLOCK_SIZE)
        };
        let mut block = [0u8; BLOCK_SIZE];
        block.copy_from_slice(block_bytes);
        dev.write_block(INTENT_SPOOL_START_LBA, &block)
    }

    pub fn read_payload<D: BlockDevice>(dev: &D, header: &IntentSpoolHeader, out_buf: &mut [u8]) -> Result<usize, BlockError> {
        let content_len = header.total_content_len as usize;
        if out_buf.len() < content_len || header.total_sectors == 0 || header.total_sectors > INTENT_SPOOL_MAX_SECTORS {
            return Err(BlockError::InvalidLba);
        }
        let mut bytes_read = 0;
        for s in 0..(header.total_sectors as u64) {
            let mut block = [0u8; BLOCK_SIZE];
            dev.read_block(INTENT_SPOOL_START_LBA + 1 + s, &mut block)?;
            let to_copy = core::cmp::min(content_len - bytes_read, BLOCK_SIZE);
            out_buf[bytes_read..bytes_read + to_copy].copy_from_slice(&block[..to_copy]);
            bytes_read += to_copy;
        }
        Ok(bytes_read)
    }

    pub fn write_packet<D: BlockDevice>(
        dev: &mut D,
        filename: &str,
        payload: &[u8],
        provenance_hash: &[u8; 32],
        prompt_hash: &[u8; 32],
        is_ambiguous: bool,
    ) -> Result<(), BlockError> {
        if payload.len() > (INTENT_SPOOL_MAX_SECTORS as usize * BLOCK_SIZE) {
            return Err(BlockError::InvalidLba);
        }

        let mut header = IntentSpoolHeader::empty();
        header.magic = INTENT_SPOOL_MAGIC;
        header.version = 1;
        header.status = 1; // Ready
        let fn_bytes = filename.as_bytes();
        let fn_len = core::cmp::min(fn_bytes.len(), 32);
        header.filename_len = fn_len as u16;
        header.filename[..fn_len].copy_from_slice(&fn_bytes[..fn_len]);

        header.total_content_len = payload.len() as u32;
        let sectors_needed = if payload.is_empty() { 0 } else { ((payload.len() + BLOCK_SIZE - 1) / BLOCK_SIZE) as u16 };
        header.total_sectors = sectors_needed;
        header.provenance_hash.copy_from_slice(provenance_hash);
        header.prompt_hash.copy_from_slice(prompt_hash);
        header.is_ambiguous = if is_ambiguous { 1 } else { 0 };

        Self::write_header(dev, &header)?;

        for s in 0..(sectors_needed as u64) {
            let mut block = [0u8; BLOCK_SIZE];
            let start = s as usize * BLOCK_SIZE;
            let end = core::cmp::min(start + BLOCK_SIZE, payload.len());
            block[..end - start].copy_from_slice(&payload[start..end]);
            dev.write_block(INTENT_SPOOL_START_LBA + 1 + s, &block)?;
        }

        Ok(())
    }
}

pub static mut GLOBAL_FS_CONTEXT: *mut () = core::ptr::null_mut();
pub static mut GLOBAL_CREATE_FN: Option<unsafe fn(*mut (), &str) -> Result<(), FsError>> = None;
pub static mut GLOBAL_WRITE_ATOMIC_FN: Option<unsafe fn(*mut (), &str, &[u8]) -> Result<usize, FsError>> = None;
pub static mut GLOBAL_READ_FN: Option<unsafe fn(*mut (), &str, &mut [u8]) -> Result<usize, FsError>> = None;
pub static mut GLOBAL_DELETE_FN: Option<unsafe fn(*mut (), &str) -> Result<(), FsError>> = None;
pub static mut GLOBAL_RENAME_FN: Option<unsafe fn(*mut (), &str, &str) -> Result<(), FsError>> = None;
pub static mut GLOBAL_LIST_FN: Option<unsafe fn(*mut (), &mut [u8]) -> usize> = None;
pub static mut GLOBAL_FILE_EXISTS_FN: Option<unsafe fn(*mut (), &str) -> bool> = None;

pub unsafe fn register_global_fs<D: BlockDevice>(fs: &mut InosFileSystem<D>) {
    unsafe fn create_wrapper<D: BlockDevice>(ctx: *mut (), name: &str) -> Result<(), FsError> {
        let fs_ref = &mut *(ctx as *mut InosFileSystem<D>);
        fs_ref.create_file(name).map(|_| ())
    }

    unsafe fn write_wrapper<D: BlockDevice>(ctx: *mut (), name: &str, data: &[u8]) -> Result<usize, FsError> {
        let fs_ref = &mut *(ctx as *mut InosFileSystem<D>);
        fs_ref.write_file_atomic(name, data)
    }

    unsafe fn read_wrapper<D: BlockDevice>(ctx: *mut (), name: &str, buf: &mut [u8]) -> Result<usize, FsError> {
        let fs_ref = &mut *(ctx as *mut InosFileSystem<D>);
        fs_ref.read_file(name, buf)
    }

    unsafe fn delete_wrapper<D: BlockDevice>(ctx: *mut (), name: &str) -> Result<(), FsError> {
        let fs_ref = &mut *(ctx as *mut InosFileSystem<D>);
        fs_ref.delete_file(name)
    }

    unsafe fn rename_wrapper<D: BlockDevice>(ctx: *mut (), old_name: &str, new_name: &str) -> Result<(), FsError> {
        let fs_ref = &mut *(ctx as *mut InosFileSystem<D>);
        fs_ref.rename_file(old_name, new_name)
    }

    unsafe fn list_wrapper<D: BlockDevice>(ctx: *mut (), out_buf: &mut [u8]) -> usize {
        let fs_ref = &*(ctx as *const InosFileSystem<D>);
        let mut entries = [("", 0u32); 32];
        let count = fs_ref.list_files(&mut entries);
        let mut pos = 0usize;
        for i in 0..count {
            let (name, size) = entries[i];
            let nb = name.as_bytes();
            let mut size_digits = [0u8; 12];
            let mut s_len = 0usize;
            let mut val = size;
            if val == 0 {
                size_digits[0] = b'0';
                s_len = 1;
            } else {
                let mut tmp = [0u8; 12];
                let mut t = 0;
                while val > 0 {
                    tmp[t] = b'0' + (val % 10) as u8;
                    val /= 10;
                    t += 1;
                }
                for j in 0..t {
                    size_digits[j] = tmp[t - 1 - j];
                }
                s_len = t;
            }
            if pos + nb.len() + 1 + s_len + 1 > out_buf.len() {
                break;
            }
            out_buf[pos..pos + nb.len()].copy_from_slice(nb);
            pos += nb.len();
            out_buf[pos] = b':';
            pos += 1;
            out_buf[pos..pos + s_len].copy_from_slice(&size_digits[..s_len]);
            pos += s_len;
            out_buf[pos] = b'\n';
            pos += 1;
        }
        pos
    }

    unsafe fn exists_wrapper<D: BlockDevice>(ctx: *mut (), name: &str) -> bool {
        let fs_ref = &mut *(ctx as *mut InosFileSystem<D>);
        fs_ref.file_exists(name)
    }

    GLOBAL_FS_CONTEXT = fs as *mut InosFileSystem<D> as *mut ();
    GLOBAL_CREATE_FN = Some(create_wrapper::<D>);
    GLOBAL_WRITE_ATOMIC_FN = Some(write_wrapper::<D>);
    GLOBAL_READ_FN = Some(read_wrapper::<D>);
    GLOBAL_DELETE_FN = Some(delete_wrapper::<D>);
    GLOBAL_RENAME_FN = Some(rename_wrapper::<D>);
    GLOBAL_LIST_FN = Some(list_wrapper::<D>);
    GLOBAL_FILE_EXISTS_FN = Some(exists_wrapper::<D>);
}

pub unsafe fn global_create_file(name: &str) -> Result<(), FsError> {
    if let Some(create_fn) = GLOBAL_CREATE_FN {
        create_fn(GLOBAL_FS_CONTEXT, name)
    } else {
        Err(FsError::IoFailure)
    }
}

pub unsafe fn global_write_file_atomic(name: &str, data: &[u8]) -> Result<usize, FsError> {
    if let Some(write_fn) = GLOBAL_WRITE_ATOMIC_FN {
        write_fn(GLOBAL_FS_CONTEXT, name, data)
    } else {
        Err(FsError::IoFailure)
    }
}

pub unsafe fn global_read_file(name: &str, buf: &mut [u8]) -> Result<usize, FsError> {
    if transaction::is_file_in_publishing_tx(name) {
        return Err(FsError::FileNotFound);
    }
    if let Some(read_fn) = GLOBAL_READ_FN {
        read_fn(GLOBAL_FS_CONTEXT, name, buf)
    } else {
        Err(FsError::IoFailure)
    }
}

pub unsafe fn global_delete_file(name: &str) -> Result<(), FsError> {
    if let Some(del_fn) = GLOBAL_DELETE_FN {
        del_fn(GLOBAL_FS_CONTEXT, name)
    } else {
        Err(FsError::IoFailure)
    }
}

pub unsafe fn global_rename_file(old_name: &str, new_name: &str) -> Result<(), FsError> {
    if let Some(ren_fn) = GLOBAL_RENAME_FN {
        ren_fn(GLOBAL_FS_CONTEXT, old_name, new_name)
    } else {
        Err(FsError::IoFailure)
    }
}

pub unsafe fn global_list_files(out_buf: &mut [u8]) -> usize {
    if let Some(list_fn) = GLOBAL_LIST_FN {
        list_fn(GLOBAL_FS_CONTEXT, out_buf)
    } else {
        0
    }
}

pub unsafe fn global_file_exists(name: &str) -> bool {
    if transaction::is_file_in_publishing_tx(name) {
        return false;
    }
    if let Some(exists_fn) = GLOBAL_FILE_EXISTS_FN {
        exists_fn(GLOBAL_FS_CONTEXT, name)
    } else {
        false
    }
}

/// Flushes the ATA storage controller cache and polls until command completion.
/// Guarantees that write buffers have physically reached non-volatile media.
pub unsafe fn flush_ata_cache() -> Result<(), BlockError> {
    crate::arch::x86_64::cpu::outb(0x1F6, 0xF0); // Primary slave
    crate::arch::x86_64::cpu::outb(0x1F7, 0xE7); // FLUSH CACHE
    for _ in 0..2_000_000 {
        let status = crate::arch::x86_64::cpu::inb(0x1F7);
        if (status & 0x80) == 0 {
            if (status & 0x01) != 0 {
                return Err(BlockError::IoError);
            }
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(BlockError::DeviceBusy)
}

