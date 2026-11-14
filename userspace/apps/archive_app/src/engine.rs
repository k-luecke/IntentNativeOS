// userspace/apps/archive_app/src/engine.rs - Freestanding RFC 1951 Deflate & ZIP Archive Engine
// Non-negotiable security defenses: Zip-Slip path traversal defense, decompression bomb limits,
// and full RFC 1951 Dynamic Huffman (Block Type 2) & Central Directory support.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveError {
    BufferTooSmall,
    CorruptData,
    CrcMismatch,
    ZipSlipDetected,
    DecompressionBombDetected,
    UnsupportedCompressionMethod,
    UnsupportedDynamicHuffman,
    InvalidZipHeader,
    EntryNotFound,
}

pub const MAX_UNCOMPRESSED_CEILING: usize = 65536; // 64 KiB safety limit
pub const MAX_COMPRESSION_RATIO: usize = 100;     // 100:1 ratio limit

// --- 1. IEEE 802.3 CRC-32 ---

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB88320 & mask);
        }
    }
    !crc
}

// --- 2. Bit Reader for RFC 1951 Deflate ---

pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bits_in_buf: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    pub fn read_bits(&mut self, n: u8) -> Result<u32, ArchiveError> {
        while self.bits_in_buf < n {
            if self.byte_pos >= self.data.len() {
                return Err(ArchiveError::CorruptData);
            }
            self.bit_buf |= (self.data[self.byte_pos] as u32) << self.bits_in_buf;
            self.byte_pos += 1;
            self.bits_in_buf += 8;
        }
        let mask = (1u32 << n) - 1;
        let val = self.bit_buf & mask;
        self.bit_buf >>= n;
        self.bits_in_buf -= n;
        Ok(val)
    }

    pub fn align_to_byte(&mut self) {
        self.bit_buf = 0;
        self.bits_in_buf = 0;
    }

    pub fn read_u16_le(&mut self) -> Result<u16, ArchiveError> {
        self.align_to_byte();
        if self.byte_pos + 2 > self.data.len() {
            return Err(ArchiveError::CorruptData);
        }
        let b0 = self.data[self.byte_pos] as u16;
        let b1 = self.data[self.byte_pos + 1] as u16;
        self.byte_pos += 2;
        Ok(b0 | (b1 << 8))
    }

    pub fn read_bytes(&mut self, dest: &mut [u8]) -> Result<(), ArchiveError> {
        self.align_to_byte();
        if self.byte_pos + dest.len() > self.data.len() {
            return Err(ArchiveError::CorruptData);
        }
        dest.copy_from_slice(&self.data[self.byte_pos..self.byte_pos + dest.len()]);
        self.byte_pos += dest.len();
        Ok(())
    }
}

// --- 3. Canonical Huffman Tree Decoder (RFC 1951 Section 3.2.2) ---

pub struct HuffmanTree {
    nodes: [[u16; 2]; 576],
    symbols: [u16; 576],
    is_leaf: [bool; 576],
    next_node: usize,
}

impl HuffmanTree {
    pub const fn empty() -> Self {
        Self {
            nodes: [[0; 2]; 576],
            symbols: [0; 576],
            is_leaf: [false; 576],
            next_node: 1,
        }
    }

