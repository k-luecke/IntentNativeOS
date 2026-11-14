// kernel/src/net/dhcp.rs - Dynamic Host Configuration Protocol Client (RFC 2131)

use super::{Ipv4Address, MacAddress, NetError};

pub const DHCP_MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99]; // 0x63825363
pub const DHCP_CLIENT_PORT: u16 = 68;
pub const DHCP_SERVER_PORT: u16 = 67;

pub const DHCP_MSG_DISCOVER: u8 = 1;
pub const DHCP_MSG_OFFER: u8 = 2;
pub const DHCP_MSG_REQUEST: u8 = 3;
pub const DHCP_MSG_ACK: u8 = 5;

#[derive(Debug, Clone, Copy)]
pub struct DhcpConfig {
    pub assigned_ip: Ipv4Address,
    pub subnet_mask: Ipv4Address,
    pub gateway_ip: Ipv4Address,
    pub dns_server: Ipv4Address,
}

impl DhcpConfig {
    pub const fn fallback() -> Self {
        Self {
            assigned_ip: Ipv4Address::LOCAL_HOST, // 10.0.2.15
            subnet_mask: Ipv4Address([255, 255, 255, 0]),
            gateway_ip: Ipv4Address::GATEWAY,    // 10.0.2.2
            dns_server: Ipv4Address([10, 0, 2, 3]),
        }
    }
}

pub struct DhcpClient {
    pub xid: u32,
    pub config: DhcpConfig,
}

impl DhcpClient {
    pub const fn new(xid: u32) -> Self {
        Self {
            xid,
            config: DhcpConfig::fallback(),
        }
    }

    /// Builds a DHCP Discover packet payload
    pub fn build_discover(&self, client_mac: MacAddress, out: &mut [u8]) -> Result<usize, NetError> {
        if out.len() < 300 {
            return Err(NetError::BufferTooShort);
        }

        for b in out.iter_mut() {
            *b = 0;
        }

        out[0] = 1; // Boot Request
        out[1] = 1; // Ethernet
        out[2] = 6; // MAC len
        out[3] = 0; // Hops

        out[4..8].copy_from_slice(&self.xid.to_be_bytes());
        out[8..10].copy_from_slice(&[0, 0]); // Secs
        out[10..12].copy_from_slice(&[0x80, 0x00]); // Broadcast flag

        // chaddr (client hardware address)
        out[28..34].copy_from_slice(&client_mac.0);

        // Magic cookie at offset 236
        out[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

        // Option 53: DHCP Message Type = Discover (1)
        out[240] = 53;
        out[241] = 1;
        out[242] = DHCP_MSG_DISCOVER;

        // Option 55: Parameter Request List (1 = Subnet Mask, 3 = Router, 6 = DNS)
        out[243] = 55;
        out[244] = 3;
        out[245] = 1;
        out[246] = 3;
        out[247] = 6;

        // Option 255: End
        out[248] = 0xFF;

        Ok(249)
    }

    /// Parses a DHCP Offer or Ack and updates configuration
    pub fn parse_reply(&mut self, payload: &[u8]) -> Result<Option<u8>, NetError> {
        if payload.len() < 240 {
            return Err(NetError::BufferTooShort);
        }

        let op = payload[0];
        if op != 2 {
            return Ok(None); // Not a boot reply
        }

        let xid = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
        if xid != self.xid {
            return Ok(None); // Not for our transaction
        }

        // yiaddr is at offset 16
        let mut yiaddr = [0u8; 4];
        yiaddr.copy_from_slice(&payload[16..20]);
        if yiaddr != [0, 0, 0, 0] {
            self.config.assigned_ip = Ipv4Address(yiaddr);
        }

        if payload[236..240] != DHCP_MAGIC_COOKIE {
            return Ok(None);
        }

        let mut msg_type = None;
        let mut idx = 240;
        while idx < payload.len() {
            let opt = payload[idx];
            if opt == 0xFF {
                break;
            }
            if opt == 0 {
                idx += 1;
                continue;
            }
            if idx + 1 >= payload.len() {
                break;
            }
            let len = payload[idx + 1] as usize;
            idx += 2;
            if idx + len > payload.len() {
                break;
            }

            match opt {
                53 => {
                    if len >= 1 {
                        msg_type = Some(payload[idx]);
                    }
                }
                1 => {
                    if len >= 4 {
                        let mut mask = [0u8; 4];
                        mask.copy_from_slice(&payload[idx..idx + 4]);
                        self.config.subnet_mask = Ipv4Address(mask);
                    }
                }
                3 => {
                    if len >= 4 {
                        let mut gw = [0u8; 4];
                        gw.copy_from_slice(&payload[idx..idx + 4]);
                        self.config.gateway_ip = Ipv4Address(gw);
                    }
                }
                6 => {
                    if len >= 4 {
                        let mut dns = [0u8; 4];
                        dns.copy_from_slice(&payload[idx..idx + 4]);
                        self.config.dns_server = Ipv4Address(dns);
                    }
                }
                _ => {}
            }
            idx += len;
        }

        Ok(msg_type)
    }
}
