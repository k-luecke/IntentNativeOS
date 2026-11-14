// kernel/src/net/ipv4.rs - Internet Protocol Version 4 (RFC 791)

use super::{Ipv4Address, NetError};

pub const IPV4_HEADER_MIN_LEN: usize = 20;
pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

#[derive(Debug, Clone, Copy)]
pub struct Ipv4Header {
    pub version: u8,
    pub ihl: u8,
    pub tos: u8,
    pub total_len: u16,
    pub id: u16,
    pub flags_and_frag: u16,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub src: Ipv4Address,
    pub dst: Ipv4Address,
}

pub fn compute_checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        let word = u16::from_be_bytes([data[i], data[i + 1]]);
        sum = sum.wrapping_add(word as u32);
        i += 2;
    }
    if i < data.len() {
        let word = (data[i] as u16) << 8;
        sum = sum.wrapping_add(word as u32);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

impl Ipv4Header {
    pub fn parse(packet: &[u8]) -> Result<(Self, &[u8]), NetError> {
        if packet.len() < IPV4_HEADER_MIN_LEN {
            return Err(NetError::BufferTooShort);
        }

        let ver_ihl = packet[0];
        let version = ver_ihl >> 4;
        let ihl = ver_ihl & 0x0F;
        if version != 4 || ihl < 5 {
            return Err(NetError::UnsupportedProtocol);
        }

        let header_len = (ihl as usize) * 4;
        if packet.len() < header_len {
            return Err(NetError::BufferTooShort);
        }

        let tos = packet[1];
        let total_len = u16::from_be_bytes([packet[2], packet[3]]);
        if (total_len as usize) > packet.len() {
            return Err(NetError::BufferTooShort);
        }

        let id = u16::from_be_bytes([packet[4], packet[5]]);
        let flags_and_frag = u16::from_be_bytes([packet[6], packet[7]]);
        let ttl = packet[8];
        let protocol = packet[9];
        let checksum = u16::from_be_bytes([packet[10], packet[11]]);

        let mut src = [0u8; 4];
        let mut dst = [0u8; 4];
        src.copy_from_slice(&packet[12..16]);
        dst.copy_from_slice(&packet[16..20]);

        // Verify checksum: sum over header including received checksum must equal 0
        let computed = compute_checksum(&packet[..header_len]);
        if computed != 0 {
            return Err(NetError::ChecksumMismatch);
        }

        let payload = &packet[header_len..(total_len as usize)];

        Ok((
            Self {
                version,
                ihl,
                tos,
                total_len,
                id,
                flags_and_frag,
                ttl,
                protocol,
                checksum,
                src: Ipv4Address(src),
                dst: Ipv4Address(dst),
            },
            payload,
        ))
    }

    pub fn write_to(&self, out: &mut [u8], payload_len: usize) -> Result<usize, NetError> {
        if out.len() < IPV4_HEADER_MIN_LEN {
            return Err(NetError::BufferTooShort);
        }

        let total_len = (IPV4_HEADER_MIN_LEN + payload_len) as u16;
        out[0] = 0x45; // Version 4, IHL 5 (20 bytes)
        out[1] = self.tos;
        out[2..4].copy_from_slice(&total_len.to_be_bytes());
        out[4..6].copy_from_slice(&self.id.to_be_bytes());
        out[6..8].copy_from_slice(&self.flags_and_frag.to_be_bytes());
        out[8] = self.ttl;
        out[9] = self.protocol;
        out[10..12].copy_from_slice(&[0u8, 0u8]); // Zero checksum for calculation
        out[12..16].copy_from_slice(&self.src.0);
        out[16..20].copy_from_slice(&self.dst.0);

        let csum = compute_checksum(&out[..IPV4_HEADER_MIN_LEN]);
        out[10..12].copy_from_slice(&csum.to_be_bytes());

        Ok(IPV4_HEADER_MIN_LEN)
    }
}
