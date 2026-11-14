// kernel/src/process/package.rs - Inos Application Package (.inapp) Format & Security Verifier (APP-CAP-02A)

use crate::security::Sha256;
use crate::arch::x86_64::serial::SERIAL1;

pub const INAPP_MAGIC: u32 = 0x494E4150; // "INAP"
pub const INAPP_VERSION: u16 = 1;
pub const MAX_PACKAGE_SIZE: usize = 65536; // 64 KiB package buffer ceiling

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InosAppPackageHeader {
    pub magic: u32,             // 0x494E4150 ("INAP")
    pub format_version: u16,    // 1
    pub flags: u16,             // 0
    pub header_size: u32,       // size_of::<InosAppPackageHeader>() = 192 bytes
    pub package_size: u32,      // total bytes of .inapp file
    pub app_id: [u8; 32],       // package-declared identifier string
    pub app_name: [u8; 32],     // human-readable name string
    pub version: u32,           // package version number
    pub entry_point: u64,       // ELF entry point
    pub code_offset: u32,       // byte offset where ELF begins
    pub code_len: u32,          // size of ELF in bytes
    pub code_sha256: [u8; 32],  // SHA-256 digest of ELF bytes
    pub manifest_offset: u32,   // byte offset of manifest
    pub manifest_len: u32,      // size of manifest
    pub requested_caps: u32,    // requested capability bitmask
    pub reserved: [u8; 48],     // reserved padding for alignment
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageError {
    BufferTooSmall,
    InvalidMagic,
    UnsupportedVersion,
    InvalidHeaderSize,
    SizeMismatch,
    IntegrityViolation,
    UntrustedPublisher,
    MalformedElf,
    WnxViolation,
    OutOfMemory,
}

#[derive(Debug, Clone, Copy)]
pub struct TrustedInstallRecord {
    pub code_sha256: [u8; 32],
    pub trusted_app_id: &'static str,
    pub display_name: &'static str,
    pub allowed_paths: &'static [&'static str],
    pub allowed_caps: u32,
}

// Trusted Application Installation Registry (Kernel-authenticated identities)
// Security Principle: Identity is bound to verified cryptographic code hash,
// NEVER to package-supplied strings.
pub static TRUSTED_INSTALL_REGISTRY: &[TrustedInstallRecord] = &[
    TrustedInstallRecord {
        // Will match dynamically computed SHA-256 of verified notes_app
        code_sha256: [0u8; 32], // Special wildcard/init marker or updated at boot
        trusted_app_id: "org.inos.notes",
        display_name: "Notes",
        allowed_paths: &["notes_output.txt"],
        allowed_caps: 0x03, // Read + Write
    },
    TrustedInstallRecord {
        code_sha256: [0u8; 32],
        trusted_app_id: "org.inos.calc",
        display_name: "Calculator",
        allowed_paths: &["calc_output.txt"],
        allowed_caps: 0x03,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackage<'a> {
    pub authenticated_app_id: &'static str,
    pub display_name: &'static str,
    pub entry_point: u64,
    pub elf_bytes: &'a [u8],
    pub code_sha256: [u8; 32],
    pub is_trusted: bool,
}

pub fn verify_and_authenticate_package<'a>(
    pkg_bytes: &'a [u8],
    trusted_registry: &[TrustedInstallRecord],
) -> Result<VerifiedPackage<'a>, PackageError> {
    if pkg_bytes.len() < core::mem::size_of::<InosAppPackageHeader>() {
        return Err(PackageError::BufferTooSmall);
    }

    let header = unsafe { &*(pkg_bytes.as_ptr() as *const InosAppPackageHeader) };

    if header.magic != INAPP_MAGIC {
        return Err(PackageError::InvalidMagic);
    }
    if header.format_version != INAPP_VERSION {
        return Err(PackageError::UnsupportedVersion);
    }
    if (header.header_size as usize) < core::mem::size_of::<InosAppPackageHeader>() {
        return Err(PackageError::InvalidHeaderSize);
    }
    if (header.package_size as usize) != pkg_bytes.len() {
        return Err(PackageError::SizeMismatch);
    }

    let code_start = header.code_offset as usize;
    let code_end = code_start + header.code_len as usize;
    if code_end > pkg_bytes.len() || code_start > code_end {
        return Err(PackageError::SizeMismatch);
    }

    let elf_slice = &pkg_bytes[code_start..code_end];

    // 1. Verify cryptographic SHA-256 integrity of code payload
    let mut hasher = Sha256::new();
    hasher.update(elf_slice);
    let computed_hash = hasher.finalize();

    if computed_hash != header.code_sha256 {
        SERIAL1.write_str("[PACKAGE_VERIFIER] INTEGRITY FAILURE: Code SHA-256 does not match header!\n");
        return Err(PackageError::IntegrityViolation);
    }

    // 2. Authenticate Identity against Kernel Trusted Installation Registry
    // The package identity is NEVER derived from header.app_id!
    let mut authenticated_id = "sandbox.untrusted";
    let mut display_name = "Untrusted Package";
    let mut is_trusted = false;

    for record in trusted_registry {
        if record.code_sha256 == computed_hash {
            authenticated_id = record.trusted_app_id;
            display_name = record.display_name;
            is_trusted = true;
            break;
        }
    }

    // 3. Strict ELF validation (64-bit, little-endian, canonical memory range, W^X)
    validate_elf_security(elf_slice, header.entry_point)?;

    Ok(VerifiedPackage {
        authenticated_app_id: authenticated_id,
        display_name,
        entry_point: header.entry_point,
        elf_bytes: elf_slice,
        code_sha256: computed_hash,
        is_trusted,
    })
}

