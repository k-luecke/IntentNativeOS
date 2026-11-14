// kernel/src/net/icmp.rs - Internet Control Message Protocol (RFC 792)

use super::NetError;
use super::ipv4::compute_checksum;

pub const ICMP_TYPE_ECHO_REPLY: u8 = 0;
pub const ICMP_TYPE_ECHO_REQUEST: u8 = 8;
pub const ICMP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct IcmpHeader {
    pub type_: u8,
    pub code: u8,
    pub checksum: u16,
    pub id: u16,
    pub seq: u16,
}

impl IcmpHeader {
    pub fn parse(payload: &[u8]) -> Result<(Self, &[u8]), NetError> {
        if payload.len() < ICMP_HEADER_LEN {
            return Err(NetError::BufferTooShort);
        }

        let type_ = payload[0];
        let code = payload[1];
        let checksum = u16::from_be_bytes([payload[2], payload[3]]);
        let id = u16::from_be_bytes([payload[4], payload[5]]);
        let seq = u16::from_be_bytes([payload[6], payload[7]]);

        let data = &payload[ICMP_HEADER_LEN..];

        Ok((
            Self {
                type_,
                code,
                checksum,
                id,
                seq,
            },
            data,
        ))
    }

    pub fn write_echo_reply(id: u16, seq: u16, data: &[u8], out: &mut [u8]) -> Result<usize, NetError> {
        let total_len = ICMP_HEADER_LEN + data.len();
        if out.len() < total_len {
            return Err(NetError::BufferTooShort);
        }

        out[0] = ICMP_TYPE_ECHO_REPLY;
        out[1] = 0; // Code 0
        out[2..4].copy_from_slice(&[0, 0]);
        out[4..6].copy_from_slice(&id.to_be_bytes());
        out[6..8].copy_from_slice(&seq.to_be_bytes());
        out[8..total_len].copy_from_slice(data);

        let csum = compute_checksum(&out[..total_len]);
        out[2..4].copy_from_slice(&csum.to_be_bytes());

        Ok(total_len)
    }
}
