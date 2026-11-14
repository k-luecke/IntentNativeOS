// tools/inapp_packager/src/main.rs - Rust-based INOS Application Packager (Zero Python)
// Packages an ELF binary into an authenticated .inapp package format

use std::env;
use std::fs;
use std::process;

pub const INAPP_MAGIC: u32 = 0x494E4150; // "INAP"
pub const INAPP_VERSION: u16 = 1;

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

// Pure standard library SHA-256
pub struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            count: 0,
            buffer: [0; 64],
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        for &byte in data {
            let idx = (self.count % 64) as usize;
            self.buffer[idx] = byte;
            self.count += 1;
            if self.count % 64 == 0 {
                let chunk = self.buffer;
                self.transform(&chunk);
            }
        }
    }

    fn transform(&mut self, chunk: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.count * 8;
        let buf_idx = (self.count % 64) as usize;
        let pad_len = if buf_idx < 56 { 56 - buf_idx } else { 120 - buf_idx };

        self.update(&[0x80]);
        for _ in 1..pad_len {
            self.update(&[0x00]);
        }
        self.update(&bit_len.to_be_bytes());

        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

fn parse_elf_entry(elf_bytes: &[u8]) -> Result<u64, String> {
    if elf_bytes.len() < 64 {
        return Err("ELF file too small".into());
    }
    if &elf_bytes[0..4] != b"\x7FELF" {
        return Err("Not a valid ELF binary".into());
    }
    if elf_bytes[4] != 2 {
        return Err("Only 64-bit ELF supported".into());
    }
    let mut entry_bytes = [0u8; 8];
    entry_bytes.copy_from_slice(&elf_bytes[24..32]);
    Ok(u64::from_le_bytes(entry_bytes))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut elf_path = String::new();
    let mut app_id = String::new();
    let mut app_name = String::new();
    let mut out_path = String::new();
    let mut version = 1u32;
    let mut caps = 0u32;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--elf" => {
                i += 1;
                if i < args.len() { elf_path = args[i].clone(); }
            }
            "--app-id" => {
                i += 1;
                if i < args.len() { app_id = args[i].clone(); }
            }
            "--name" => {
                i += 1;
                if i < args.len() { app_name = args[i].clone(); }
            }
            "--version" => {
                i += 1;
                if i < args.len() { version = args[i].parse().unwrap_or(1); }
            }
            "--caps" => {
                i += 1;
                if i < args.len() { caps = args[i].parse().unwrap_or(0); }
            }
            "--out" => {
                i += 1;
                if i < args.len() { out_path = args[i].clone(); }
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    if elf_path.is_empty() || app_id.is_empty() || out_path.is_empty() {
        eprintln!("Usage: inapp_packager --elf <path> --app-id <id> --name <name> --version <ver> --caps <bitmask> --out <output.inapp>");
        process::exit(1);
    }

    let elf_bytes = match fs::read(&elf_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to read ELF '{}': {}", elf_path, e);
            process::exit(1);
        }
    };

    let entry_point = match parse_elf_entry(&elf_bytes) {
        Ok(ep) => ep,
        Err(e) => {
            eprintln!("Error parsing ELF: {}", e);
            process::exit(1);
        }
    };

    let mut hasher = Sha256::new();
    hasher.update(&elf_bytes);
    let code_sha256 = hasher.finalize();

    let manifest = format!("{{\"app_id\":\"{}\",\"name\":\"{}\",\"version\":{}}}", app_id, app_name, version);
    let manifest_bytes = manifest.as_bytes();

    let header_size = std::mem::size_of::<InosAppPackageHeader>() as u32;
    let code_offset = header_size;
    let code_len = elf_bytes.len() as u32;
    let manifest_offset = code_offset + code_len;
    let manifest_len = manifest_bytes.len() as u32;
    let package_size = manifest_offset + manifest_len;

    let mut app_id_bytes = [0u8; 32];
    let id_slice = app_id.as_bytes();
    let id_len = std::cmp::min(id_slice.len(), 32);
    app_id_bytes[..id_len].copy_from_slice(&id_slice[..id_len]);

    let mut app_name_bytes = [0u8; 32];
    let name_slice = app_name.as_bytes();
    let name_len = std::cmp::min(name_slice.len(), 32);
    app_name_bytes[..name_len].copy_from_slice(&name_slice[..name_len]);

    let header = InosAppPackageHeader {
        magic: INAPP_MAGIC,
        format_version: INAPP_VERSION,
        flags: 0,
        header_size,
        package_size,
        app_id: app_id_bytes,
        app_name: app_name_bytes,
        version,
        entry_point,
        code_offset,
        code_len,
        code_sha256,
        manifest_offset,
        manifest_len,
        requested_caps: caps,
        reserved: [0u8; 48],
    };

    let mut package_data = Vec::with_capacity(package_size as usize);
    let header_slice = unsafe {
        std::slice::from_raw_parts(
            &header as *const InosAppPackageHeader as *const u8,
            header_size as usize,
        )
    };
    package_data.extend_from_slice(header_slice);
    package_data.extend_from_slice(&elf_bytes);
    package_data.extend_from_slice(manifest_bytes);

    if let Err(e) = fs::write(&out_path, &package_data) {
        eprintln!("Failed to write package '{}': {}", out_path, e);
        process::exit(1);
    }

    println!("[INAPP_PACKAGER] Packaged '{}' ({} bytes) -> '{}'", app_id, package_size, out_path);
    print!("[INAPP_PACKAGER] Code SHA-256: ");
    for b in code_sha256 {
        print!("{:02x}", b);
    }
    println!();
}