    pub fn from_lengths(lengths: &[u8]) -> Result<Self, ArchiveError> {
        let mut tree = Self::empty();
        let mut bl_count = [0u16; 16];
        for &len in lengths {
            if len > 15 {
                return Err(ArchiveError::CorruptData);
            }
            if len > 0 {
                bl_count[len as usize] += 1;
            }
        }

        let mut code = 0u16;
        let mut next_code = [0u16; 16];
        for bits in 1..=15 {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        let mut active_count = 0;
        let mut last_sym = 0u16;
        for (sym, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            active_count += 1;
            last_sym = sym as u16;
            let c = next_code[len as usize];
            next_code[len as usize] += 1;

            let mut curr = 0usize;
            for bit_pos in (0..len).rev() {
                let bit = ((c >> bit_pos) & 1) as usize;
                if tree.nodes[curr][bit] == 0 {
                    if tree.next_node >= 576 {
                        return Err(ArchiveError::CorruptData);
                    }
                    let new_node = tree.next_node;
                    tree.next_node += 1;
                    tree.nodes[curr][bit] = new_node as u16;
                }
                curr = tree.nodes[curr][bit] as usize;
            }
            tree.is_leaf[curr] = true;
            tree.symbols[curr] = sym as u16;
        }

        // Single symbol edge case (RFC 1951): allow both 0 and 1 paths
        if active_count == 1 {
            tree.nodes[0][0] = 1;
            tree.nodes[0][1] = 1;
            tree.is_leaf[1] = true;
            tree.symbols[1] = last_sym;
            tree.next_node = 2;
        }

        Ok(tree)
    }

    pub fn decode(&self, reader: &mut BitReader) -> Result<u16, ArchiveError> {
        let mut curr = 0usize;
        while !self.is_leaf[curr] {
            let bit = reader.read_bits(1)? as usize;
            let next = self.nodes[curr][bit] as usize;
            if next == 0 {
                return Err(ArchiveError::CorruptData);
            }
            curr = next;
        }
        Ok(self.symbols[curr])
    }
}

// --- 4. RFC 1951 Deflate Decompressor ---

const CODE_LEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15
];

