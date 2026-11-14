// kernel/src/net/udp.rs - User Datagram Protocol (RFC 768)

use super::{Ipv4Address, NetError};
use super::ipv4::compute_checksum;

pub const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UdpHeader {
    pub fn parse(payload: &[u8]) -> Result<(Self, &[u8]), NetError> {
        if payload.len() < UDP_HEADER_LEN {
            return Err(NetError::BufferTooShort);
        }

        let src_port = u16::from_be_bytes([payload[0], payload[1]]);
        let dst_port = u16::from_be_bytes([payload[2], payload[3]]);
        let length = u16::from_be_bytes([payload[4], payload[5]]);
        let checksum = u16::from_be_bytes([payload[6], payload[7]]);

        if (length as usize) > payload.len() || (length as usize) < UDP_HEADER_LEN {
            return Err(NetError::BufferTooShort);
        }

        let data = &payload[UDP_HEADER_LEN..(length as usize)];

        Ok((
            Self {
                src_port,
                dst_port,
                length,
                checksum,
            },
            data,
        ))
    }

    pub fn write_to(
        src_ip: Ipv4Address,
        dst_ip: Ipv4Address,
        src_port: u16,
        dst_port: u16,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, NetError> {
        let total_len = UDP_HEADER_LEN + data.len();
        if out.len() < total_len {
            return Err(NetError::BufferTooShort);
        }

        out[0..2].copy_from_slice(&src_port.to_be_bytes());
        out[2..4].copy_from_slice(&dst_port.to_be_bytes());
        out[4..6].copy_from_slice(&(total_len as u16).to_be_bytes());
        out[6..8].copy_from_slice(&[0, 0]); // Zero checksum
        out[UDP_HEADER_LEN..total_len].copy_from_slice(data);

        // Compute UDP checksum with IPv4 Pseudo-Header
        let mut pseudo_buf = [0u8; 1536];
        if 12 + total_len <= pseudo_buf.len() {
            pseudo_buf[0..4].copy_from_slice(&src_ip.0);
            pseudo_buf[4..8].copy_from_slice(&dst_ip.0);
            pseudo_buf[8] = 0; // Zero
            pseudo_buf[9] = 17; // Protocol UDP
            pseudo_buf[10..12].copy_from_slice(&(total_len as u16).to_be_bytes());
            pseudo_buf[12..12 + total_len].copy_from_slice(&out[..total_len]);

            let csum = compute_checksum(&pseudo_buf[..12 + total_len]);
            let final_csum = if csum == 0 { 0xFFFF } else { csum };
            out[6..8].copy_from_slice(&final_csum.to_be_bytes());
        }

        Ok(total_len)
    }
}
