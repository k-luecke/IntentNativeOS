// kernel/src/net/arp.rs - Address Resolution Protocol (RFC 826)

use super::{Ipv4Address, MacAddress, NetError};

pub const ARP_HTYPE_ETHERNET: u16 = 1;
pub const ARP_PTYPE_IPV4: u16 = 0x0800;
pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;
pub const ARP_PACKET_LEN: usize = 28;

#[derive(Debug, Clone, Copy)]
pub struct ArpPacket {
    pub htype: u16,
    pub ptype: u16,
    pub hlen: u8,
    pub plen: u8,
    pub oper: u16,
    pub sender_mac: MacAddress,
    pub sender_ip: Ipv4Address,
    pub target_mac: MacAddress,
    pub target_ip: Ipv4Address,
}

impl ArpPacket {
    pub fn parse(payload: &[u8]) -> Result<Self, NetError> {
        if payload.len() < ARP_PACKET_LEN {
            return Err(NetError::BufferTooShort);
        }

        let htype = u16::from_be_bytes([payload[0], payload[1]]);
        let ptype = u16::from_be_bytes([payload[2], payload[3]]);
        let hlen = payload[4];
        let plen = payload[5];
        let oper = u16::from_be_bytes([payload[6], payload[7]]);

        let mut s_mac = [0u8; 6];
        let mut s_ip = [0u8; 4];
        let mut t_mac = [0u8; 6];
        let mut t_ip = [0u8; 4];

        s_mac.copy_from_slice(&payload[8..14]);
        s_ip.copy_from_slice(&payload[14..18]);
        t_mac.copy_from_slice(&payload[18..24]);
        t_ip.copy_from_slice(&payload[24..28]);

        Ok(Self {
            htype,
            ptype,
            hlen,
            plen,
            oper,
            sender_mac: MacAddress(s_mac),
            sender_ip: Ipv4Address(s_ip),
            target_mac: MacAddress(t_mac),
            target_ip: Ipv4Address(t_ip),
        })
    }

    pub fn write_to(&self, out: &mut [u8]) -> Result<usize, NetError> {
        if out.len() < ARP_PACKET_LEN {
            return Err(NetError::BufferTooShort);
        }

        out[0..2].copy_from_slice(&self.htype.to_be_bytes());
        out[2..4].copy_from_slice(&self.ptype.to_be_bytes());
        out[4] = self.hlen;
        out[5] = self.plen;
        out[6..8].copy_from_slice(&self.oper.to_be_bytes());
        out[8..14].copy_from_slice(&self.sender_mac.0);
        out[14..18].copy_from_slice(&self.sender_ip.0);
        out[18..24].copy_from_slice(&self.target_mac.0);
        out[24..28].copy_from_slice(&self.target_ip.0);

        Ok(ARP_PACKET_LEN)
    }

    pub fn create_request(sender_mac: MacAddress, sender_ip: Ipv4Address, target_ip: Ipv4Address) -> Self {
        Self {
            htype: ARP_HTYPE_ETHERNET,
            ptype: ARP_PTYPE_IPV4,
            hlen: 6,
            plen: 4,
            oper: ARP_OP_REQUEST,
            sender_mac,
            sender_ip,
            target_mac: MacAddress([0x00; 6]),
            target_ip,
        }
    }

    pub fn create_reply(
        sender_mac: MacAddress,
        sender_ip: Ipv4Address,
        target_mac: MacAddress,
        target_ip: Ipv4Address,
    ) -> Self {
        Self {
            htype: ARP_HTYPE_ETHERNET,
            ptype: ARP_PTYPE_IPV4,
            hlen: 6,
            plen: 4,
            oper: ARP_OP_REPLY,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }
}

pub struct ArpTable {
    entries: [Option<(Ipv4Address, MacAddress)>; 16],
    count: usize,
}

impl ArpTable {
    pub const fn new() -> Self {
        Self {
            entries: [None; 16],
            count: 0,
        }
    }

    pub fn insert(&mut self, ip: Ipv4Address, mac: MacAddress) {
        for slot in self.entries.iter_mut() {
            if let Some((existing_ip, existing_mac)) = slot {
                if *existing_ip == ip {
                    *existing_mac = mac;
                    return;
                }
            }
        }

        for slot in self.entries.iter_mut() {
            if slot.is_none() {
                *slot = Some((ip, mac));
                self.count += 1;
                return;
            }
        }

        // Table full, evict first slot
        self.entries[0] = Some((ip, mac));
    }

    pub fn lookup(&self, ip: Ipv4Address) -> Option<MacAddress> {
        // Known default for QEMU SLIRP Gateway 10.0.2.2
        if ip == Ipv4Address::GATEWAY {
            return Some(MacAddress([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]));
        }

        for slot in self.entries.iter() {
            if let Some((cached_ip, mac)) = slot {
                if *cached_ip == ip {
                    return Some(*mac);
                }
            }
        }
        None
    }
}