pub fn deflate_decompress(compressed: &[u8], output: &mut [u8]) -> Result<usize, ArchiveError> {
    let mut reader = BitReader::new(compressed);
    let mut out_pos = 0usize;
    let mut is_final = false;

    while !is_final {
        let bfinal = reader.read_bits(1)?;
        is_final = bfinal == 1;

        let btype = reader.read_bits(2)?;
        match btype {
            0 => {
                // Non-compressed block (BTYPE = 00)
                let len = reader.read_u16_le()? as usize;
                let nlen = reader.read_u16_le()? as usize;
                if (len ^ 0xFFFF) != nlen {
                    return Err(ArchiveError::CorruptData);
                }
                if out_pos + len > output.len() {
                    return Err(ArchiveError::BufferTooSmall);
                }
                reader.read_bytes(&mut output[out_pos..out_pos + len])?;
                out_pos += len;
            }
            1 => {
                // Fixed Huffman codes (BTYPE = 01)
                loop {
                    let lit_symbol = decode_fixed_literal(&mut reader)?;
                    if lit_symbol < 256 {
                        if out_pos >= output.len() {
                            return Err(ArchiveError::BufferTooSmall);
                        }
                        output[out_pos] = lit_symbol as u8;
                        out_pos += 1;
                    } else if lit_symbol == 256 {
                        break;
                    } else if lit_symbol <= 285 {
                        let length = decode_lz77_length(lit_symbol, &mut reader)?;
                        let distance = decode_fixed_distance(&mut reader)?;
                        if distance > out_pos || out_pos + length > output.len() {
                            return Err(ArchiveError::CorruptData);
                        }
                        for _ in 0..length {
                            let b = output[out_pos - distance];
                            output[out_pos] = b;
                            out_pos += 1;
                        }
                    } else {
                        return Err(ArchiveError::CorruptData);
                    }
                }
            }
            2 => {
                // Dynamic Huffman codes (BTYPE = 10) - RFC 1951 Section 3.2.7
                let hlit = reader.read_bits(5)? as usize + 257;
                let hdist = reader.read_bits(5)? as usize + 1;
                let hclen = reader.read_bits(4)? as usize + 4;

                if hlit > 286 || hdist > 32 || hclen > 19 {
                    return Err(ArchiveError::CorruptData);
                }

                let mut cl_lengths = [0u8; 19];
                for i in 0..hclen {
                    cl_lengths[CODE_LEN_ORDER[i]] = reader.read_bits(3)? as u8;
                }

                let cl_tree = HuffmanTree::from_lengths(&cl_lengths)?;

                let total_codes = hlit + hdist;
                let mut all_lengths = [0u8; 320];
                let mut idx = 0usize;

                while idx < total_codes {
                    let sym = cl_tree.decode(&mut reader)?;
                    match sym {
                        0..=15 => {
                            all_lengths[idx] = sym as u8;
                            idx += 1;
                        }
                        16 => {
                            if idx == 0 {
                                return Err(ArchiveError::CorruptData);
                            }
                            let prev = all_lengths[idx - 1];
                            let repeat = 3 + reader.read_bits(2)? as usize;
                            if idx + repeat > total_codes {
                                return Err(ArchiveError::CorruptData);
                            }
                            for _ in 0..repeat {
                                all_lengths[idx] = prev;
                                idx += 1;
                            }
                        }
                        17 => {
                            let repeat = 3 + reader.read_bits(3)? as usize;
                            if idx + repeat > total_codes {
                                return Err(ArchiveError::CorruptData);
                            }
                            for _ in 0..repeat {
                                all_lengths[idx] = 0;
                                idx += 1;
                            }
                        }
                        18 => {
                            let repeat = 11 + reader.read_bits(7)? as usize;
                            if idx + repeat > total_codes {
                                return Err(ArchiveError::CorruptData);
                            }
                            for _ in 0..repeat {
                                all_lengths[idx] = 0;
                                idx += 1;
                            }
                        }
                        _ => return Err(ArchiveError::CorruptData),
                    }
                }

                let lit_tree = HuffmanTree::from_lengths(&all_lengths[..hlit])?;
                let dist_tree = HuffmanTree::from_lengths(&all_lengths[hlit..total_codes])?;

                loop {
                    let lit_symbol = lit_tree.decode(&mut reader)?;
                    if lit_symbol < 256 {
                        if out_pos >= output.len() {
                            return Err(ArchiveError::BufferTooSmall);
                        }
                        output[out_pos] = lit_symbol as u8;
                        out_pos += 1;
                    } else if lit_symbol == 256 {
                        break;
                    } else if lit_symbol <= 285 {
                        let length = decode_lz77_length(lit_symbol, &mut reader)?;
                        let dist_sym = dist_tree.decode(&mut reader)?;
                        let distance = decode_lz77_distance(dist_sym, &mut reader)?;
                        if distance > out_pos || out_pos + length > output.len() {
                            return Err(ArchiveError::CorruptData);
                        }
                        for _ in 0..length {
                            let b = output[out_pos - distance];
                            output[out_pos] = b;
                            out_pos += 1;
                        }
                    } else {
                        return Err(ArchiveError::CorruptData);
                    }
                }
            }
            _ => {
                // Reserved error (BTYPE = 11)
                return Err(ArchiveError::CorruptData);
            }
        }
    }

    Ok(out_pos)
}

fn decode_fixed_literal(reader: &mut BitReader) -> Result<u16, ArchiveError> {
    let val7 = reader.read_bits(7)?;
    if val7 <= 0b0010111 {
        return Ok(256 + reverse_bits(val7, 7) as u16);
    }
    let bit8 = reader.read_bits(1)?;
    let val8 = val7 | (bit8 << 7);
    let rev8 = reverse_bits(val8, 8);
    if rev8 >= 0b00110000 && rev8 <= 0b10111111 {
        return Ok((rev8 - 0b00110000) as u16);
    }
    if rev8 >= 0b11000000 && rev8 <= 0b11000111 {
        return Ok((280 + (rev8 - 0b11000000)) as u16);
    }
    let bit9 = reader.read_bits(1)?;
    let val9 = val8 | (bit9 << 8);
    let rev9 = reverse_bits(val9, 9);
    if rev9 >= 0b110010000 && rev9 <= 0b111111111 {
        return Ok((144 + (rev9 - 0b110010000)) as u16);
    }
    Err(ArchiveError::CorruptData)
}