pub fn validate_elf_security(elf_bytes: &[u8], expected_entry: u64) -> Result<(), PackageError> {
    if elf_bytes.len() < 64 {
        return Err(PackageError::MalformedElf);
    }
    // ELF Magic: 0x7F, 'E', 'L', 'F'
    if elf_bytes[0..4] != [0x7F, 0x45, 0x4C, 0x46] {
        return Err(PackageError::MalformedElf);
    }
    // 64-bit class
    if elf_bytes[4] != 2 {
        return Err(PackageError::MalformedElf);
    }
    // Little endian
    if elf_bytes[5] != 1 {
        return Err(PackageError::MalformedElf);
    }
    // Machine: x86-64 (0x3E)
    let machine = u16::from_le_bytes([elf_bytes[18], elf_bytes[19]]);
    if machine != 0x3E {
        return Err(PackageError::MalformedElf);
    }

    let mut entry_bytes = [0u8; 8];
    entry_bytes.copy_from_slice(&elf_bytes[24..32]);
    let entry_point = u64::from_le_bytes(entry_bytes);
    if entry_point != expected_entry {
        return Err(PackageError::MalformedElf);
    }

    // Canonical user range check for entry point
    if entry_point < 0x0040_0000 || entry_point >= 0x0060_0000 {
        return Err(PackageError::MalformedElf);
    }

    // Parse program headers
    let mut phoff_bytes = [0u8; 8];
    phoff_bytes.copy_from_slice(&elf_bytes[32..40]);
    let ph_offset = u64::from_le_bytes(phoff_bytes) as usize;

    let mut phentsize_bytes = [0u8; 2];
    phentsize_bytes.copy_from_slice(&elf_bytes[54..56]);
    let ph_entsize = u16::from_le_bytes(phentsize_bytes) as usize;

    let mut phnum_bytes = [0u8; 2];
    phnum_bytes.copy_from_slice(&elf_bytes[56..58]);
    let ph_num = u16::from_le_bytes(phnum_bytes) as usize;

    if ph_entsize < 56 || ph_offset + ph_num * ph_entsize > elf_bytes.len() {
        return Err(PackageError::MalformedElf);
    }

    for i in 0..ph_num {
        let off = ph_offset + i * ph_entsize;
        let p_type = u32::from_le_bytes([
            elf_bytes[off], elf_bytes[off + 1], elf_bytes[off + 2], elf_bytes[off + 3]
        ]);
        let p_flags = u32::from_le_bytes([
            elf_bytes[off + 4], elf_bytes[off + 5], elf_bytes[off + 6], elf_bytes[off + 7]
        ]);
        let p_vaddr = u64::from_le_bytes([
            elf_bytes[off + 16], elf_bytes[off + 17], elf_bytes[off + 18], elf_bytes[off + 19],
            elf_bytes[off + 20], elf_bytes[off + 21], elf_bytes[off + 22], elf_bytes[off + 23],
        ]);
        let p_filesz = u64::from_le_bytes([
            elf_bytes[off + 32], elf_bytes[off + 33], elf_bytes[off + 34], elf_bytes[off + 35],
            elf_bytes[off + 36], elf_bytes[off + 37], elf_bytes[off + 38], elf_bytes[off + 39],
        ]);
        let p_memsz = u64::from_le_bytes([
            elf_bytes[off + 40], elf_bytes[off + 41], elf_bytes[off + 42], elf_bytes[off + 43],
            elf_bytes[off + 44], elf_bytes[off + 45], elf_bytes[off + 46], elf_bytes[off + 47],
        ]);

        if p_type == 1 { // PT_LOAD
            if p_filesz > p_memsz {
                return Err(PackageError::MalformedElf);
            }
            if p_vaddr < 0x0040_0000 || p_vaddr + p_memsz > 0x0060_0000 {
                return Err(PackageError::MalformedElf);
            }

            // W^X Enforcement: PF_W (bit 1) and PF_X (bit 0) cannot both be set
            let is_writable = (p_flags & 2) != 0;
            let is_executable = (p_flags & 1) != 0;
            if is_writable && is_executable {
                SERIAL1.write_str("[PACKAGE_VERIFIER] W^X VIOLATION: Segment is both writable and executable!\n");
                return Err(PackageError::WnxViolation);
            }
        }
    }

    Ok(())
}

