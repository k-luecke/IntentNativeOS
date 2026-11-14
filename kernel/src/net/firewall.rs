// kernel/src/net/firewall.rs - INOS Sentinel Native Stateful Packet Inspection (SPI) Firewall (S201, S202)
//
// Enforces default-deny inbound network policy, stateful connection tracking (conntrack),
// process capability binding for network egress, active flow revocation, bounded connection table,
// egress destination filtering, and fail-closed handling for unsupported protocols / raw packets.

use super::{Ipv4Address, NetError};
use super::ipv4::{Ipv4Header, IP_PROTO_ICMP, IP_PROTO_TCP, IP_PROTO_UDP};

pub const MAX_CONNTRACK_ENTRIES: usize = 128;
pub const MAX_EGRESS_RULES: usize = 32;
pub const DEFAULT_TCP_TIMEOUT_TICKS: u64 = 300; // 300 seconds
pub const DEFAULT_UDP_TIMEOUT_TICKS: u64 = 60;  // 60 seconds
pub const DEFAULT_ICMP_TIMEOUT_TICKS: u64 = 10; // 10 seconds

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    DefaultDenyInbound,
    NoMatchingConnection,
    InvalidTcpFlags,
    ConntrackTableFull,
    CapabilityMissing,
    CapabilityRevoked,
    DestinationNotPermitted,
    UnsupportedProtocolExtension,
    RawSocketUnauthorized,
    ProcessIdentityMismatch,
    MalformedPacket,
    ProtocolBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallVerdict {
    Accept,
    Drop(DropReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConntrackState {
    New,
    Established,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FiveTuple {
    pub src_ip: Ipv4Address,
    pub dst_ip: Ipv4Address,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
}

impl FiveTuple {
    pub const fn empty() -> Self {
        Self {
            src_ip: Ipv4Address::UNASSIGNED,
            dst_ip: Ipv4Address::UNASSIGNED,
            src_port: 0,
            dst_port: 0,
            protocol: 0,
        }
    }

    pub fn reverse(&self) -> Self {
        Self {
            src_ip: self.dst_ip,
            dst_ip: self.src_ip,
            src_port: self.dst_port,
            dst_port: self.src_port,
            protocol: self.protocol,
        }
    }

    pub fn matches_reverse(&self, other: &FiveTuple) -> bool {
        self.protocol == other.protocol
            && self.src_ip == other.dst_ip
            && self.dst_ip == other.src_ip
            && self.src_port == other.dst_port
            && self.dst_port == other.src_port
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConntrackEntry {
    pub tuple: FiveTuple,
    pub state: ConntrackState,
    pub owner_pid: u64,
    pub cap_token: u64,
    pub last_seen_tick: u64,
    pub timeout_ticks: u64,
    pub bytes_forward: u64,
    pub bytes_reverse: u64,
    pub packets_forward: u32,
    pub packets_reverse: u32,
    pub is_active: bool,
}

impl ConntrackEntry {
    pub const fn empty() -> Self {
        Self {
            tuple: FiveTuple::empty(),
            state: ConntrackState::Closed,
            owner_pid: 0,
            cap_token: 0,
            last_seen_tick: 0,
            timeout_ticks: DEFAULT_TCP_TIMEOUT_TICKS,
            bytes_forward: 0,
            bytes_reverse: 0,
            packets_forward: 0,
            packets_reverse: 0,
            is_active: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EgressRule {
    pub pid: u64,
    pub cap_token: u64,
    pub allowed_dst_ip: Ipv4Address,
    pub allowed_dst_port: u16,
    pub protocol: u8,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FirewallTelemetry {
    pub total_packets_in: u64,
    pub total_packets_out: u64,
    pub total_accepted: u64,
    pub total_dropped: u64,
    pub active_connections: usize,
    pub peak_connections: usize,
    pub default_deny_drops: u64,
    pub capability_missing_drops: u64,
    pub capability_revoked_drops: u64,
    pub destination_not_permitted_drops: u64,
    pub unsupported_protocol_drops: u64,
    pub raw_socket_unauthorized_drops: u64,
}

pub struct StatefulFirewall {
    table: [Option<ConntrackEntry>; MAX_CONNTRACK_ENTRIES],
    egress_rules: [Option<EgressRule>; MAX_EGRESS_RULES],
    current_tick: u64,
    telemetry: FirewallTelemetry,
}

impl StatefulFirewall {
    pub const fn new() -> Self {
        Self {
            table: [None; MAX_CONNTRACK_ENTRIES],
            egress_rules: [None; MAX_EGRESS_RULES],
            current_tick: 0,
            telemetry: FirewallTelemetry {
                total_packets_in: 0,
                total_packets_out: 0,
                total_accepted: 0,
                total_dropped: 0,
                active_connections: 0,
                peak_connections: 0,
                default_deny_drops: 0,
                capability_missing_drops: 0,
                capability_revoked_drops: 0,
                destination_not_permitted_drops: 0,
                unsupported_protocol_drops: 0,
                raw_socket_unauthorized_drops: 0,
            },
        }
    }

    pub fn set_current_tick(&mut self, tick: u64) {
        self.current_tick = tick;
    }

    pub fn add_egress_rule(&mut self, rule: EgressRule) -> Result<(), DropReason> {
        let slot = self.egress_rules.iter().position(|r| r.is_none())
            .ok_or(DropReason::ConntrackTableFull)?;
        self.egress_rules[slot] = Some(rule);
        Ok(())
    }

    /// Revokes all active conntrack flows associated with a capability token or PID (S202).
    /// Immediately closes connection and blocks subsequent packets.
    pub fn revoke_capability_flows(&mut self, owner_pid: u64, cap_token: u64) -> usize {
        let mut count = 0;
        for i in 0..MAX_CONNTRACK_ENTRIES {
            if let Some(ref mut entry) = self.table[i] {
                if entry.owner_pid == owner_pid && (cap_token == 0 || entry.cap_token == cap_token) {
                    entry.is_active = false;
                    entry.state = ConntrackState::Closed;
                    self.table[i] = None;
                    self.telemetry.active_connections = self.telemetry.active_connections.saturating_sub(1);
                    self.telemetry.capability_revoked_drops += 1;
                    count += 1;
                }
            }
        }
        for r in self.egress_rules.iter_mut() {
            if let Some(ref rule) = r {
                if rule.pid == owner_pid && (cap_token == 0 || rule.cap_token == cap_token) {
                    *r = None;
                }
            }
        }
        count
    }

    pub fn terminate_all_flows_for_pid(&mut self, owner_pid: u64) -> usize {
        self.revoke_capability_flows(owner_pid, 0)
    }

    /// Enforces fail-closed handling for IPv6, extension headers, and fragmentation (S202)
    pub fn check_ipv6_or_unsupported(packet: &[u8]) -> Result<(), DropReason> {
        if packet.is_empty() {
            return Err(DropReason::MalformedPacket);
        }
        let version = packet[0] >> 4;
        if version == 6 {
            // IPv6 packets fail-closed until full IPv6 stack is verified
            return Err(DropReason::UnsupportedProtocolExtension);
        } else if version != 4 {
            return Err(DropReason::MalformedPacket);
        }
        Ok(())
    }

    /// Blocks unmediated raw ethernet frames from Ring 3 (S202)
    pub fn check_raw_egress(is_ring0: bool) -> Result<(), DropReason> {
        if !is_ring0 {
            return Err(DropReason::RawSocketUnauthorized);
        }
        Ok(())
    }

    pub fn parse_five_tuple(packet: &[u8]) -> Result<FiveTuple, DropReason> {
        let (ipv4, payload) = Ipv4Header::parse(packet).map_err(|_| DropReason::MalformedPacket)?;
        let protocol = ipv4.protocol;

        let (src_port, dst_port) = match protocol {
            IP_PROTO_TCP => {
                if payload.len() < 4 {
                    return Err(DropReason::MalformedPacket);
                }
                let src_p = u16::from_be_bytes([payload[0], payload[1]]);
                let dst_p = u16::from_be_bytes([payload[2], payload[3]]);
                (src_p, dst_p)
            }
            IP_PROTO_UDP => {
                if payload.len() < 4 {
                    return Err(DropReason::MalformedPacket);
                }
                let src_p = u16::from_be_bytes([payload[0], payload[1]]);
                let dst_p = u16::from_be_bytes([payload[2], payload[3]]);
                (src_p, dst_p)
            }
            IP_PROTO_ICMP => {
                if payload.len() < 2 {
                    return Err(DropReason::MalformedPacket);
                }
                (payload[0] as u16, payload[1] as u16)
            }
            _ => return Err(DropReason::ProtocolBlocked),
        };

        Ok(FiveTuple {
            src_ip: ipv4.src,
            dst_ip: ipv4.dst,
            src_port,
            dst_port,
            protocol,
        })
    }

    /// Evaluates an outbound packet from a local process with capability & destination context (S201, S202)
    pub fn evaluate_outbound_with_context(
        &mut self,
        packet: &[u8],
        caller_pid: u64,
        cap_token: u64,
        has_net_capability: bool,
    ) -> FirewallVerdict {
        self.telemetry.total_packets_out += 1;

        // Invariant 1: Caller must hold network capability
        if !has_net_capability {
            self.telemetry.total_dropped += 1;
            self.telemetry.capability_missing_drops += 1;
            return FirewallVerdict::Drop(DropReason::CapabilityMissing);
        }

        // Invariant 2: Check for unsupported protocol or IPv6
        if let Err(reason) = Self::check_ipv6_or_unsupported(packet) {
            self.telemetry.total_dropped += 1;
            self.telemetry.unsupported_protocol_drops += 1;
            return FirewallVerdict::Drop(reason);
        }

        let tuple = match Self::parse_five_tuple(packet) {
            Ok(t) => t,
            Err(reason) => {
                self.telemetry.total_dropped += 1;
                return FirewallVerdict::Drop(reason);
            }
        };

        // Invariant 3: Egress destination rule checking (if rules registered for this process)
        let mut has_rules = false;
        let mut matched_rule = false;
        for r in self.egress_rules.iter().flatten() {
            if r.pid == caller_pid && (r.cap_token == 0 || r.cap_token == cap_token) {
                has_rules = true;
                if r.allowed_dst_ip == tuple.dst_ip && (r.allowed_dst_port == 0 || r.allowed_dst_port == tuple.dst_port) {
                    matched_rule = true;
                    break;
                }
            }
        }
        if has_rules && !matched_rule {
            self.telemetry.total_dropped += 1;
            self.telemetry.destination_not_permitted_drops += 1;
            return FirewallVerdict::Drop(DropReason::DestinationNotPermitted);
        }

        // Check if connection already exists in conntrack table
        let mut found_slot = None;
        for i in 0..MAX_CONNTRACK_ENTRIES {
            if let Some(ref mut entry) = self.table[i] {
                if entry.is_active && entry.tuple == tuple {
                    // Check if capability token matches or was revoked
                    if entry.owner_pid != caller_pid || (cap_token != 0 && entry.cap_token != cap_token) {
                        self.telemetry.total_dropped += 1;
                        self.telemetry.capability_revoked_drops += 1;
                        return FirewallVerdict::Drop(DropReason::ProcessIdentityMismatch);
                    }
                    entry.last_seen_tick = self.current_tick;
                    entry.bytes_forward += packet.len() as u64;
                    entry.packets_forward += 1;
                    self.telemetry.total_accepted += 1;
                    return FirewallVerdict::Accept;
                }
            } else if found_slot.is_none() {
                found_slot = Some(i);
            }
        }

        // New outbound connection: allocate conntrack entry
        let timeout = match tuple.protocol {
            IP_PROTO_TCP => DEFAULT_TCP_TIMEOUT_TICKS,
            IP_PROTO_UDP => DEFAULT_UDP_TIMEOUT_TICKS,
            _ => DEFAULT_ICMP_TIMEOUT_TICKS,
        };

        let free_slot = match found_slot {
            Some(idx) => idx,
            None => {
                self.expire_stale_connections();
                self.find_free_slot().unwrap_or(MAX_CONNTRACK_ENTRIES)
            }
        };

        if free_slot >= MAX_CONNTRACK_ENTRIES {
            self.telemetry.total_dropped += 1;
            return FirewallVerdict::Drop(DropReason::ConntrackTableFull);
        }

        self.table[free_slot] = Some(ConntrackEntry {
            tuple,
            state: ConntrackState::New,
            owner_pid: caller_pid,
            cap_token,
            last_seen_tick: self.current_tick,
            timeout_ticks: timeout,
            bytes_forward: packet.len() as u64,
            bytes_reverse: 0,
            packets_forward: 1,
            packets_reverse: 0,
            is_active: true,
        });

        self.telemetry.active_connections += 1;
        if self.telemetry.active_connections > self.telemetry.peak_connections {
            self.telemetry.peak_connections = self.telemetry.active_connections;
        }

        self.telemetry.total_accepted += 1;
        FirewallVerdict::Accept
    }

    /// Evaluates an outbound packet without explicit token (legacy S201 compatibility)
    pub fn evaluate_outbound(
        &mut self,
        packet: &[u8],
        caller_pid: u64,
        has_net_capability: bool,
    ) -> FirewallVerdict {
        self.evaluate_outbound_with_context(packet, caller_pid, 0, has_net_capability)
    }

    /// Evaluates an inbound packet from the wire (S201: Default Deny)
    pub fn evaluate_inbound(&mut self, packet: &[u8]) -> FirewallVerdict {
        self.telemetry.total_packets_in += 1;

        if let Err(reason) = Self::check_ipv6_or_unsupported(packet) {
            self.telemetry.total_dropped += 1;
            self.telemetry.unsupported_protocol_drops += 1;
            return FirewallVerdict::Drop(reason);
        }

        let incoming_tuple = match Self::parse_five_tuple(packet) {
            Ok(t) => t,
            Err(reason) => {
                self.telemetry.total_dropped += 1;
                return FirewallVerdict::Drop(reason);
            }
        };

        // Match against existing active connection in reverse direction
        for i in 0..MAX_CONNTRACK_ENTRIES {
            if let Some(ref mut entry) = self.table[i] {
                if entry.is_active && entry.tuple.matches_reverse(&incoming_tuple) {
                    // Check if entry has expired
                    if self.current_tick > entry.last_seen_tick + entry.timeout_ticks {
                        entry.is_active = false;
                        self.table[i] = None;
                        self.telemetry.active_connections = self.telemetry.active_connections.saturating_sub(1);
                        self.telemetry.total_dropped += 1;
                        self.telemetry.default_deny_drops += 1;
                        return FirewallVerdict::Drop(DropReason::NoMatchingConnection);
                    }

                    // Update conntrack state to Established on response
                    if entry.state == ConntrackState::New {
                        entry.state = ConntrackState::Established;
                    }
                    entry.last_seen_tick = self.current_tick;
                    entry.bytes_reverse += packet.len() as u64;
                    entry.packets_reverse += 1;
                    self.telemetry.total_accepted += 1;
                    return FirewallVerdict::Accept;
                }
            }
        }

        // Default Deny: Unsolicited inbound packets are dropped fail-closed
        self.telemetry.total_dropped += 1;
        self.telemetry.default_deny_drops += 1;
        FirewallVerdict::Drop(DropReason::DefaultDenyInbound)
    }

    pub fn expire_stale_connections(&mut self) -> usize {
        let mut expired_count = 0;
        for i in 0..MAX_CONNTRACK_ENTRIES {
            if let Some(ref entry) = self.table[i] {
                if entry.is_active && self.current_tick > entry.last_seen_tick + entry.timeout_ticks {
                    self.table[i] = None;
                    self.telemetry.active_connections = self.telemetry.active_connections.saturating_sub(1);
                    expired_count += 1;
                }
            }
        }
        expired_count
    }

    fn find_free_slot(&self) -> Option<usize> {
        self.table.iter().position(|slot| slot.is_none())
    }

    pub fn get_telemetry(&self) -> FirewallTelemetry {
        self.telemetry
    }

    pub fn reset_telemetry(&mut self) {
        self.telemetry = FirewallTelemetry::default();
    }
}

pub static mut GLOBAL_FIREWALL: StatefulFirewall = StatefulFirewall::new();