fn reverse_bits(val: u32, n: u8) -> u32 {
    let mut res = 0;
    let mut v = val;
    for _ in 0..n {
        res = (res << 1) | (v & 1);
        v >>= 1;
    }
    res
}

fn decode_lz77_length(code: u16, reader: &mut BitReader) -> Result<usize, ArchiveError> {
    let (base_len, extra_bits) = match code {
        257..=264 => (3 + (code - 257) as usize, 0),
        265..=268 => (11 + ((code - 265) as usize * 2), 1),
        269..=272 => (19 + ((code - 269) as usize * 4), 2),
        273..=276 => (35 + ((code - 273) as usize * 8), 3),
        277..=280 => (67 + ((code - 277) as usize * 16), 4),
        281..=284 => (131 + ((code - 281) as usize * 32), 5),
        285 => (258, 0),
        _ => return Err(ArchiveError::CorruptData),
    };
    let extra = if extra_bits > 0 { reader.read_bits(extra_bits)? as usize } else { 0 };
    Ok(base_len + extra)
}

fn decode_fixed_distance(reader: &mut BitReader) -> Result<usize, ArchiveError> {
    let dist_code = reverse_bits(reader.read_bits(5)?, 5) as u16;
    decode_lz77_distance(dist_code, reader)
}

fn decode_lz77_distance(dist_code: u16, reader: &mut BitReader) -> Result<usize, ArchiveError> {
    let (base_dist, dist_extra_bits) = match dist_code {
        0..=3 => (dist_code as usize + 1, 0),
        4..=5 => (5 + ((dist_code as usize - 4) * 2), 1),
        6..=7 => (9 + ((dist_code as usize - 6) * 4), 2),
        8..=9 => (17 + ((dist_code as usize - 8) * 8), 3),
        10..=11 => (33 + ((dist_code as usize - 10) * 16), 4),
        12..=13 => (65 + ((dist_code as usize - 12) * 32), 5),
        14..=15 => (129 + ((dist_code as usize - 14) * 64), 6),
        16..=17 => (257 + ((dist_code as usize - 16) * 128), 7),
        18..=19 => (513 + ((dist_code as usize - 18) * 256), 8),
        20..=21 => (1025 + ((dist_code as usize - 20) * 512), 9),
        22..=23 => (2049 + ((dist_code as usize - 22) * 1024), 10),
        24..=25 => (4097 + ((dist_code as usize - 24) * 2048), 11),
        26..=27 => (8193 + ((dist_code as usize - 26) * 4096), 12),
        28..=29 => (16385 + ((dist_code as usize - 28) * 8192), 13),
        _ => return Err(ArchiveError::CorruptData),
    };
    let dist_extra = if dist_extra_bits > 0 { reader.read_bits(dist_extra_bits)? as usize } else { 0 };
    Ok(base_dist + dist_extra)
}

// --- 5. RFC 1951 Deflate Compressor (Fast uncompressed block) ---

pub fn deflate_compress(input: &[u8], output: &mut [u8]) -> Result<usize, ArchiveError> {
    if input.len() > 65535 {
        return Err(ArchiveError::BufferTooSmall);
    }
    let needed = 5 + input.len();
    if output.len() < needed {
        return Err(ArchiveError::BufferTooSmall);
    }

    output[0] = 0x01; // BFINAL=1, BTYPE=00
    let len = input.len() as u16;
    let nlen = !len;
    output[1..3].copy_from_slice(&len.to_le_bytes());
    output[3..5].copy_from_slice(&nlen.to_le_bytes());
    output[5..5 + input.len()].copy_from_slice(input);

    Ok(needed)
}

// --- 6. ZIP Archive Engine & Security Defenses ---

