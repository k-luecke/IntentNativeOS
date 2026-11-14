// kernel/src/net/ethernet.rs - Ethernet II Frame Framing & Parsing (RFC 894)

use super::{MacAddress, NetError};

pub const ETHERTYPE_IPV4: u16 = 0x0800;
pub const ETHERTYPE_ARP: u16 = 0x0806;
pub const ETH_HEADER_LEN: usize = 14;

#[derive(Debug, Clone, Copy)]
pub struct EthernetHeader {
    pub dst: MacAddress,
    pub src: MacAddress,
    pub ethertype: u16,
}

impl EthernetHeader {
    pub fn parse(packet: &[u8]) -> Result<(Self, &[u8]), NetError> {
        if packet.len() < ETH_HEADER_LEN {
            return Err(NetError::BufferTooShort);
        }

        let mut dst = [0u8; 6];
        let mut src = [0u8; 6];
        dst.copy_from_slice(&packet[0..6]);
        src.copy_from_slice(&packet[6..12]);
        let ethertype = u16::from_be_bytes([packet[12], packet[13]]);

        Ok((
            Self {
                dst: MacAddress(dst),
                src: MacAddress(src),
                ethertype,
            },
            &packet[ETH_HEADER_LEN..],
        ))
    }

    pub fn write_to(&self, out: &mut [u8]) -> Result<usize, NetError> {
        if out.len() < ETH_HEADER_LEN {
            return Err(NetError::BufferTooShort);
        }
        out[0..6].copy_from_slice(&self.dst.0);
        out[6..12].copy_from_slice(&self.src.0);
        out[12..14].copy_from_slice(&self.ethertype.to_be_bytes());
        Ok(ETH_HEADER_LEN)
    }
}