pub const PKG_DB_MAGIC: u32 = 0x504B4744; // "PKGD"
pub const PKG_DB_VERSION: u16 = 1;
pub const MAX_PKG_RECORDS: usize = 16;
pub const PKG_DB_FILE: &str = "packages.db";

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PackageDbRecord {
    pub app_id: [u8; 32],
    pub app_id_len: u8,
    pub display_name: [u8; 32],
    pub display_name_len: u8,
    pub version: u32,
    pub prev_version: u32,
    pub code_sha256: [u8; 32],
    pub prev_code_sha256: [u8; 32],
    pub package_path: [u8; 32],
    pub package_path_len: u8,
    pub prev_package_path: [u8; 32],
    pub prev_package_path_len: u8,
    pub allowed_path: [u8; 32],
    pub allowed_path_len: u8,
    pub allowed_caps: u32,
    pub is_active: u8,
    pub reserved: [u8; 31],
}

impl PackageDbRecord {
    pub const fn empty() -> Self {
        Self {
            app_id: [0u8; 32],
            app_id_len: 0,
            display_name: [0u8; 32],
            display_name_len: 0,
            version: 0,
            prev_version: 0,
            code_sha256: [0u8; 32],
            prev_code_sha256: [0u8; 32],
            package_path: [0u8; 32],
            package_path_len: 0,
            prev_package_path: [0u8; 32],
            prev_package_path_len: 0,
            allowed_path: [0u8; 32],
            allowed_path_len: 0,
            allowed_caps: 0,
            is_active: 0,
            reserved: [0u8; 31],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PackageDbHeader {
    pub magic: u32,
    pub version: u16,
    pub record_count: u16,
    pub checksum: u32,
}

pub struct PackageDatabase {
    pub records: [PackageDbRecord; MAX_PKG_RECORDS],
    pub count: usize,
}

impl PackageDatabase {
    pub const fn new() -> Self {
        Self {
            records: [PackageDbRecord::empty(); MAX_PKG_RECORDS],
            count: 0,
        }
    }

    pub fn load<D: crate::storage::BlockDevice>(fs: &mut crate::storage::InosFileSystem<D>) -> Self {
        let mut db = Self::new();
        if !fs.file_exists(PKG_DB_FILE) {
            return db;
        }

        let mut buf = [0u8; 5120];
        let bytes_read = match fs.read_file(PKG_DB_FILE, &mut buf) {
            Ok(b) => b,
            Err(_) => return db,
        };

        if bytes_read < core::mem::size_of::<PackageDbHeader>() {
            return db;
        }

        let header = unsafe { &*(buf.as_ptr() as *const PackageDbHeader) };
        if header.magic != PKG_DB_MAGIC || header.version != PKG_DB_VERSION {
            return db;
        }

        let record_size = core::mem::size_of::<PackageDbRecord>();
        let count = core::cmp::min(header.record_count as usize, MAX_PKG_RECORDS);
        let mut off = core::mem::size_of::<PackageDbHeader>();

        for i in 0..count {
            if off + record_size > bytes_read {
                break;
            }
            unsafe {
                core::ptr::copy_nonoverlapping(
                    buf.as_ptr().add(off),
                    &mut db.records[i] as *mut PackageDbRecord as *mut u8,
                    record_size,
                );
            }
            off += record_size;
            db.count += 1;
        }

        db
    }

    pub fn save<D: crate::storage::BlockDevice>(&self, fs: &mut crate::storage::InosFileSystem<D>) -> Result<(), PackageError> {
        let mut buf = [0u8; 5120];
        let record_size = core::mem::size_of::<PackageDbRecord>();
        let header_size = core::mem::size_of::<PackageDbHeader>();
        let total_size = header_size + self.count * record_size;

        let header = PackageDbHeader {
            magic: PKG_DB_MAGIC,
            version: PKG_DB_VERSION,
            record_count: self.count as u16,
            checksum: 0,
        };

        unsafe {
            core::ptr::copy_nonoverlapping(
                &header as *const PackageDbHeader as *const u8,
                buf.as_mut_ptr(),
                header_size,
            );
        }

        let mut off = header_size;
        for i in 0..self.count {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    &self.records[i] as *const PackageDbRecord as *const u8,
                    buf.as_mut_ptr().add(off),
                    record_size,
                );
            }
            off += record_size;
        }

        if !fs.file_exists(PKG_DB_FILE) {
            fs.create_file(PKG_DB_FILE).map_err(|_| PackageError::OutOfMemory)?;
        }
        fs.write_file_atomic(PKG_DB_FILE, &buf[..total_size]).map_err(|_| PackageError::OutOfMemory)?;
        Ok(())
    }

    pub fn install<D: crate::storage::BlockDevice>(
        &mut self,
        fs: &mut crate::storage::InosFileSystem<D>,
        pkg_file_name: &str,
        pkg_bytes: &[u8],
        allowed_path: &str,
        allowed_caps: u32,
    ) -> Result<(), PackageError> {
        if pkg_bytes.len() < core::mem::size_of::<InosAppPackageHeader>() {
            return Err(PackageError::BufferTooSmall);
        }
        let header = unsafe { &*(pkg_bytes.as_ptr() as *const InosAppPackageHeader) };
        if header.magic != INAPP_MAGIC || header.format_version != INAPP_VERSION {
            return Err(PackageError::InvalidMagic);
        }
        let code_start = header.code_offset as usize;
        let code_end = code_start + header.code_len as usize;
        if code_end > pkg_bytes.len() {
            return Err(PackageError::SizeMismatch);
        }
        let elf_slice = &pkg_bytes[code_start..code_end];
        let mut hasher = Sha256::new();
        hasher.update(elf_slice);
        let computed_hash = hasher.finalize();
        if computed_hash != header.code_sha256 {
            return Err(PackageError::IntegrityViolation);
        }
        validate_elf_security(elf_slice, header.entry_point)?;

        let app_id_len = get_str_len(&header.app_id);
        let app_id_slice = &header.app_id[..app_id_len];
        if app_id_slice.starts_with(b"org.inos.system") || app_id_slice.starts_with(b"system.") {
            return Err(PackageError::UntrustedPublisher);
        }

        for i in 0..self.count {
            if self.records[i].is_active == 1 && &self.records[i].app_id[..self.records[i].app_id_len as usize] == app_id_slice {
                return Err(PackageError::UntrustedPublisher);
            }
        }

        if self.count >= MAX_PKG_RECORDS {
            return Err(PackageError::OutOfMemory);
        }

        if !fs.file_exists(pkg_file_name) {
            fs.create_file(pkg_file_name).map_err(|_| PackageError::OutOfMemory)?;
        }
        fs.write_file_atomic(pkg_file_name, pkg_bytes).map_err(|_| PackageError::OutOfMemory)?;

        let mut rec = PackageDbRecord::empty();
        let app_id_len = get_str_len(&header.app_id);
        rec.app_id[..app_id_len].copy_from_slice(&header.app_id[..app_id_len]);
        rec.app_id_len = app_id_len as u8;

        let name_len = get_str_len(&header.app_name);
        rec.display_name[..name_len].copy_from_slice(&header.app_name[..name_len]);
        rec.display_name_len = name_len as u8;

        rec.version = header.version;
        rec.code_sha256 = computed_hash;

        let pkg_path_len = core::cmp::min(pkg_file_name.len(), 32);
        rec.package_path[..pkg_path_len].copy_from_slice(&pkg_file_name.as_bytes()[..pkg_path_len]);
        rec.package_path_len = pkg_path_len as u8;

        let p_len = core::cmp::min(allowed_path.len(), 32);
        rec.allowed_path[..p_len].copy_from_slice(&allowed_path.as_bytes()[..p_len]);
        rec.allowed_path_len = p_len as u8;

        rec.allowed_caps = allowed_caps;
        rec.is_active = 1;

        let idx = self.count;
        self.records[idx] = rec;
        self.count += 1;

        self.save(fs)?;
        Ok(())
    }

    pub fn update<D: crate::storage::BlockDevice>(
        &mut self,
        fs: &mut crate::storage::InosFileSystem<D>,
        new_pkg_file_name: &str,
        new_pkg_bytes: &[u8],
    ) -> Result<(), PackageError> {
        let header = unsafe { &*(new_pkg_bytes.as_ptr() as *const InosAppPackageHeader) };
        if header.magic != INAPP_MAGIC {
            return Err(PackageError::InvalidMagic);
        }
        let code_start = header.code_offset as usize;
        let code_end = code_start + header.code_len as usize;
        let elf_slice = &new_pkg_bytes[code_start..code_end];
        let mut hasher = Sha256::new();
        hasher.update(elf_slice);
        let computed_hash = hasher.finalize();
        if computed_hash != header.code_sha256 {
            return Err(PackageError::IntegrityViolation);
        }
        validate_elf_security(elf_slice, header.entry_point)?;

        let app_id_bytes = &header.app_id[..get_str_len(&header.app_id)];
        for i in 0..self.count {
            if self.records[i].is_active == 1 && &self.records[i].app_id[..self.records[i].app_id_len as usize] == app_id_bytes {
                if header.version <= self.records[i].version {
                    return Err(PackageError::UnsupportedVersion);
                }

                self.records[i].prev_version = self.records[i].version;
                self.records[i].prev_code_sha256 = self.records[i].code_sha256;
                self.records[i].prev_package_path = self.records[i].package_path;
                self.records[i].prev_package_path_len = self.records[i].package_path_len;

                if !fs.file_exists(new_pkg_file_name) {
                    fs.create_file(new_pkg_file_name).map_err(|_| PackageError::OutOfMemory)?;
                }
                fs.write_file_atomic(new_pkg_file_name, new_pkg_bytes).map_err(|_| PackageError::OutOfMemory)?;

                self.records[i].version = header.version;
                self.records[i].code_sha256 = computed_hash;
                let pkg_path_len = core::cmp::min(new_pkg_file_name.len(), 32);
                self.records[i].package_path = [0u8; 32];
                self.records[i].package_path[..pkg_path_len].copy_from_slice(&new_pkg_file_name.as_bytes()[..pkg_path_len]);
                self.records[i].package_path_len = pkg_path_len as u8;

                self.save(fs)?;
                return Ok(());
            }
        }

        Err(PackageError::UntrustedPublisher)
    }

    pub fn rollback<D: crate::storage::BlockDevice>(
        &mut self,
        fs: &mut crate::storage::InosFileSystem<D>,
        app_id: &str,
    ) -> Result<(), PackageError> {
        for i in 0..self.count {
            if self.records[i].is_active == 1 && &self.records[i].app_id[..self.records[i].app_id_len as usize] == app_id.as_bytes() {
                if self.records[i].prev_version == 0 {
                    return Err(PackageError::UnsupportedVersion);
                }

                self.records[i].version = self.records[i].prev_version;
                self.records[i].code_sha256 = self.records[i].prev_code_sha256;
                self.records[i].package_path = self.records[i].prev_package_path;
                self.records[i].package_path_len = self.records[i].prev_package_path_len;
                self.records[i].prev_version = 0;

                self.save(fs)?;
                return Ok(());
            }
        }
        Err(PackageError::UntrustedPublisher)
    }

    pub fn uninstall<D: crate::storage::BlockDevice>(
        &mut self,
        fs: &mut crate::storage::InosFileSystem<D>,
        app_id: &str,
    ) -> Result<(), PackageError> {
        for i in 0..self.count {
            if self.records[i].is_active == 1 && &self.records[i].app_id[..self.records[i].app_id_len as usize] == app_id.as_bytes() {
                self.records[i].is_active = 0;
                let pkg_name = core::str::from_utf8(&self.records[i].package_path[..self.records[i].package_path_len as usize]).unwrap_or("");
                if !pkg_name.is_empty() && fs.file_exists(pkg_name) {
                    let _ = fs.delete_file(pkg_name);
                }
                self.save(fs)?;
                return Ok(());
            }
        }
        Err(PackageError::UntrustedPublisher)
    }

    pub fn find_by_code_sha256(&self, hash: &[u8; 32]) -> Option<&PackageDbRecord> {
        for i in 0..self.count {
            if self.records[i].is_active == 1 && &self.records[i].code_sha256 == hash {
                return Some(&self.records[i]);
            }
        }
        None
    }
}

fn get_str_len(buf: &[u8]) -> usize {
    let mut len = 0;
    while len < buf.len() && buf[len] != 0 {
        len += 1;
    }
    len
}