pub fn is_zip_slip_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.is_empty() || bytes[0] == b'/' || bytes[0] == b'\\' {
        return true;
    }
    for i in 0..bytes.len() {
        if bytes[i] == b':' {
            return true;
        }
        if bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b'.' {
            let prev_sep = i == 0 || bytes[i - 1] == b'/' || bytes[i - 1] == b'\\';
            let next_sep = i + 2 == bytes.len() || bytes[i + 2] == b'/' || bytes[i + 2] == b'\\';
            if prev_sep && next_sep {
                return true;
            }
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipEntry<'a> {
    pub name: &'a str,
    pub uncompressed_size: u32,
    pub compressed_size: u32,
    pub crc32: u32,
    pub compression_method: u16, // 0 = stored, 8 = deflated
    pub data_offset: usize,
}

pub fn parse_zip_file<'a>(zip_data: &'a [u8]) -> Result<ZipEntry<'a>, ArchiveError> {
    if zip_data.len() < 30 {
        return Err(ArchiveError::InvalidZipHeader);
    }
    // Local File Header Signature: 0x04034B50 ("PK\x03\x04")
    if zip_data[0] != 0x50 || zip_data[1] != 0x4B || zip_data[2] != 0x03 || zip_data[3] != 0x04 {
        return Err(ArchiveError::InvalidZipHeader);
    }

    let comp_method = u16::from_le_bytes([zip_data[8], zip_data[9]]);
    let crc = u32::from_le_bytes([zip_data[14], zip_data[15], zip_data[16], zip_data[17]]);
    let comp_size = u32::from_le_bytes([zip_data[18], zip_data[19], zip_data[20], zip_data[21]]);
    let uncomp_size = u32::from_le_bytes([zip_data[22], zip_data[23], zip_data[24], zip_data[25]]);
    let name_len = u16::from_le_bytes([zip_data[26], zip_data[27]]) as usize;
    let extra_len = u16::from_le_bytes([zip_data[28], zip_data[29]]) as usize;

    let name_start = 30;
    let name_end = name_start + name_len;
    if name_end + extra_len > zip_data.len() {
        return Err(ArchiveError::InvalidZipHeader);
    }

    let name_str = core::str::from_utf8(&zip_data[name_start..name_end])
        .map_err(|_| ArchiveError::InvalidZipHeader)?;

    if is_zip_slip_path(name_str) {
        return Err(ArchiveError::ZipSlipDetected);
    }

    if (uncomp_size as usize) > MAX_UNCOMPRESSED_CEILING {
        return Err(ArchiveError::DecompressionBombDetected);
    }
    if comp_size > 0 && (uncomp_size as usize / comp_size as usize) > MAX_COMPRESSION_RATIO {
        return Err(ArchiveError::DecompressionBombDetected);
    }

    let data_offset = name_end + extra_len;
    Ok(ZipEntry {
        name: name_str,
        uncompressed_size: uncomp_size,
        compressed_size: comp_size,
        crc32: crc,
        compression_method: comp_method,
        data_offset,
    })
}

// --- 7. Multi-File ZIP Central Directory Parser & Extractor ---

#[derive(Debug, Clone, Copy)]
pub struct CentralDirEntry<'a> {
    pub filename: &'a str,
    pub compression_method: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub local_header_offset: u32,
}

pub struct ZipArchive<'a> {
    pub entries: [Option<CentralDirEntry<'a>>; 16],
    pub count: usize,
}

impl<'a> ZipArchive<'a> {
    pub fn find_entry(&self, name: &str) -> Option<&CentralDirEntry<'a>> {
        for i in 0..self.count {
            if let Some(ref entry) = self.entries[i] {
                if entry.filename == name {
                    return Some(entry);
                }
            }
        }
        None
    }
}

pub fn parse_zip_central_directory<'a>(data: &'a [u8]) -> Result<ZipArchive<'a>, ArchiveError> {
    if data.len() < 22 {
        return Err(ArchiveError::InvalidZipHeader);
    }

    let max_search = core::cmp::min(data.len() - 22, 65536);
    let mut eocd_pos = None;
    for i in 0..=max_search {
        let pos = data.len() - 22 - i;
        if data[pos] == 0x50 && data[pos + 1] == 0x4B && data[pos + 2] == 0x05 && data[pos + 3] == 0x06 {
            eocd_pos = Some(pos);
            break;
        }
    }

    let eocd = eocd_pos.ok_or(ArchiveError::InvalidZipHeader)?;
    let total_entries = u16::from_le_bytes([data[eocd + 10], data[eocd + 11]]) as usize;
    let cd_size = u32::from_le_bytes([data[eocd + 12], data[eocd + 13], data[eocd + 14], data[eocd + 15]]) as usize;
    let cd_offset = u32::from_le_bytes([data[eocd + 16], data[eocd + 17], data[eocd + 18], data[eocd + 19]]) as usize;

    if cd_offset + cd_size > data.len() {
        return Err(ArchiveError::InvalidZipHeader);
    }

    let mut archive = ZipArchive {
        entries: [None; 16],
        count: 0,
    };

    let mut pos = cd_offset;
    for _ in 0..total_entries {
        if pos + 46 > cd_offset + cd_size {
            return Err(ArchiveError::InvalidZipHeader);
        }
        if data[pos] != 0x50 || data[pos + 1] != 0x4B || data[pos + 2] != 0x01 || data[pos + 3] != 0x02 {
            return Err(ArchiveError::InvalidZipHeader);
        }

        let method = u16::from_le_bytes([data[pos + 10], data[pos + 11]]);
        let crc = u32::from_le_bytes([data[pos + 16], data[pos + 17], data[pos + 18], data[pos + 19]]);
        let comp_size = u32::from_le_bytes([data[pos + 20], data[pos + 21], data[pos + 22], data[pos + 23]]);
        let uncomp_size = u32::from_le_bytes([data[pos + 24], data[pos + 25], data[pos + 26], data[pos + 27]]);
        let name_len = u16::from_le_bytes([data[pos + 28], data[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([data[pos + 30], data[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([data[pos + 32], data[pos + 33]]) as usize;
        let local_offset = u32::from_le_bytes([data[pos + 42], data[pos + 43], data[pos + 44], data[pos + 45]]);

        let name_start = pos + 46;
        let name_end = name_start + name_len;
        if name_end > cd_offset + cd_size {
            return Err(ArchiveError::InvalidZipHeader);
        }

        let filename = core::str::from_utf8(&data[name_start..name_end])
            .map_err(|_| ArchiveError::InvalidZipHeader)?;

        if is_zip_slip_path(filename) {
            return Err(ArchiveError::ZipSlipDetected);
        }

        if uncomp_size as usize > MAX_UNCOMPRESSED_CEILING {
            return Err(ArchiveError::DecompressionBombDetected);
        }
        if comp_size > 0 && (uncomp_size as usize / comp_size as usize) > MAX_COMPRESSION_RATIO {
            return Err(ArchiveError::DecompressionBombDetected);
        }

        if archive.count < 16 {
            archive.entries[archive.count] = Some(CentralDirEntry {
                filename,
                compression_method: method,
                crc32: crc,
                compressed_size: comp_size,
                uncompressed_size: uncomp_size,
                local_header_offset: local_offset,
            });
            archive.count += 1;
        }

        pos = name_end + extra_len + comment_len;
    }

    Ok(archive)
}

pub fn extract_file(
    data: &[u8],
    entry: &CentralDirEntry,
    out_buf: &mut [u8],
) -> Result<usize, ArchiveError> {
    let loc = entry.local_header_offset as usize;
    if loc + 30 > data.len() {
        return Err(ArchiveError::InvalidZipHeader);
    }
    if data[loc] != 0x50 || data[loc + 1] != 0x4B || data[loc + 2] != 0x03 || data[loc + 3] != 0x04 {
        return Err(ArchiveError::InvalidZipHeader);
    }

    let local_name_len = u16::from_le_bytes([data[loc + 26], data[loc + 27]]) as usize;
    let local_extra_len = u16::from_le_bytes([data[loc + 28], data[loc + 29]]) as usize;
    let payload_start = loc + 30 + local_name_len + local_extra_len;
    let payload_end = payload_start + entry.compressed_size as usize;

    if payload_end > data.len() {
        return Err(ArchiveError::CorruptData);
    }

    if out_buf.len() < entry.uncompressed_size as usize {
        return Err(ArchiveError::BufferTooSmall);
    }

    let decomp_len = match entry.compression_method {
        0 => {
            let sz = entry.uncompressed_size as usize;
            out_buf[..sz].copy_from_slice(&data[payload_start..payload_start + sz]);
            sz
        }
        8 => {
            deflate_decompress(&data[payload_start..payload_end], out_buf)?
        }
        _ => return Err(ArchiveError::UnsupportedCompressionMethod),
    };

    if decomp_len != entry.uncompressed_size as usize {
        return Err(ArchiveError::CorruptData);
    }

    let computed_crc = crc32(&out_buf[..decomp_len]);
    if computed_crc != entry.crc32 {
        return Err(ArchiveError::CrcMismatch);
    }

    Ok(decomp_len)
}

#[derive(Debug, Clone, Copy)]
pub struct ZipInputFile<'a> {
    pub name: &'a str,
    pub data: &'a [u8],
    pub method: u16, // 0 = Stored, 8 = Deflated
}

pub fn create_multi_zip_archive(
    files: &[ZipInputFile],
    out_buf: &mut [u8],
) -> Result<usize, ArchiveError> {
    if files.is_empty() || files.len() > 16 {
        return Err(ArchiveError::BufferTooSmall);
    }

    for f in files {
        if is_zip_slip_path(f.name) {
            return Err(ArchiveError::ZipSlipDetected);
        }
    }

    let mut pos = 0;
    let mut local_header_offsets = [0u32; 16];
    let mut file_crcs = [0u32; 16];
    let mut comp_sizes = [0u32; 16];

    // 1. Write Local File Headers and Data
    for (i, f) in files.iter().enumerate() {
        let name_bytes = f.name.as_bytes();
        let crc = crc32(f.data);
        file_crcs[i] = crc;
        local_header_offsets[i] = pos as u32;

        let mut comp_buf = [0u8; 4096];
        let (comp_len, is_deflated) = match f.method {
            0 => (f.data.len(), false),
            8 => {
                let sz = deflate_compress(f.data, &mut comp_buf)?;
                (sz, true)
            }
            _ => return Err(ArchiveError::UnsupportedCompressionMethod),
        };
        comp_sizes[i] = comp_len as u32;

        let local_hdr_size = 30 + name_bytes.len();
        if pos + local_hdr_size + comp_len > out_buf.len() {
            return Err(ArchiveError::BufferTooSmall);
        }

        // Local Header
        out_buf[pos..pos + 4].copy_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out_buf[pos + 4..pos + 6].copy_from_slice(&20u16.to_le_bytes()); // Version 2.0
        out_buf[pos + 6..pos + 8].copy_from_slice(&0u16.to_le_bytes());  // Flags
        out_buf[pos + 8..pos + 10].copy_from_slice(&f.method.to_le_bytes());
        out_buf[pos + 10..pos + 14].copy_from_slice(&0u32.to_le_bytes()); // Mod time/date
        out_buf[pos + 14..pos + 18].copy_from_slice(&crc.to_le_bytes());
        out_buf[pos + 18..pos + 22].copy_from_slice(&(comp_len as u32).to_le_bytes());
        out_buf[pos + 22..pos + 26].copy_from_slice(&(f.data.len() as u32).to_le_bytes());
        out_buf[pos + 26..pos + 28].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out_buf[pos + 28..pos + 30].copy_from_slice(&0u16.to_le_bytes()); // Extra len
        pos += 30;

        out_buf[pos..pos + name_bytes.len()].copy_from_slice(name_bytes);
        pos += name_bytes.len();

        if is_deflated {
            out_buf[pos..pos + comp_len].copy_from_slice(&comp_buf[..comp_len]);
        } else {
            out_buf[pos..pos + comp_len].copy_from_slice(f.data);
        }
        pos += comp_len;
    }

    let central_dir_offset = pos as u32;

    // 2. Write Central Directory File Headers
    for (i, f) in files.iter().enumerate() {
        let name_bytes = f.name.as_bytes();
        let cd_hdr_size = 46 + name_bytes.len();
        if pos + cd_hdr_size > out_buf.len() {
            return Err(ArchiveError::BufferTooSmall);
        }

        out_buf[pos..pos + 4].copy_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        out_buf[pos + 4..pos + 6].copy_from_slice(&20u16.to_le_bytes()); // Made by
        out_buf[pos + 6..pos + 8].copy_from_slice(&20u16.to_le_bytes()); // Needed
        out_buf[pos + 8..pos + 10].copy_from_slice(&0u16.to_le_bytes()); // Flags
        out_buf[pos + 10..pos + 12].copy_from_slice(&f.method.to_le_bytes());
        out_buf[pos + 12..pos + 16].copy_from_slice(&0u32.to_le_bytes());
        out_buf[pos + 16..pos + 20].copy_from_slice(&file_crcs[i].to_le_bytes());
        out_buf[pos + 20..pos + 24].copy_from_slice(&comp_sizes[i].to_le_bytes());
        out_buf[pos + 24..pos + 28].copy_from_slice(&(f.data.len() as u32).to_le_bytes());
        out_buf[pos + 28..pos + 30].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out_buf[pos + 30..pos + 32].copy_from_slice(&0u16.to_le_bytes()); // Extra len
        out_buf[pos + 32..pos + 34].copy_from_slice(&0u16.to_le_bytes()); // Comment len
        out_buf[pos + 34..pos + 36].copy_from_slice(&0u16.to_le_bytes()); // Disk num
        out_buf[pos + 36..pos + 38].copy_from_slice(&0u16.to_le_bytes()); // Internal attr
        out_buf[pos + 38..pos + 42].copy_from_slice(&0u32.to_le_bytes()); // External attr
        out_buf[pos + 42..pos + 46].copy_from_slice(&local_header_offsets[i].to_le_bytes());
        pos += 46;

        out_buf[pos..pos + name_bytes.len()].copy_from_slice(name_bytes);
        pos += name_bytes.len();
    }

    let central_dir_size = (pos - central_dir_offset as usize) as u32;

    // 3. Write End of Central Directory
    if pos + 22 > out_buf.len() {
        return Err(ArchiveError::BufferTooSmall);
    }
    out_buf[pos..pos + 4].copy_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
    out_buf[pos + 4..pos + 6].copy_from_slice(&0u16.to_le_bytes());
    out_buf[pos + 6..pos + 8].copy_from_slice(&0u16.to_le_bytes());
    out_buf[pos + 8..pos + 10].copy_from_slice(&(files.len() as u16).to_le_bytes());
    out_buf[pos + 10..pos + 12].copy_from_slice(&(files.len() as u16).to_le_bytes());
    out_buf[pos + 12..pos + 16].copy_from_slice(&central_dir_size.to_le_bytes());
    out_buf[pos + 16..pos + 20].copy_from_slice(&central_dir_offset.to_le_bytes());
    out_buf[pos + 20..pos + 22].copy_from_slice(&0u16.to_le_bytes());
    pos += 22;

    Ok(pos)
}

pub fn create_zip_archive(
    filename: &str,
    file_data: &[u8],
    out_buf: &mut [u8],
) -> Result<usize, ArchiveError> {
    create_multi_zip_archive(
        &[ZipInputFile {
            name: filename,
            data: file_data,
            method: 8,
        }],
        out_buf,
    )
}
